mod app;
mod db;
mod post;
mod routes;
mod rss;
mod services;

use crate::app::AppState;
use crate::routes::{
    about, contact, get_image, main_page, post as post_detail, posts_index, search, sitemap,
    switch_db, Static, WellKnown,
};
use crate::rss::feed;
use std::{env, path::PathBuf};

use axum::{
    extract::{MatchedPath, Request},
    routing::{get, post},
    Router,
};
use rusqlite::ffi::sqlite3_auto_extension;
use sqlite_vec::sqlite3_vec_init;
use tower_http::trace::TraceLayer;
use tracing::info_span;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    unsafe {
        #[allow(clippy::missing_transmute_annotations)]
        sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
    }
    let db_path = PathBuf::from(env::var("DATABASE_URL").expect("No DATABASE_URL set"));
    let initial_pool = crate::db::init_pool(&db_path).expect("Failed to create initial DB pool");
    let db_handles = crate::db::DbHandles::new(initial_pool, db_path.clone());
    let state = AppState::new(db_handles.clone());

    let static_files = axum_embed::ServeEmbed::<Static>::with_parameters(
        None,
        axum_embed::FallbackBehavior::NotFound,
        None,
    );

    let well_known = axum_embed::ServeEmbed::<WellKnown>::with_parameters(
        Some("404.html".to_owned()),
        axum_embed::FallbackBehavior::NotFound,
        None,
    );

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!(
                    "{}=debug,tower_http=debug,axum::rejection=trace",
                    env!("CARGO_PKG_NAME")
                )
                .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .route("/", get(main_page))
        .route("/sitemap.xml", get(sitemap))
        .route("/search", get(search))
        .route("/posts", get(posts_index))
        .route("/about", get(about))
        .route("/contact", get(contact))
        .route("/post/:id", get(post_detail))
        .route("/feed", get(feed))
        .route("/admin/switch_db/:filename", post(switch_db))
        .route("/images/:id", get(get_image))
        .nest_service("/static", static_files)
        .nest_service("/.well-known", well_known)
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                let matched_path = request
                    .extensions()
                    .get::<MatchedPath>()
                    .map(MatchedPath::as_str);
                info_span!("http_request", method = ?request.method(), matched_path)
            }),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!(
        "0.0.0.0:{}",
        &env::var("PORT").expect("No PORT set")
    ))
    .await
    .expect("Failed to bind port");
    axum::serve(listener, app).await.unwrap();
}
