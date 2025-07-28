use crate::{app::AppState, services::search_query::SearchQuery};
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::DateTime;
use rust_embed::RustEmbed;
use serde::Deserialize;
use std::fmt::Write;
use tera::Context;
use tokio::fs;

async fn update_database_url_env(new_path: &std::path::Path) -> anyhow::Result<()> {
    const ENV_FILE: &str = ".env";
    let env_path = std::path::Path::new(ENV_FILE);

    // Read existing contents (if any)
    let contents = if env_path.exists() {
        fs::read_to_string(env_path).await?
    } else {
        String::new()
    };

    // Split into lines, update or append DATABASE_URL
    let mut lines: Vec<String> = contents
        .lines()
        .map(std::string::ToString::to_string)
        .collect();
    let mut updated = false;
    for line in &mut lines {
        if line.starts_with("DATABASE_URL=") {
            *line = format!("DATABASE_URL={}", new_path.display());
            updated = true;
            break;
        }
    }
    if !updated {
        lines.push(format!("DATABASE_URL={}", new_path.display()));
    }

    // Write back
    fs::write(env_path, lines.join("\n")).await?;
    Ok(())
}

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

pub async fn get_image(Path(id): Path<String>, state: State<AppState>) -> impl IntoResponse {
    if id.is_empty() || id.len() > 100 {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let filename = format!("images/{id}");
    match state.image_service.get_image_data(&filename).await {
        Ok(Some(data)) => {
            let content_type = match id.split('.').next_back() {
                Some("png") => "image/png",
                Some("jpg" | "jpeg") => "image/jpeg",
                Some("gif") => "image/gif",
                Some("webp") => "image/webp",
                Some("svg") => "image/svg+xml",
                _ => "application/octet-stream",
            };

            (StatusCode::OK, [(header::CONTENT_TYPE, content_type)], data).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
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

            let total_pages = total.div_ceil(per_page);
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

pub async fn switch_db(
    Path(filename): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if filename.is_empty() || filename.contains('/') || filename.len() > 200 {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let new_path = std::path::PathBuf::from(format!("./{filename}"));

    if !new_path.exists() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let pool = match crate::db::init_pool(&new_path) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Failed to init new pool: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    state.db.swap_primary(pool, new_path.clone()).await;

    // Persist new DB path so the next server restart uses it
    if let Err(e) = update_database_url_env(&new_path).await {
        tracing::error!("Failed to update .env file: {}", e);
    }

    (StatusCode::OK, format!("Database switched to {filename}")).into_response()
}

#[derive(RustEmbed, Clone)]
#[folder = "static/"]
pub struct Static;

#[derive(RustEmbed, Clone)]
#[folder = ".well-known/"]
pub struct WellKnown;

pub async fn sitemap(state: State<AppState>) -> Response {
    const BASE_URL: &str = "https://jonathansm.com";

    let mut entries = String::new();

    // Add static pages
    let static_pages = [
        "https://jonathansm.com/",
        "https://jonathansm.com/about",
        "https://jonathansm.com/contact",
        "https://jonathansm.com/posts",
        "https://jonathansm.com/feed",
    ];

    for url in &static_pages {
        write!(entries, "<url><loc>{url}</loc></url>").unwrap();
    }

    // Add blog posts with last modified dates
    if let Ok(post_entries) = state.post_service.get_all_post_urls().await {
        for (id, date_str) in post_entries {
            let url = format!("{BASE_URL}/post/{id}");
            if let Ok(date) = DateTime::parse_from_rfc3339(&date_str) {
                let w3c_date = date.to_rfc3339();
                write!(
                    entries,
                    r"<url><loc>{url}</loc><lastmod>{w3c_date}</lastmod></url>"
                )
                .unwrap();
            } else {
                write!(entries, "<url><loc>{url}</loc></url>").unwrap();
            }
        }
    }

    let sitemap = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
            {entries}
        </urlset>"#
    );

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/xml".parse().unwrap());

    (StatusCode::OK, headers, sitemap).into_response()
}
