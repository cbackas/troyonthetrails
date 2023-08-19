use std::sync::Arc;
use std::{ops::AddAssign, str};

use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::log::{debug, error, info};

use crate::AppState;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum HAMessage {
    #[serde(rename = "auth_required")]
    HAAuthRequired(HAAuthRequired),
    #[serde(rename = "auth_ok")]
    HAAuthOk(HAAuthOk),
    #[serde(rename = "auth_invalid")]
    HAAuthInvalid(HAAuthInvalid),
    #[serde(rename = "result")]
    HAResult(HAResult),
    #[serde(rename = "event")]
    HAEvent(HAEvent),
}

#[derive(Debug, Deserialize)]
pub struct HAAuthRequired {
    ha_version: String,
}

#[derive(Debug, Deserialize)]
pub struct HAAuthOk {
    ha_version: String,
}

#[derive(Debug, Deserialize)]
pub struct HAAuthInvalid {
    message: String,
}

#[derive(Debug, Deserialize)]
pub struct HAResult {
    id: u64,
    success: bool,
    result: Option<Value>, // Can hold any valid JSON value
}

#[derive(Debug, Deserialize)]
pub struct HAEvent {
    id: u64,
    event: EventDetails,
}

#[derive(Debug, Deserialize)]
pub struct EventDetails {
    pub context: EventContext,
    pub variables: Variables,
}

#[derive(Debug, Deserialize)]
pub struct EventContext {
    pub id: String,
    pub parent_id: Option<String>,
    pub user_id: String,
}

#[derive(Debug, Deserialize)]
pub struct Variables {
    pub trigger: EventTrigger,
}

#[derive(Debug, Deserialize)]
pub struct EventTrigger {
    pub alias: Option<String>,
    pub attribute: Option<String>,
    pub description: String,
    pub entity_id: String,
    pub for_: Option<String>,
    #[serde(rename = "from_state")]
    pub from_state: State,
    pub id: String,
    pub idx: String,
    pub platform: String,
    #[serde(rename = "to_state")]
    pub to_state: State,
}

#[derive(Debug, Deserialize)]
pub struct State {
    pub attributes: Attributes,
    pub context: EventContext,
    pub entity_id: String,
    pub last_changed: String,
    pub last_updated: String,
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct Attributes {
    pub altitude: f64,
    pub battery_level: u8,
    pub friendly_name: String,
    pub gps_accuracy: f64,
    pub latitude: f64,
    pub longitude: f64,
    pub source_type: String,
    pub speed: u8,
    pub vertical_accuracy: f64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HAAuthRequest {
    #[serde(rename = "type")]
    pub type_field: String,
    access_token: String,
}

#[derive(Debug, Serialize)]
pub struct SubscribeTriggerRequest {
    pub id: u64,
    #[serde(rename = "type")]
    pub type_field: String,
    pub trigger: Trigger,
}

#[derive(Debug, Serialize)]
pub struct Trigger {
    pub platform: String,
    pub entity_id: String,
}

pub async fn ws_connect(state: &Arc<Mutex<AppState>>) -> anyhow::Result<()> {
    let url = url::Url::parse("wss://ha.zac.gg/api/websocket").unwrap();

    let (ws_stream, _response) = connect_async(url)
        .await
        .context("Failed to connect to home assistant websocket server")?;

    let (mut write, read) = ws_stream.split();

    let (tx, mut rx) = mpsc::channel(32);

    tokio::spawn(async move {
        while let Some(json) = rx.recv().await {
            write.send(Message::Text(json)).await.unwrap();
        }
    });

    read.for_each(move |message| {
        let tx = tx.clone();
        async move {
            let data = message.unwrap().into_data();
            let message_str = str::from_utf8(&data).unwrap();
            match serde_json::from_str(message_str) {
                Ok(message) => {
                    debug!("Received message: {}", message_str);

                    match message {
                        HAMessage::HAAuthRequired(_) => {
                            info!("Auth required, sending...");

                            let auth_request = HAAuthRequest {
                                type_field: "auth".to_string(),
                                access_token: std::env::var("HA_TOKEN").unwrap(),
                            };

                            let json = serde_json::to_string(&auth_request).unwrap();
                            tx.send(json).await.unwrap();
                        }

                        HAMessage::HAAuthOk(_) => {
                            info!("Auth ok. Subscribing to state changes...");

                            let subscribe_trigger_request = SubscribeTriggerRequest {
                                id: 2,
                                type_field: "subscribe_trigger".to_string(),
                                trigger: Trigger {
                                    platform: "state".to_string(),
                                    entity_id: "device_tracker.zacs_iphone_2".to_string(),
                                },
                            };

                            let json = serde_json::to_string(&subscribe_trigger_request).unwrap();
                            tx.send(json).await.unwrap();
                        }

                        HAMessage::HAAuthInvalid(_) => {
                            error!("Provided auth invalid");
                        }

                        HAMessage::HAResult(_result) => {
                            info!("Subscribed to state changes");
                        }

                        HAMessage::HAEvent(event) => {
                            let trigger = event.event.variables.trigger;
                            if trigger.from_state.state == "home"
                                && trigger.to_state.state == "away"
                            {
                                info!("Troy is on the trails!");
                                state.lock().await.is_troy_on_the_trails = true;
                            }

                            if trigger.from_state.state == "away"
                                && trigger.to_state.state == "home"
                            {
                                info!("Troy is home!");
                                state.lock().await.is_troy_on_the_trails = false;
                            }
                        }
                    };
                }

                Err(e) => {
                    if message_str != "" {
                        info!("Error parsing message: {}", e);
                    }
                }
            };
        }
    })
    .await;

    Ok(())
}
