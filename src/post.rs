use crate::{AppState, TEMPLATES};

use axum::{
    extract::State,
    response::{Html, IntoResponse, Response},
};
use serde::Serialize;
use tera::Context;

#[derive(Debug, Clone, Serialize)]
pub struct Commit {
    pub id: String,
    pub date: String,
    pub subject: String,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Copy, sqlx::Type, Serialize)]
#[sqlx(type_name = "TEXT")]
pub enum ContentType {
    Post,
    Link,
    Quote,
}

impl From<String> for ContentType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "post" => ContentType::Post,
            "link" => ContentType::Link,
            "quote" => ContentType::Quote,
            _ => ContentType::Post,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueryPost {
    pub id: String,
    pub content_type: ContentType,
    pub title: Option<String>,
    pub link: Option<String>,
    pub via: Option<String>,
    pub quote_author: Option<String>,
    pub date: String,
    pub content: String,
    pub commits: Option<String>,
}

impl QueryPost {
    pub async fn fetch(id: &str, app: State<AppState>) -> Result<QueryPost, sqlx::Error> {
        sqlx::query_as!(
            QueryPost,
            r#"
        SELECT *
        FROM posts
        WHERE id = ? AND content_type != 'special'
        "#,
            id,
        )
        .fetch_one(&app.pool)
        .await
    }

    pub async fn fetch_special(id: &str, app: State<AppState>) -> Result<QueryPost, sqlx::Error> {
        sqlx::query_as!(
            QueryPost,
            r#"
        SELECT *
        FROM posts
        WHERE id = ? AND content_type = 'special'
        "#,
            id
        )
        .fetch_one(&app.pool)
        .await
    }

    pub async fn into_post(self, app: State<AppState>) -> Post {
        let commits = match self.commits {
            Some(commits) => {
                // The commit ids for the post are stored as a whitespace-separated string
                let commits_clone = commits.clone();
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

                Some(commits)
            }
            None => None,
        };

        Post {
            id: self.id,
            title: self.title,
            content_type: self.content_type,
            link: self.link,
            via: self.via,
            quote_author: self.quote_author,
            date: self.date,
            content: self.content,
            real_commits: commits,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Post {
    pub id: String,
    pub content_type: ContentType,
    pub title: Option<String>,
    pub link: Option<String>,
    pub via: Option<String>,
    pub quote_author: Option<String>,
    pub date: String,
    pub content: String,
    pub real_commits: Option<Vec<Commit>>,
}

impl IntoResponse for Post {
    fn into_response(self) -> Response {
        let mut context = Context::new();
        context.insert("post", &self);
        let rendered = TEMPLATES
            .render("post.html", &context)
            .expect("Failed to render template");
        Html(rendered).into_response()
    }
}
