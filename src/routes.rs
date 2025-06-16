use crate::{post::QueryPost, AppState, MainPage, Post, PostsPage};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use rust_embed::RustEmbed;
use serde::Deserialize;

pub async fn main_page(app: State<AppState>) -> impl IntoResponse {
    let queried = sqlx::query_as!(
        QueryPost,
        r#"
        SELECT *
        FROM posts
        WHERE content_type != 'special'
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

#[derive(Deserialize)]
pub struct Pagination {
    page: Option<usize>,
}

pub async fn posts_index(pagination: Query<Pagination>, app: State<AppState>) -> impl IntoResponse {
    let page = pagination.page.unwrap_or(1);
    const POSTS_PER_PAGE: i64 = 10;
    let offset = (page as i64 - 1) * POSTS_PER_PAGE;

    // Fetch one page of posts
    let queried = sqlx::query_as!(
        QueryPost,
        r#"
        SELECT *
        FROM posts
        WHERE content_type != 'special'
        ORDER BY date DESC
        LIMIT ? OFFSET ?
        "#,
        POSTS_PER_PAGE,
        offset
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();

    let mut posts: Vec<Post> = Vec::new();
    for post in queried.iter() {
        posts.push(post.clone().into_post(app.clone()).await);
    }

    // Get total number of posts to calculate total pages
    let total_posts: i64 =
        sqlx::query!("SELECT COUNT(*) as count FROM posts WHERE content_type != 'special'")
            .fetch_one(&app.pool)
            .await
            .unwrap()
            .count;

    let total_pages = (total_posts as f64 / POSTS_PER_PAGE as f64).ceil() as usize;

    PostsPage {
        title: "All Posts".to_string(),
        posts,
        current_page: page,
        total_pages,
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
