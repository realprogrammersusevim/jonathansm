use regex::Regex;

use crate::post::ContentType;

#[derive(Debug, Default)]
pub struct SearchQuery {
    pub text_query: String,
    pub tags: Vec<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub post_type: Vec<ContentType>,
}

impl SearchQuery {
    pub fn from_raw(raw: &str) -> Self {
        let mut result = SearchQuery::default();
        let tag_re = Regex::new(r"tag:([^\s]+)").unwrap();
        let date_re = Regex::new(r"(from|to):(\d{4}-\d{2}-\d{2})").unwrap();
        let type_re = Regex::new(r"type:(post|link|quote)").unwrap();

        // Extract tags
        for cap in tag_re.captures_iter(raw) {
            if let Some(tag) = cap.get(1).map(|m| m.as_str().to_string()) {
                result.tags.push(tag);
            }
        }

        // Extract dates
        for cap in date_re.captures_iter(raw) {
            if let (Some(typ), Some(date)) = (cap.get(1), cap.get(2)) {
                match typ.as_str() {
                    "from" => result.from_date = Some(date.as_str().to_string()),
                    "to" => result.to_date = Some(date.as_str().to_string()),
                    _ => (),
                }
            }
        }

        // Extract type
        for cap in type_re.captures_iter(raw) {
            if let Some(p_type) = cap.get(1).map(|m| m.as_str().to_string()) {
                result.post_type.push(ContentType::from(p_type));
            }
        }

        // Clean text query
        result.text_query = tag_re.replace_all(raw, "").to_string();
        result.text_query = date_re.replace_all(&result.text_query, "").to_string();
        result.text_query = type_re.replace_all(&result.text_query, "").to_string();
        result.text_query = result.text_query.trim().to_string();

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_query() {
        let q = SearchQuery::from_raw("hello world");
        assert_eq!(q.text_query, "hello world");
        assert!(q.tags.is_empty());
        assert!(q.from_date.is_none());
        assert!(q.to_date.is_none());
        assert!(q.post_type.is_empty());
    }

    #[test]
    fn empty_query() {
        let q = SearchQuery::from_raw("");
        assert_eq!(q.text_query, "");
        assert!(q.tags.is_empty());
        assert!(q.from_date.is_none());
        assert!(q.to_date.is_none());
        assert!(q.post_type.is_empty());
    }

    #[test]
    fn single_tag() {
        let q = SearchQuery::from_raw("tag:rust");
        assert_eq!(q.tags, vec!["rust"]);
        assert_eq!(q.text_query, "");
    }

    #[test]
    fn multiple_tags() {
        let q = SearchQuery::from_raw("tag:rust tag:web");
        assert_eq!(q.tags, vec!["rust", "web"]);
    }

    #[test]
    fn tag_with_text() {
        let q = SearchQuery::from_raw("hello tag:rust");
        assert_eq!(q.text_query, "hello");
        assert_eq!(q.tags, vec!["rust"]);
    }

    #[test]
    fn from_date() {
        let q = SearchQuery::from_raw("from:2023-01-01");
        assert_eq!(q.from_date, Some("2023-01-01".to_string()));
        assert!(q.to_date.is_none());
    }

    #[test]
    fn to_date() {
        let q = SearchQuery::from_raw("to:2023-12-31");
        assert_eq!(q.to_date, Some("2023-12-31".to_string()));
        assert!(q.from_date.is_none());
    }

    #[test]
    fn date_range() {
        let q = SearchQuery::from_raw("from:2023-01-01 to:2023-12-31");
        assert_eq!(q.from_date, Some("2023-01-01".to_string()));
        assert_eq!(q.to_date, Some("2023-12-31".to_string()));
        assert_eq!(q.text_query, "");
    }

    #[test]
    fn type_post() {
        let q = SearchQuery::from_raw("type:post");
        assert_eq!(q.post_type.len(), 1);
        assert!(matches!(q.post_type[0], ContentType::Post));
    }

    #[test]
    fn type_link_and_quote() {
        let q = SearchQuery::from_raw("type:link type:quote");
        assert_eq!(q.post_type.len(), 2);
        assert!(matches!(q.post_type[0], ContentType::Link));
        assert!(matches!(q.post_type[1], ContentType::Quote));
    }

    #[test]
    fn all_filters_combined() {
        let q = SearchQuery::from_raw("type:post tag:rust hello world");
        assert_eq!(q.text_query, "hello world");
        assert_eq!(q.tags, vec!["rust"]);
        assert_eq!(q.post_type.len(), 1);
        assert!(matches!(q.post_type[0], ContentType::Post));
    }
}
