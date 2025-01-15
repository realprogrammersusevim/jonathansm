use crate::AppState;

use askama::Template;
use axum::extract::State;

#[derive(Debug, Clone)]
pub struct Commit {
    pub id: String,
    pub date: String,
    pub subject: String,
    pub body: Option<String>,
}

#[derive(Debug, Clone)]
pub struct QueryPost {
    pub id: String,
    pub title: Option<String>,
    pub link: Option<String>,
    pub via: Option<String>,
    pub date: String,
    pub content: String,
    pub commits: Option<String>,
}

impl QueryPost {
    pub async fn fetch(id: &str, app: State<AppState>) -> Result<QueryPost, sqlx::Error> {
        sqlx::query_as!(
            QueryPost,
            r#"
        SELECT id, title, link, via, date, content, commits
        FROM posts
        WHERE id = ?
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
        SELECT id, title, link, via, date, content, commits
        FROM special
        WHERE id = ?
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
            link: self.link,
            via: self.via,
            date: self.date,
            content: self.content,
            real_commits: commits,
        }
    }
}

#[derive(Debug, Clone, Template)]
#[template(path = "post.html")]
pub struct Post {
    pub id: String,
    pub title: Option<String>,
    pub link: Option<String>,
    pub via: Option<String>,
    pub date: String,
    pub content: String,
    pub real_commits: Option<Vec<Commit>>,
}

impl Post {}
