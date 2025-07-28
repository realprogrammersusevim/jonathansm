use crate::db::DbHandles;
use crate::post::{Commit, ContentType, Post, SummaryPost};
use anyhow::{Context, Result};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task;
use tracing;

#[derive(Debug, Clone)]
pub struct PostService {
    db: Arc<DbHandles>,
}

impl PostService {
    pub fn new(db: Arc<DbHandles>) -> Self {
        Self { db }
    }

    async fn run_db_query<F, T>(&self, query_fn: F) -> Result<T>
    where
        F: FnOnce(r2d2::PooledConnection<SqliteConnectionManager>) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let pool = self.db.primary.load();
        task::spawn_blocking(move || {
            let conn = pool.get()?;
            query_fn(conn)
        })
        .await
        .context("Failed to join blocking task")?
    }

    pub fn row_to_post(row: &rusqlite::Row) -> rusqlite::Result<Post> {
        let id: String = row.get("id")?;
        let content_type_str: String = row.get("content_type")?;
        let content_type = ContentType::from(content_type_str);
        let title: Option<String> = row.get("title")?;
        let link: Option<String> = row.get("link")?;
        let via: Option<String> = row.get("via")?;
        let quote_author: Option<String> = row.get("quote_author")?;
        let date: String = row.get("date")?;
        let content: String = row.get("content")?;
        let commits_str: Option<String> = row.get("commits")?;
        let commits = commits_str.and_then(|s| serde_json::from_str(&s).ok());

        let tags_str: Option<String> = row.get("tags")?;
        let tags = tags_str.and_then(|s| serde_json::from_str(&s).ok());

        Ok(Post {
            id,
            content_type,
            title,
            link,
            via,
            quote_author,
            date,
            last_updated: None,
            content,
            commits,
            tags,
            real_commits: None,
            related_posts: None,
        })
    }

    pub fn row_to_summary_post(row: &rusqlite::Row) -> rusqlite::Result<SummaryPost> {
        Ok(SummaryPost {
            id: row.get("id")?,
            content_type: ContentType::from(row.get::<_, String>("content_type")?),
            title: row.get("title")?,
            link: row.get("link")?,
            via: row.get("via")?,
            quote_author: row.get("quote_author")?,
            date: row.get("date")?,
        })
    }

    pub async fn get_main_posts(&self) -> Result<Vec<Post>> {
        let queried = self
            .run_db_query(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT * FROM posts WHERE content_type != 'special' ORDER BY date DESC LIMIT 5",
                )?;
                let iter = stmt.query_map([], Self::row_to_post)?;
                iter.collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(anyhow::Error::from)
            })
            .await?;

        self.bulk_convert_to_posts(queried).await
    }

    pub async fn get_paginated_posts(&self, page: usize) -> Result<(Vec<Post>, usize, usize)> {
        const POSTS_PER_PAGE: i64 = 10;
        #[allow(clippy::cast_possible_wrap)]
        let offset = (page as i64 - 1) * POSTS_PER_PAGE;

        let queried = self
            .run_db_query(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT * FROM posts WHERE content_type != 'special' ORDER BY date DESC LIMIT ? OFFSET ?",
                )?;
                let iter = stmt.query_map(params![POSTS_PER_PAGE, offset], Self::row_to_post)?;
                iter.collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(anyhow::Error::from)
            })
            .await?;

        let total_posts: i64 = self
            .run_db_query(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM posts WHERE content_type != 'special'",
                    [],
                    |row| row.get(0),
                )
                .map_err(anyhow::Error::from)
            })
            .await?;

        let total_pages = usize::try_from((total_posts + POSTS_PER_PAGE - 1) / POSTS_PER_PAGE)
            .context("Total pages exceeds usize range")?;
        let posts = self.bulk_convert_to_posts(queried).await?;

        Ok((posts, page, total_pages))
    }

    async fn get_post_by_id_internal(
        &self,
        id: &str,
        content_type_sql: &str,
        not_found_msg: &str,
    ) -> Result<Post> {
        let id_owned = id.to_owned();
        let query_sql = format!("SELECT * FROM posts WHERE id = ? AND {content_type_sql}");
        let not_found_msg_owned = not_found_msg.to_owned();

        self.run_db_query(move |conn| {
            conn.query_row(&query_sql, [&id_owned], Self::row_to_post)
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        anyhow::anyhow!("{}: {}", not_found_msg_owned, id_owned)
                    }
                    _ => e.into(),
                })
        })
        .await
    }

    pub async fn get_post(&self, id: &str) -> Result<Post> {
        let query = self
            .get_post_by_id_internal(id, "content_type != 'special'", "Post not found")
            .await?;
        let mut post = self.convert_to_post(query).await?;

        match self.get_related_posts(&post.id).await {
            Ok(related_posts) => {
                if !related_posts.is_empty() {
                    post.related_posts = Some(related_posts);
                }
            }
            Err(e) => {
                tracing::error!("Failed to get related posts for {}: {}", post.id, e);
            }
        }

        Ok(post)
    }

    pub async fn get_special_page(&self, id: &str) -> Result<Post> {
        let query = self
            .get_post_by_id_internal(id, "content_type = 'special'", "Special page not found")
            .await?;
        self.convert_to_post(query).await
    }

    async fn get_related_posts(&self, id: &str) -> Result<Vec<SummaryPost>> {
        let id_for_blocking = id.to_owned();

        let related_ids: Vec<String> = self
            .run_db_query(move |conn| {
                let embedding: Vec<u8> = match conn.query_row(
                    "SELECT embedding FROM post_embeddings WHERE id = ?",
                    [&id_for_blocking],
                    |row| row.get(0),
                ) {
                    Ok(embedding) => embedding,
                    Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(vec![]),
                    Err(e) => return Err(e.into()),
                };

                let mut stmt = conn.prepare(
                    r"
                SELECT id FROM post_embeddings
                WHERE embedding MATCH ?1 AND id != ?2
                ORDER BY distance
                LIMIT 3
                ",
                )?;
                let ids_iter =
                    stmt.query_map(params![embedding, &id_for_blocking], |row| row.get(0))?;

                ids_iter
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(anyhow::Error::from)
            })
            .await?;

        if related_ids.is_empty() {
            return Ok(vec![]);
        }

        let related_ids_for_query = related_ids.clone();
        let posts = self
            .run_db_query(move |conn| {
                let placeholders = related_ids_for_query
                    .iter()
                    .map(|_| "?")
                    .collect::<Vec<_>>()
                    .join(",");
                let sql = format!("SELECT id, content_type, title, link, via, quote_author, date FROM posts WHERE id IN ({placeholders})");
                let mut stmt = conn.prepare(&sql)?;

                let post_iter = stmt.query_map(
                    rusqlite::params_from_iter(related_ids_for_query.iter()),
                    Self::row_to_summary_post,
                )?;

                post_iter
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(anyhow::Error::from)
            })
            .await?;

        let mut posts_map = posts
            .into_iter()
            .map(|p| (p.id.clone(), p))
            .collect::<HashMap<_, _>>();

        let ordered_posts = related_ids
            .into_iter()
            .filter_map(|id| posts_map.remove(&id))
            .collect();

        Ok(ordered_posts)
    }

    pub async fn get_rss_entries(&self) -> Result<Vec<Post>> {
        let posts = self
            .run_db_query(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT * FROM posts WHERE content_type != 'special' ORDER BY date DESC LIMIT 20",
                )?;
                let iter = stmt.query_map([], Self::row_to_post)?;
                iter.collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(anyhow::Error::from)
            })
            .await?;

        self.bulk_convert_to_posts(posts).await
    }

    async fn convert_to_post(&self, post: Post) -> Result<Post> {
        self.bulk_convert_to_posts(vec![post])
            .await
            .map(|mut v| v.remove(0))
    }

    pub async fn get_all_post_urls(&self) -> Result<Vec<(String, String)>> {
        self.run_db_query(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, date FROM posts WHERE content_type != 'special' ORDER BY date DESC",
            )?;
            let iter = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
            iter.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(anyhow::Error::from)
        })
        .await
    }

    pub async fn bulk_convert_to_posts(&self, mut posts: Vec<Post>) -> Result<Vec<Post>> {
        let all_commit_ids: Vec<_> = posts
            .iter()
            .filter_map(|post| post.commits.as_ref())
            .flatten()
            .cloned()
            .collect();

        let commits_map = if all_commit_ids.is_empty() {
            HashMap::new()
        } else {
            self.run_db_query(move |conn| {
                let placeholders = all_commit_ids
                    .iter()
                    .map(|_| "?")
                    .collect::<Vec<_>>()
                    .join(",");
                let sql = format!(
                    "SELECT id, date, subject, body FROM commits WHERE id IN ({placeholders})"
                );

                let mut stmt = conn.prepare(&sql)?;
                let commits_iter = stmt.query_map(
                    rusqlite::params_from_iter(
                        all_commit_ids
                            .iter()
                            .map(|s| s as &dyn rusqlite::types::ToSql),
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
            .await?
        };

        for post in &mut posts {
            if let Some(ids) = &post.commits {
                let real_commits: Vec<Commit> = ids
                    .iter()
                    .filter_map(|id| commits_map.get(id))
                    .cloned()
                    .collect();

                // If the post has commits, set `last_updated` to the date of the first (most
                // recent) commit
                if !real_commits.is_empty() {
                    if let Some(first_id) = ids.first() {
                        if let Some(first_commit) = real_commits.iter().find(|c| &c.id == first_id)
                        {
                            post.last_updated = Some(first_commit.date.clone());
                        }
                    }
                }

                post.real_commits = Some(real_commits);
            }
        }

        Ok(posts)
    }
}
