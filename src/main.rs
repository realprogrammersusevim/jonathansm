mod routes;
use routes::{about, contact, main_page, post, Static, WellKnown};

use std::env;

use askama::Template;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use sqlx::{sqlite::SqlitePool, Pool, Sqlite};

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

#[derive(Debug, Clone)]
pub struct Commit {
    pub id: String,
    pub date: String,
    pub subject: String,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Template)]
#[template(path = "post.html")]
pub struct FinalPost {
    pub id: String,
    pub title: String,
    pub date: String,
    pub content: String,
    pub commits: Option<Vec<Commit>>,
}

async fn fetch_post(id: &str, app: State<AppState>) -> Result<Post, sqlx::Error> {
    sqlx::query_as!(
        Post,
        r#"
        SELECT id, title, date, content, commits
        FROM posts
        WHERE id = ?
        "#,
        id
    )
    .fetch_one(&app.pool)
    .await
}

async fn fetch_special(id: &str, app: State<AppState>) -> Result<Post, sqlx::Error> {
    sqlx::query_as!(
        Post,
        r#"
        SELECT id, title, date, content, commits
        FROM special
        WHERE id = ?
        "#,
        id
    )
    .fetch_one(&app.pool)
    .await
}

pub enum Table {
    Posts,
    Special,
}

pub async fn get_final_post(
    id: &str,
    table: Table,
    app: State<AppState>,
) -> Result<FinalPost, sqlx::Error> {
    let post = match table {
        Table::Posts => fetch_post(id, app.clone())
            .await
            .expect("Failed to fetch post"),
        Table::Special => fetch_special(id, app.clone())
            .await
            .expect("Failed to fetch special post"),
    };

    let final_post = match Some(post.commits) {
        Some(commits) => {
            // The commit ids for the post are stored as a whitespace-separated string
            let commits_clone = commits.clone().unwrap();
            let commit_ids: Vec<&str> = commits_clone.split_whitespace().collect();
            let mut commits = Vec::new();
            for commit_id in commit_ids {
                let commit = sqlx::query_as!(
                    Commit,
                    r#"
                    SELECT id, date, subject, body
                    FROM commits
                    WHERE id = ?
                    "#,
                    commit_id
                )
                .fetch_one(&app.pool)
                .await
                .unwrap();

                commits.push(commit);
            }

            FinalPost {
                id: post.id,
                title: post.title,
                date: post.date,
                content: post.content,
                commits: Some(commits),
            }
        }
        None => FinalPost {
            id: post.id,
            title: post.title,
            date: post.date,
            content: post.content,
            commits: None,
        },
    };

    Ok(final_post)
}

struct Post {
    id: String,
    title: String,
    date: String,
    content: String,
    commits: Option<String>,
}

#[derive(Debug, Clone)]
struct PostSummary {
    id: String,
    title: String,
    date: String,
}

#[derive(Debug, Clone, Template)]
#[template(path = "index.html")]
struct MainPage {
    title: String,
    posts: Vec<PostSummary>,
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
        SELECT id, title, date, content, commits
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
