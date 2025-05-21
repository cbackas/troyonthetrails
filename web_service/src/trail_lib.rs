use anyhow::Context;
use shared_lib::trail_structs::TrailSystem;

pub async fn get_trail_data() -> anyhow::Result<Vec<TrailSystem>> {
    let html = get_trail_html().await?;
    let trail_systems = extract_trail_data(html)?;
    Ok(trail_systems)
}

async fn get_trail_html() -> anyhow::Result<String> {
    let url =
        std::env::var("TRAIL_DATA_URL").context("TRAIL_DATA_URL environment variable not found")?;

    let resp = reqwest::get(url)
        .await
        .context("Failed to get HTML from data source")?;
    let html = resp.text().await.context("Couldn't find html body")?;

    tracing::trace!("Fetched trail data from data source");

    Ok(html)
}

fn extract_trail_data(html: String) -> anyhow::Result<Vec<TrailSystem>> {
    let start_tag = "var trail_systems = ";
    let end_tag = ";</script>";

    let start = html
        .find(start_tag)
        .ok_or(anyhow::anyhow!("Start tag not found"))?
        + start_tag.len();
    let end = html[start..]
        .find(end_tag)
        .ok_or(anyhow::anyhow!("End tag not found"))?
        + start;

    let json = &html[start..end];

    let trail_systems = serde_json::from_str(json);
    match trail_systems {
        Ok(trail_systems) => Ok(trail_systems),
        Err(err) => Err(err.into()),
    }
}
