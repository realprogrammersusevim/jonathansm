use crate::{get_final_post, AppState, HtmlTemplate, IntoResponse, MainPage, PostSummary, Table};
use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use rust_embed::RustEmbed;

pub async fn main_page(app: State<AppState>) -> impl IntoResponse {
    let posts = sqlx::query_as!(
        PostSummary,
        r#"
        SELECT id, title, date
        FROM posts
        ORDER BY date DESC
        LIMIT 5
        "#,
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();

    let template = MainPage {
        title: "Jonathan's Blog".to_string(),
        posts,
    };
    HtmlTemplate(template)
}

pub async fn about(app: State<AppState>) -> impl IntoResponse {
    match get_final_post("about", Table::Special, app).await {
        Ok(about) => HtmlTemplate(about).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch about page",
        )
            .into_response(),
    }
}

pub async fn contact(app: State<AppState>) -> impl IntoResponse {
    match get_final_post("contact", Table::Special, app).await {
        Ok(contact) => HtmlTemplate(contact).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch contact page",
        )
            .into_response(),
    }
}

pub async fn post(Path(id): Path<String>, app: State<AppState>) -> impl IntoResponse {
    let post = get_final_post(&id, Table::Posts, app).await;

    // If the post is found, render the post template
    // If the post is not found, return a 404
    match post {
        Ok(post) => HtmlTemplate(post).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Post not found").into_response(),
    }
}

#[derive(RustEmbed, Clone)]
#[folder = "static/"]
pub struct Static;

#[derive(RustEmbed, Clone)]
#[folder = ".well-known/"]
pub struct WellKnown;
