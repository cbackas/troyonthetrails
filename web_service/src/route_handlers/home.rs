pub async fn handler() -> impl axum::response::IntoResponse {
    let last_updated = match db_service::get_troy_status().await.trail_status_updated {
        None => "never".to_string(),
        Some(last_updated) => {
            let elapsed = last_updated.elapsed().unwrap();
            if elapsed.as_secs() > 14400 {
                db_service::set_troy_status(false).await;
            }

            let elapsed = humantime::format_duration(elapsed).to_string();
            let elapsed = elapsed
                .split_whitespace()
                .filter(|group| {
                    !group.contains("ms") && !group.contains("us") && !group.contains("ns")
                })
                .collect::<Vec<&str>>()
                .join(" ");
            format!("{elapsed} ago")
        }
    };

    let has_strava_token = strava_service::auth::get_token().await.is_some();

    let template = HomeTemplate {
        last_updated,
        has_strava_token,
    };
    super::html_template::HtmlTemplate(template)
}

#[derive(askama::Template)]
#[template(path = "pages/home.html")]
struct HomeTemplate {
    last_updated: String,
    has_strava_token: bool,
}
