use std::env;

use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use sqlx::{sqlite::SqlitePool, Pool, Sqlite};

#[derive(Debug, Clone)]
struct AppState {
    pool: Pool<Sqlite>,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let pool = SqlitePool::connect(&env::var("DATABASE_URL").expect("No DATABASE_URL set"))
        .await
        .unwrap();
    let state = AppState { pool };

    let app = Router::new()
        .route("/", get(main_page))
        .route("/post/:id", get(post))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!(
        "0.0.0.0:{}",
        &env::var("PORT").expect("No PORT set")
    ))
    .await
    .unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Clone, Template)]
#[template(path = "post.html")]
struct Post {
    id: String,
    title: String,
    date: String,
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
        SELECT id, title, date, content
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

async fn post(Path(id): Path<String>, app: State<AppState>) -> impl IntoResponse {
    let post = sqlx::query_as!(
        Post,
        r#"
        SELECT id, title, date, content
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
