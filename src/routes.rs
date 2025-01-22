use crate::{post::QueryPost, AppState, MainPage, Post};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use rust_embed::RustEmbed;

pub async fn main_page(app: State<AppState>) -> impl IntoResponse {
    let queried = sqlx::query_as!(
        QueryPost,
        r#"
        SELECT *
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

    MainPage {
        title: "Jonathan's Blog".to_string(),
        posts,
    }
}

pub async fn about(app: State<AppState>) -> impl IntoResponse {
    let query = QueryPost::fetch_special("about", app.clone())
        .await
        .unwrap(); // If this crashes we've got major problems
    query.into_post(app).await
}

pub async fn contact(app: State<AppState>) -> impl IntoResponse {
    let query = QueryPost::fetch_special("contact", app.clone())
        .await
        .unwrap();
    query.into_post(app).await
}

pub async fn post(Path(id): Path<String>, app: State<AppState>) -> impl IntoResponse {
    let query = QueryPost::fetch(&id, app.clone()).await;
    match query {
        Ok(post) => post.into_post(app.clone()).await.into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Post not found").into_response(),
    }
}

#[derive(RustEmbed, Clone)]
#[folder = "static/"]
pub struct Static;

#[derive(RustEmbed, Clone)]
#[folder = ".well-known/"]
pub struct WellKnown;
