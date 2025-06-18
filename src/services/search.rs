use super::{post::PostService, search_query::SearchQuery};
use anyhow::Context;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Row;
use tokio::task;

#[derive(Clone, Debug)]
pub struct SearchService {
    pool: Pool<SqliteConnectionManager>,
}

impl SearchService {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn row_to_post(row: &Row) -> rusqlite::Result<crate::post::Post> {
        PostService::row_to_post(row)
    }

    pub async fn search(
        &self,
        query: &SearchQuery,
        page: usize,
        per_page: usize,
    ) -> anyhow::Result<(Vec<crate::post::Post>, usize)> {
        // Create a full clone of the query data to move into the thread
        let owned_query = SearchQuery {
            text_query: query.text_query.clone(),
            tags: query.tags.clone(),
            from_date: query.from_date.clone(),
            to_date: query.to_date.clone(),
        };
        let offset = (page - 1) * per_page;
        let pool = self.pool.clone();

        let (posts, total) = task::spawn_blocking(move || {
            let conn = pool.get()?;
            let base_query = if owned_query.text_query.is_empty() {
                "FROM posts".to_string()
            } else {
                "FROM posts INNER JOIN posts_fts ON posts.rowid = posts_fts.rowid".to_string()
            };

            let mut conditions = vec![];
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

            if !owned_query.text_query.is_empty() {
                conditions.push("posts_fts MATCH ?".to_string());
                params.push(Box::new(owned_query.text_query.clone()));
            }

            for tag in &owned_query.tags {
                conditions.push("EXISTS (SELECT 1 FROM json_each(posts.tags) WHERE value = ?)".to_string());
                params.push(Box::new(tag));
            }

            if let Some(date) = &owned_query.from_date {
                conditions.push("posts.date >= ?".to_string());
                params.push(Box::new(date));
            }
            if let Some(date) = &owned_query.to_date {
                conditions.push("posts.date <= ?".to_string());
                params.push(Box::new(date));
            }

            let where_clause = if !conditions.is_empty() {
                format!("WHERE {}", conditions.join(" AND "))
            } else {
                "".to_string()
            };

            let order_clause = if owned_query.text_query.is_empty() {
                "ORDER BY date DESC".to_string()
            } else {
                "ORDER BY rank".to_string()
            };

            // Prepare count query first (borrows params immutably)
            let count_query = format!(
                "SELECT COUNT(*)
                {}
                {}",
                base_query, where_clause
            );
            let total: i64 = conn.query_row(
                &count_query,
                rusqlite::params_from_iter(params.iter().map(|p| &**p)),
                |r| r.get(0),
            )?;

            // Main query to fetch posts (takes ownership of params)
            let posts_query = format!(
                "SELECT posts.*
                {}
                {}
                {}
                LIMIT ? OFFSET ?",
                base_query, where_clause, order_clause
            );

            let mut stmt = conn.prepare(&posts_query)?;
            params.push(Box::new(per_page as i64));
            params.push(Box::new(offset as i64));

            // Execute query and collect results
            let iter = stmt.query_map(
                rusqlite::params_from_iter(params.iter().map(|p| &**p)),
                Self::row_to_post,
            )?;
            let mut posts = Vec::new();
            for post in iter {
                posts.push(post?);
            }

            Ok::<_, anyhow::Error>((posts, total as usize))
        })
        .await?
        .context("Search execution failed")?;

        let post_service = PostService::new(self.pool.clone());
        let posts_with_commits = post_service.bulk_convert_to_posts(posts).await?;

        Ok((posts_with_commits, total))
    }
}
