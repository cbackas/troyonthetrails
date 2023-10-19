use std::env;

use tracing::error;

pub fn get_host_uri(port: Option<u16>) -> String {
    let host_uri = match env::var("HOST") {
        Ok(host) => format!("https://{}", host),
        _ => match env::var("FLY_APP_NAME") {
            Ok(host) => format!("https://{}.fly.dev", host),
            _ => {
                let port = match port {
                    Some(port) => port,
                    _ => get_port(),
                };
                format!("http://localhost:{}", port)
            }
        },
    };
    host_uri
}

pub fn get_port() -> u16 {
    let default_port: u16 = 8080;

    let port = match env::var("PORT") {
        Ok(port) => port,
        _ => default_port.to_string(),
    };
    let port: u16 = match port.parse::<_>() {
        Ok(port) => port,
        _ => {
            error!("Failed to parse PORT env var, using default");
            default_port
        }
    };

    port
}
