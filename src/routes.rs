use crate::{app::AppState, services::search_query::SearchQuery};
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

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    q: Option<String>,
    page: Option<usize>,
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

async fn handle_special_page(state: State<AppState>, page_id: &str) -> Response {
    match state.post_service.get_special_page(page_id).await {
        Ok(post) => {
            let mut context = Context::new();
            context.insert("post", &post);
            state.render("post.html", &context).unwrap_or_else(|e| {
                tracing::error!("Rendering error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            })
        }
        Err(e) => {
            tracing::error!("Database error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn about(state: State<AppState>) -> Response {
    handle_special_page(state, "about").await
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
    if id.is_empty() || id.len() > 100 {
        return StatusCode::BAD_REQUEST.into_response();
    }

    match state.post_service.get_post(&id).await {
        Ok(post) => {
            let mut context = Context::new();
            context.insert("post", &post);
            state.render("post.html", &context).unwrap_or_else(|e| {
                tracing::error!("Rendering error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            })
        }
        Err(e) => {
            if e.to_string().contains("not found") {
                StatusCode::NOT_FOUND.into_response()
            } else {
                tracing::error!("Database error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

pub async fn search(Query(params): Query<SearchParams>, state: State<AppState>) -> Response {
    let query_str = params.q.unwrap_or_default();
    let page = params.page.unwrap_or(1);
    let per_page = 10;

    let search_query = SearchQuery::from_raw(&query_str);
    match state
        .search_service
        .search(&search_query, page, per_page)
        .await
    {
        Ok((posts, total)) => {
            let mut context = Context::new();
            context.insert("query", &query_str);
            context.insert("posts", &posts);
            context.insert("current_page", &page);

            let total_pages = (total as f64 / per_page as f64).ceil() as usize;
            context.insert("total_pages", &total_pages);
            context.insert("per_page", &per_page);
            context.insert("total_results", &total);

            state.render("search.html", &context).unwrap()
        }
        Err(err) => {
            tracing::error!("Search failed: {:?}", err);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(RustEmbed, Clone)]
#[folder = "static/"]
pub struct Static;

#[derive(RustEmbed, Clone)]
#[folder = ".well-known/"]
pub struct WellKnown;
