pub async fn testingthing(key: &str, data: Vec<u8>) -> anyhow::Result<()> {
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = aws_sdk_s3::Client::new(&config);

    client
        .put_object()
        .bucket(std::env::var("BUCKET_NAME").expect("BUCKET_NAME env var required"))
        .key(format!("ride_images/{}.png", key))
        .body(data.into())
        .set_content_type(Some("image/png".to_string()))
        .send()
        .await?;
    Ok(())
}
