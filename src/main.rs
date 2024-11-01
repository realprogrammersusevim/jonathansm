use std::env;

use askama::Template;
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use rust_embed::RustEmbed;
use sqlx::{sqlite::SqlitePool, Pool, Sqlite};

#[derive(Debug, Clone)]
struct AppState {
    pool: Pool<Sqlite>,
}

#[derive(RustEmbed, Clone)]
#[folder = "static/"]
struct Static;

#[derive(RustEmbed, Clone)]
#[folder = ".well-known/"]
struct WellKnown;

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

    let app = Router::new()
        .route("/", get(main_page))
        .route("/about", get(about))
        .route("/contact", get(contact))
        .route("/post/:id", get(post))
        .route("/feed", get(feed))
        .nest_service("/static", static_files)
        .nest_service("/.well-known", well_known)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!(
        "0.0.0.0:{}",
        &env::var("PORT").expect("No PORT set")
    ))
    .await
    .expect("Failed to bind port");
    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Clone, Template)]
#[template(path = "post.html")]
struct Post {
    id: String,
    title: String,
    date: String,
    updated: Option<String>,
    content: String,
}

#[derive(Debug, Clone, Template)]
#[template(path = "index.html")]
struct MainPage {
    title: String,
    posts: Vec<Post>,
}

/// A wrapper type that we'll use to encapsulate HTML parsed by askama into valid HTML for axum to serve.
struct HtmlTemplate<T>(T);

/// Allows us to convert Askama HTML templates into valid HTML for axum to serve in the response.
impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        // Attempt to render the template with askama
        match self.0.render() {
            // If we're able to successfully parse and aggregate the template, serve it
            Ok(html) => Html(html).into_response(),
            // If we're not, return an error or some bit of fallback HTML
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {}", err),
            )
                .into_response(),
        }
    }
}

async fn main_page(app: State<AppState>) -> impl IntoResponse {
    let posts = sqlx::query_as!(
        Post,
        r#"
        SELECT id, title, date, content, updated
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

async fn get_special(id: &str, app: State<AppState>) -> Result<Post, sqlx::Error> {
    sqlx::query_as!(
        Post,
        r#"
        SELECT id, title, date, updated, content
        FROM special
        WHERE id = ?
        "#,
        id
    )
    .fetch_one(&app.pool)
    .await
}

async fn about(app: State<AppState>) -> impl IntoResponse {
    match get_special("about", app).await {
        Ok(about) => HtmlTemplate(about).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch about page",
        )
            .into_response(),
    }
}

async fn contact(app: State<AppState>) -> impl IntoResponse {
    match get_special("contact", app).await {
        Ok(contact) => HtmlTemplate(contact).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to fetch contact page",
        )
            .into_response(),
    }
}

async fn post(Path(id): Path<String>, app: State<AppState>) -> impl IntoResponse {
    let post = sqlx::query_as!(
        Post,
        r#"
        SELECT id, title, date, updated, content
        FROM posts
        WHERE id = ?
        "#,
        id
    )
    .fetch_one(&app.pool)
    .await;

    // If the post is found, render the post template
    // If the post is not found, return a 404
    match post {
        Ok(post) => HtmlTemplate(post).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Post not found").into_response(),
    }
}

struct RssEntry {
    title: String,
    link: String,
    content: String,
    pub_date: String,
    author: String,
    guid: String,
}

impl From<Post> for RssEntry {
    fn from(post: Post) -> Self {
        let full_url = format!("https://jonathansm.com/post/{}", post.id);
        Self {
            title: post.title,
            link: full_url.clone(),
            content: post.content,
            author: "Jonathan".to_string(),
            pub_date: post.date,
            guid: full_url,
        }
    }
}

impl RssEntry {
    fn to_xml(&self) -> String {
        format!(
            r#"
            <item>
                <title>{}</title>
                <link>{}</link>
                <content type="html">{}</content>
                <pubDate>{}</pubDate>
                <author>{}</author>
                <guid isPermalink="true">{}</guid>
            </item>
            "#,
            self.title, self.link, self.content, self.pub_date, self.author, self.guid
        )
        .trim()
        .to_string()
    }
}

async fn feed(app: State<AppState>) -> impl IntoResponse {
    let rss_items: String = sqlx::query_as!(
        Post,
        r#"
        SELECT id, title, date, content, updated
        FROM posts
        ORDER BY date DESC
        LIMIT 20
        "#,
    )
    .fetch_all(&app.pool)
    .await
    .unwrap()
    .into_iter()
    .map(|post| RssEntry::from(post).to_xml())
    .collect();

    let rss = format!(
        r#"
        <?xml version="1.0" encoding="UTF-8" ?>
        <rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
            <channel>
                <title>Jonathan's Blog</title>
                <link>https://jonathansm.com</link>
                <description>Jonathan's Blog</description>
                <language>en-us</language>
                <atom:link href="https://jonathansm.com/feed" rel="self" type="application/rss+xml" />
                {}
            </channel>
        </rss>
        "#,
        rss_items
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/atom+xml")],
        rss,
    )
}
