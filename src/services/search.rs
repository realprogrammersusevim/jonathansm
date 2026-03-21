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
                "FROM posts INNER JOIN posts_fts ON posts.id = posts_fts.id".to_string()
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
            let posts_query = if owned_query.text_query.is_empty() {
                format!(
                    "SELECT posts.id, posts.content_type, posts.title, posts.link, posts.via, posts.quote_author, posts.date {base_query} {filter_clauses} LIMIT ? OFFSET ?"
                )
            } else {
                format!(
                    "SELECT posts.id, posts.content_type, posts.title, posts.link, posts.via, posts.quote_author, posts.date, bm25(posts_fts) AS rank {base_query} {filter_clauses} LIMIT ? OFFSET ?"
                )
            };

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::search_query::SearchQuery;
    use crate::services::test_helpers::test_db;

    fn search_service() -> SearchService {
        let (db, _) = test_db();
        SearchService::new(db)
    }

    #[tokio::test]
    async fn empty_query_returns_all_posts() {
        let svc = search_service();
        let q = SearchQuery::from_raw("");
        // 12 regular + 1 special = 13 total rows; search has no special filter
        let (_, total) = svc.search(&q, 1, 100).await.unwrap();
        assert_eq!(total, 13);
    }

    #[tokio::test]
    async fn text_query_finds_matching_posts() {
        let svc = search_service();
        // post-03 content = "golang article about performance"
        let q = SearchQuery::from_raw("golang");
        let (posts, total) = svc.search(&q, 1, 10).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(posts[0].id, "post-03");
    }

    #[tokio::test]
    async fn text_query_no_match_returns_empty() {
        let svc = search_service();
        let q = SearchQuery::from_raw("xyzzy_no_such_word");
        let (posts, total) = svc.search(&q, 1, 10).await.unwrap();
        assert_eq!(total, 0);
        assert!(posts.is_empty());
    }

    #[tokio::test]
    async fn tag_filter_returns_only_tagged_posts() {
        let svc = search_service();
        // post-01 and post-02 have tag "rust"
        let q = SearchQuery::from_raw("tag:rust");
        let (posts, total) = svc.search(&q, 1, 10).await.unwrap();
        assert_eq!(total, 2);
        let ids: Vec<&str> = posts.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"post-01"));
        assert!(ids.contains(&"post-02"));
    }

    #[tokio::test]
    async fn from_date_excludes_earlier_posts() {
        let svc = search_service();
        // Only post-01 (2024-01-12) and post-02 (2024-01-11) are on or after 2024-01-11
        let q = SearchQuery::from_raw("from:2024-01-11");
        let (_, total) = svc.search(&q, 1, 100).await.unwrap();
        assert_eq!(total, 2);
    }

    #[tokio::test]
    async fn to_date_excludes_later_posts() {
        let svc = search_service();
        // Dates are stored as "2024-01-NNT00:00:00Z"; string comparison means
        // "2024-01-01T..." < "2024-01-02", so "to:2024-01-02" captures post-12.
        let q = SearchQuery::from_raw("to:2024-01-02");
        let (posts, _total) = svc.search(&q, 1, 100).await.unwrap();
        let regular: Vec<_> = posts.iter().filter(|p| p.id != "about").collect();
        assert_eq!(regular.len(), 1);
        assert_eq!(regular[0].id, "post-12");
    }

    #[tokio::test]
    async fn type_filter_link_returns_only_links() {
        let svc = search_service();
        let q = SearchQuery::from_raw("type:link");
        let (posts, total) = svc.search(&q, 1, 10).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(posts[0].id, "post-01");
    }

    #[tokio::test]
    async fn type_filter_quote_returns_only_quotes() {
        let svc = search_service();
        let q = SearchQuery::from_raw("type:quote");
        let (posts, total) = svc.search(&q, 1, 10).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(posts[0].id, "post-02");
    }

    #[tokio::test]
    async fn pagination_page2_returns_correct_slice() {
        let svc = search_service();
        let q = SearchQuery::from_raw("");
        let (page1, _) = svc.search(&q, 1, 5).await.unwrap();
        let (page2, _) = svc.search(&q, 2, 5).await.unwrap();
        assert_eq!(page1.len(), 5);
        assert_eq!(page2.len(), 5);
        // No overlap between pages
        let ids1: std::collections::HashSet<&str> = page1.iter().map(|p| p.id.as_str()).collect();
        let ids2: std::collections::HashSet<&str> = page2.iter().map(|p| p.id.as_str()).collect();
        assert!(ids1.is_disjoint(&ids2));
    }

    #[tokio::test]
    async fn combined_tag_and_type_filter() {
        let svc = search_service();
        // post-01 is a link with tag "rust"; post-02 is a quote with tag "rust"
        let q = SearchQuery::from_raw("tag:rust type:link");
        let (posts, total) = svc.search(&q, 1, 10).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(posts[0].id, "post-01");
    }
}
