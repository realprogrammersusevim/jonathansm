mod post;
mod routes;
mod rss;

use post::{Post, QueryPost};
use routes::{about, contact, main_page, post, posts_index, Static, WellKnown};
use rss::feed;

use std::env;

use axum::{
    extract::{MatchedPath, Request},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use lazy_static::lazy_static;
use rust_embed::RustEmbed;
use sqlx::{sqlite::SqlitePool, Pool, Sqlite};
use tera::{Context, Tera};
use tower_http::trace::TraceLayer;
use tracing::info_span;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Clone)]
pub struct AppState {
    pub pool: Pool<Sqlite>,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let pool = SqlitePool::connect(&env::var("DATABASE_URL").expect("No DATABASE_URL set"))
        .await
        .expect("Couldn't connect to database");
    let state = AppState { pool };

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
        .route("/posts", get(posts_index))
        .route("/about", get(about))
        .route("/contact", get(contact))
        .route("/post/:id", get(post))
        .route("/feed", get(feed))
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

#[derive(Debug, Clone)]
struct MainPage {
    title: String,
    posts: Vec<Post>,
}

impl IntoResponse for MainPage {
    fn into_response(self) -> Response {
        let mut context = Context::new();
        context.insert("title", &self.title);
        context.insert("posts", &self.posts);
        let rendered = TEMPLATES
            .render("index.html", &context)
            .expect("Failed to render template");
        Html(rendered).into_response()
    }
}

#[derive(Debug, Clone)]
struct PostsPage {
    title: String,
    posts: Vec<Post>,
    current_page: usize,
    total_pages: usize,
}

impl IntoResponse for PostsPage {
    fn into_response(self) -> Response {
        let mut context = Context::new();
        context.insert("title", &self.title);
        context.insert("posts", &self.posts);
        context.insert("current_page", &self.current_page);
        context.insert("total_pages", &self.total_pages);
        let rendered = TEMPLATES
            .render("posts.html", &context)
            .expect("Failed to render template");
        Html(rendered).into_response()
    }
}

#[derive(RustEmbed)]
#[folder = "templates/"]
struct Templates;

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        let mut tera = Tera::default();

        let templates_to_add = Templates::iter().map(|path| {
            let contents = Templates::get(path.as_ref()).unwrap().data;
            let content_string = String::from_utf8(contents.into_owned()).unwrap();
            (path, content_string)
        });

        tera.add_raw_templates(templates_to_add)
            .expect("Failed to add raw templates");

        return tera;
    };
}
