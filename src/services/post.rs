use crate::post::{Commit, Post, QueryPost};
use anyhow::{Context, Result};
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PostService {
    pool: Pool<Sqlite>,
}

impl PostService {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    pub async fn get_main_posts(&self) -> Result<Vec<Post>> {
        let queried = sqlx::query_as!(
            QueryPost,
            r#"
            SELECT *
            FROM posts
            WHERE content_type != 'special'
            ORDER BY date DESC
            LIMIT 5
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch main posts")?;

        self.bulk_convert_to_posts(queried).await
    }

    pub async fn get_paginated_posts(&self, page: usize) -> Result<(Vec<Post>, usize, usize)> {
        const POSTS_PER_PAGE: i64 = 10;
        let offset = (page as i64 - 1) * POSTS_PER_PAGE;

        let queried = sqlx::query_as!(
            QueryPost,
            r#"
            SELECT *
            FROM posts
            WHERE content_type != 'special'
            ORDER BY date DESC
            LIMIT ? OFFSET ?
            "#,
            POSTS_PER_PAGE,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch paginated posts")?;

        let total_posts: i64 =
            sqlx::query!("SELECT COUNT(*) as count FROM posts WHERE content_type != 'special'")
                .fetch_one(&self.pool)
                .await
                .context("Failed to count posts")?
                .count;

        let total_pages = (total_posts as f64 / POSTS_PER_PAGE as f64).ceil() as usize;
        let posts = self.bulk_convert_to_posts(queried).await?;

        Ok((posts, page, total_pages))
    }

    pub async fn get_post(&self, id: &str) -> Result<Post> {
        let query = sqlx::query_as!(
            QueryPost,
            r#"
            SELECT *
            FROM posts
            WHERE id = ? AND content_type != 'special'
            "#,
            id,
        )
        .fetch_one(&self.pool)
        .await
        .with_context(|| format!("Failed to fetch post {id}"))?;

        self.convert_to_post(query).await
    }

    pub async fn get_special_page(&self, id: &str) -> Result<Post> {
        let query = sqlx::query_as!(
            QueryPost,
            r#"
            SELECT *
            FROM posts
            WHERE id = ? AND content_type = 'special'
            "#,
            id
        )
        .fetch_one(&self.pool)
        .await
        .with_context(|| format!("Failed to fetch special page {id}"))?;

        self.convert_to_post(query).await
    }

    pub async fn get_rss_entries(&self) -> Result<Vec<QueryPost>> {
        sqlx::query_as!(
            QueryPost,
            r#"
            SELECT *
            FROM posts
            WHERE content_type != 'special'
            ORDER BY date DESC
            LIMIT 20
            "#,
        )
        .fetch_all(&self.pool)
        .await
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
            .collect();

        let mut commits_map = HashMap::new();
        if !all_commit_ids.is_empty() {
            let query_str = format!(
                "SELECT id, date, subject, body FROM commits WHERE id IN ({})",
                vec!["?"; all_commit_ids.len()].join(", ")
            );

            let mut query = sqlx::query_as::<_, Commit>(&query_str);
            for id in &all_commit_ids {
                query = query.bind(id);
            }

            for commit in query
                .fetch_all(&self.pool)
                .await
                .context("Failed to bulk fetch commits")?
            {
                commits_map.insert(commit.id.clone(), commit);
            }
        }

        let posts = query_posts
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
            .collect();

        Ok(posts)
    }
}
