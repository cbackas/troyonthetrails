use fantoccini::{self, Locator};
use std::time::Duration;

pub async fn get_screenshot(url: &str) -> anyhow::Result<Vec<u8>> {
    let caps = {
        let mut caps = serde_json::map::Map::new();
        caps.insert("browserName".to_string(), serde_json::json!("chrome"));
        caps.insert(
            "goog:chromeOptions".to_string(),
            serde_json::json!({
                "args": ["--headless", "--disable-gpu", "--window-size=1600,1600"]
            }),
        );
        caps
    };

    let client = fantoccini::ClientBuilder::native()
        .capabilities(caps)
        .connect("http://localhost:4444")
        .await?;

    client.goto(url).await?;
    client.wait().for_element(Locator::Css("canvas")).await?;
    client
        .wait()
        .for_element(Locator::Css(
            "#tiles-loaded-indicator[style='display: block;']",
        ))
        .await?;
    let image = client.screenshot().await?;
    client.close().await?;

    Ok(image)
}
