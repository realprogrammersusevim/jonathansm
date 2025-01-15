use crate::{post::QueryPost, AppState, HtmlTemplate, IntoResponse, MainPage, Post};
use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use rust_embed::RustEmbed;

pub async fn main_page(app: State<AppState>) -> impl IntoResponse {
    let queried = sqlx::query_as!(
        QueryPost,
        r#"
        SELECT id, title, date, content, via, link, commits
        FROM posts
        ORDER BY date DESC
        LIMIT 5
        "#,
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();

    let mut posts: Vec<Post> = Vec::new();
    for post in queried.iter() {
        posts.push(post.clone().into_post(app.clone()).await);
    }

    let template = MainPage {
        title: "Jonathan's Blog".to_string(),
        posts,
    };
    HtmlTemplate(template)
}

pub async fn about(app: State<AppState>) -> impl IntoResponse {
    let query = QueryPost::fetch_special("about", app.clone())
        .await
        .unwrap(); // If this crashes we've got major problems
    HtmlTemplate(query.into_post(app).await).into_response()
}

pub async fn contact(app: State<AppState>) -> impl IntoResponse {
    let query = QueryPost::fetch_special("contact", app.clone())
        .await
        .unwrap();
    HtmlTemplate(query.into_post(app).await).into_response()
}

pub async fn post(Path(id): Path<String>, app: State<AppState>) -> impl IntoResponse {
    let query = QueryPost::fetch(&id, app.clone()).await;
    match query {
        Ok(post) => HtmlTemplate(post.into_post(app.clone()).await).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Post not found").into_response(),
    }
}

#[derive(RustEmbed, Clone)]
#[folder = "static/"]
pub struct Static;

#[derive(RustEmbed, Clone)]
#[folder = ".well-known/"]
pub struct WellKnown;
