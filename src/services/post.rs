use crate::post::{Commit, ContentType, Post, QueryPost};
use anyhow::{Context, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use std::collections::HashMap;
use tokio::task;

#[derive(Debug, Clone)]
pub struct PostService {
    pool: Pool<SqliteConnectionManager>,
}

impl PostService {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn row_to_query_post(row: &rusqlite::Row) -> rusqlite::Result<QueryPost> {
        let id: String = row.get("id")?;
        let content_type_str: String = row.get("content_type")?;
        let content_type = ContentType::from(content_type_str);
        let title: Option<String> = row.get("title")?;
        let link: Option<String> = row.get("link")?;
        let via: Option<String> = row.get("via")?;
        let quote_author: Option<String> = row.get("quote_author")?;
        let date: String = row.get("date")?;
        let content: String = row.get("content")?;
        let commits: Option<String> = row.get("commits")?;

        Ok(QueryPost {
            id,
            content_type,
            title,
            link,
            via,
            quote_author,
            date,
            content,
            commits,
        })
    }

    pub async fn get_main_posts(&self) -> Result<Vec<Post>> {
        let pool = self.pool.clone();
        let queried = task::spawn_blocking(move || {
            let conn = pool.get()?;
            let mut stmt = conn.prepare(
                "SELECT * FROM posts WHERE content_type != 'special' ORDER BY date DESC LIMIT 5",
            )?;
            let iter = stmt.query_map([], Self::row_to_query_post)?;
            let mut result = Vec::new();
            for post in iter {
                result.push(post?);
            }
            anyhow::Result::<_, anyhow::Error>::Ok(result)
        })
        .await
        .context("Failed to join blocking task")??;

        self.bulk_convert_to_posts(queried).await
    }

    pub async fn get_paginated_posts(&self, page: usize) -> Result<(Vec<Post>, usize, usize)> {
        const POSTS_PER_PAGE: i64 = 10;
        let offset = (page as i64 - 1) * POSTS_PER_PAGE;

        let pool = self.pool.clone();
        let queried = task::spawn_blocking(move || {
            let conn = pool.get()?;
            let mut stmt = conn.prepare(
                "SELECT * FROM posts WHERE content_type != 'special' ORDER BY date DESC LIMIT ? OFFSET ?",
            )?;
            let iter = stmt.query_map(params![POSTS_PER_PAGE, offset], Self::row_to_query_post)?;
            let mut result = Vec::new();
            for post in iter {
                result.push(post?);
            }
            anyhow::Result::<_, anyhow::Error>::Ok(result)
        })
        .await
        .context("Failed to join blocking task")??;

        let total_posts: i64 = {
            let pool = self.pool.clone();
            task::spawn_blocking(move || {
                let conn = pool.get()?;
                let count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM posts WHERE content_type != 'special'",
                    [],
                    |row| row.get(0),
                )?;
                anyhow::Result::<_>::Ok(count)
            })
            .await?
            .context("Failed to count posts")?
        };

        let total_pages = (total_posts as f64 / POSTS_PER_PAGE as f64).ceil() as usize;
        let posts = self.bulk_convert_to_posts(queried).await?;

        Ok((posts, page, total_pages))
    }

    pub async fn get_post(&self, id: &str) -> Result<Post> {
        let id_clone = id.to_owned();
        let pool = self.pool.clone();
        let query = task::spawn_blocking(move || {
            let conn = pool.get()?;
            let result = conn.query_row(
                "SELECT * FROM posts WHERE id = ? AND content_type != 'special'",
                [&id_clone],
                Self::row_to_query_post,
            )?;
            anyhow::Result::<_>::Ok(result)
        })
        .await
        .context("Failed to join blocking task")?
        .context(format!("Failed to fetch post {}", id))?;

        self.convert_to_post(query).await
    }

    pub async fn get_special_page(&self, id: &str) -> Result<Post> {
        let id_clone = id.to_owned();
        let pool = self.pool.clone();
        let query = task::spawn_blocking(move || {
            let conn = pool.get()?;
            let result = conn.query_row(
                "SELECT * FROM posts WHERE id = ? AND content_type = 'special'",
                [&id_clone],
                Self::row_to_query_post,
            )?;
            anyhow::Result::<_>::Ok(result)
        })
        .await
        .context("Failed to join blocking task")?
        .context(format!("Failed to fetch special page {}", id))?;

        self.convert_to_post(query).await
    }

    pub async fn get_rss_entries(&self) -> Result<Vec<QueryPost>> {
        let pool = self.pool.clone();
        task::spawn_blocking(move || {
            let conn = pool.get()?;
            let mut stmt = conn.prepare(
                "SELECT * FROM posts WHERE content_type != 'special' ORDER BY date DESC LIMIT 20",
            )?;
            let iter = stmt.query_map([], Self::row_to_query_post)?;
            let mut result = Vec::new();
            for post in iter {
                result.push(post?);
            }
            anyhow::Result::<_>::Ok(result)
        })
        .await
        .context("Failed to join blocking task")?
        .context("Failed to fetch RSS entries")
    }

    async fn convert_to_post(&self, query: QueryPost) -> Result<Post> {
        self.bulk_convert_to_posts(vec![query])
            .await
            .map(|mut v| v.remove(0))
    }

    async fn bulk_convert_to_posts(&self, query_posts: Vec<QueryPost>) -> Result<Vec<Post>> {
        let all_commit_ids: Vec<_> = query_posts
            .iter()
            .filter_map(|post| post.commits.as_ref())
            .flat_map(|commits| commits.split_whitespace())
            .map(|s| s.to_owned())
            .collect();

        let commits_map = if !all_commit_ids.is_empty() {
            let pool = self.pool.clone();
            let ids = all_commit_ids.clone();
            task::spawn_blocking(move || {
                let conn = pool.get()?;
                let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                let sql = format!(
                    "SELECT id, date, subject, body FROM commits WHERE id IN ({})",
                    placeholders
                );

                let mut stmt = conn.prepare(&sql)?;
                let commits_iter = stmt.query_map(
                    rusqlite::params_from_iter(
                        ids.iter().map(|s| s as &dyn rusqlite::types::ToSql),
                    ),
                    |row| {
                        Ok(Commit {
                            id: row.get(0)?,
                            date: row.get(1)?,
                            subject: row.get(2)?,
                            body: row.get(3)?,
                        })
                    },
                )?;

                let mut map = HashMap::new();
                for commit in commits_iter {
                    let commit = commit?;
                    map.insert(commit.id.clone(), commit);
                }
                anyhow::Result::<_>::Ok(map)
            })
            .await
            .context("Failed to join blocking task")??
        } else {
            HashMap::new()
        };

        Ok(query_posts
            .into_iter()
            .map(|post| {
                let real_commits = post.commits.as_ref().map(|ids| {
                    ids.split_whitespace()
                        .filter_map(|id| commits_map.get(id).cloned())
                        .collect()
                });

                Post {
                    id: post.id,
                    content_type: post.content_type,
                    title: post.title,
                    link: post.link,
                    via: post.via,
                    quote_author: post.quote_author,
                    date: post.date,
                    content: post.content,
                    real_commits,
                }
            })
            .collect())
    }
}
