use crate::app::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;
use serde::Deserialize;
use tera::Context;

pub async fn main_page(state: State<AppState>) -> Response {
    match state.post_service.get_main_posts().await {
        Ok(posts) => {
            let mut context = Context::new();
            context.insert("title", "Jonathan's Blog");
            context.insert("posts", &posts);
            state.render("index.html", &context).unwrap()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct Pagination {
    page: Option<usize>,
}

pub async fn posts_index(pagination: Query<Pagination>, state: State<AppState>) -> Response {
    let page = pagination.page.unwrap_or(1);
    match state.post_service.get_paginated_posts(page).await {
        Ok((posts, current_page, total_pages)) => {
            let mut context = Context::new();
            context.insert("title", "All Posts");
            context.insert("posts", &posts);
            context.insert("current_page", &current_page);
            context.insert("total_pages", &total_pages);
            state.render("posts.html", &context).unwrap()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn about(state: State<AppState>) -> Response {
    match state.post_service.get_special_page("about").await {
        Ok(post) => {
            let mut context = Context::new();
            context.insert("post", &post);
            state.render("post.html", &context).unwrap()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn contact(state: State<AppState>) -> Response {
    match state.post_service.get_special_page("contact").await {
        Ok(post) => {
            let mut context = Context::new();
            context.insert("post", &post);
            state.render("post.html", &context).unwrap()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn post(Path(id): Path<String>, state: State<AppState>) -> Response {
    match state.post_service.get_post(&id).await {
        Ok(post) => {
            let mut context = Context::new();
            context.insert("post", &post);
            state.render("post.html", &context).unwrap()
        }
        Err(_) => (StatusCode::NOT_FOUND, "Post not found").into_response(),
    }
}

#[derive(RustEmbed, Clone)]
#[folder = "static/"]
pub struct Static;

#[derive(RustEmbed, Clone)]
#[folder = ".well-known/"]
pub struct WellKnown;
