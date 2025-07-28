use super::{post::PostService, search_query::SearchQuery};
use crate::db::DbHandles;
use anyhow::Context;
use std::sync::Arc;
use tokio::task;

#[derive(Clone, Debug)]
pub struct SearchService {
    db: Arc<DbHandles>,
}

impl SearchService {
    pub fn new(db: Arc<DbHandles>) -> Self {
        Self { db }
    }

    fn build_search_query(
        owned_query: &SearchQuery,
        post_types_as_strings: &[String],
    ) -> (String, Vec<Box<dyn rusqlite::ToSql>>) {
        let mut conditions = vec![];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if !owned_query.text_query.is_empty() {
            conditions.push("posts_fts MATCH ?".to_string());
            params.push(Box::new(owned_query.text_query.clone()));
        }

        for tag in &owned_query.tags {
            conditions
                .push("EXISTS (SELECT 1 FROM json_each(posts.tags) WHERE value = ?)".to_string());
            params.push(Box::new(tag.clone()));
        }

        if let Some(date) = &owned_query.from_date {
            conditions.push("posts.date >= ?".to_string());
            params.push(Box::new(date.clone()));
        }
        if let Some(date) = &owned_query.to_date {
            conditions.push("posts.date <= ?".to_string());
            params.push(Box::new(date.clone()));
        }

        if !post_types_as_strings.is_empty() {
            let placeholders = post_types_as_strings
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(", ");
            conditions.push(format!("posts.content_type IN ({placeholders})"));
            for pt_str in post_types_as_strings {
                params.push(Box::new(pt_str.clone()));
            }
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let order_clause = if owned_query.text_query.is_empty() {
            "ORDER BY date DESC".to_string()
        } else {
            "ORDER BY rank".to_string()
        };

        (format!("{where_clause} {order_clause}"), params)
    }

    pub async fn search(
        &self,
        query: &SearchQuery,
        page: usize,
        per_page: usize,
    ) -> anyhow::Result<(Vec<crate::post::SummaryPost>, usize)> {
        // Create a full clone of the query data to move into the thread
        let owned_query = SearchQuery {
            text_query: query.text_query.clone(),
            tags: query.tags.clone(),
            from_date: query.from_date.clone(),
            to_date: query.to_date.clone(),
            post_type: Vec::default(),
        };
        let post_types_as_strings: Vec<String> = query
            .post_type
            .iter()
            .map(|pt| pt.to_owned().into())
            .collect();
        let offset = (page - 1) * per_page;
        let pool = self.db.primary.load();

        let (posts, total) = task::spawn_blocking(move || {
            let conn = pool.get()?;
            let base_query = if owned_query.text_query.is_empty() {
                "FROM posts".to_string()
            } else {
                "FROM posts INNER JOIN posts_fts ON posts.rowid = posts_fts.rowid".to_string()
            };

            let (filter_clauses, mut params) =
                Self::build_search_query(&owned_query, &post_types_as_strings);

            // Prepare count query first (borrows params immutably)
            let count_query = format!("SELECT COUNT(*) {base_query} {filter_clauses}");
            let total: i64 = conn.query_row(
                &count_query,
                rusqlite::params_from_iter(params.iter().map(|p| &**p)),
                |r| r.get(0),
            )?;

            // Main query to fetch posts (takes ownership of params)
            let posts_query = format!(
                "SELECT posts.id, posts.content_type, posts.title, posts.link, posts.via, posts.quote_author, posts.date {base_query} {filter_clauses} LIMIT ? OFFSET ?"
            );

            let mut stmt = conn.prepare(&posts_query)?;
            #[allow(clippy::cast_possible_wrap)]
            params.push(Box::new(per_page as i64));
            #[allow(clippy::cast_possible_wrap)]
            params.push(Box::new(offset as i64));

            // Execute query and collect results
            let iter = stmt.query_map(
                rusqlite::params_from_iter(params.iter().map(|p| &**p)),
                PostService::row_to_summary_post,
            )?;
            let mut posts = Vec::new();
            for post in iter {
                posts.push(post?);
            }

            Ok::<_, anyhow::Error>((posts, usize::try_from(total)?))
        })
        .await?
        .context("Search execution failed")?;

        Ok((posts, total))
    }
}
