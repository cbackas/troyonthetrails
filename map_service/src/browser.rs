use fantoccini::{self, Locator};

pub async fn get_screenshot(url: &str) -> anyhow::Result<Vec<u8>> {
    let caps = {
        let mut caps = serde_json::map::Map::new();
        caps.insert("browserName".to_string(), serde_json::json!("firefox"));
        caps.insert(
            "moz:firefoxOptions".to_string(),
            serde_json::json!({
                "args": ["--headless", "--disable-extensions", "--no-sandbox", "--disable-gpu", "--disable-application-cache", "--disable-dev-shm-usage"],
            }),
        );
        caps
    };

    let client = fantoccini::ClientBuilder::native()
        .capabilities(caps)
        .connect("http://localhost:4444")
        .await?;

    client.set_window_size(1600, 1600).await?;

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
