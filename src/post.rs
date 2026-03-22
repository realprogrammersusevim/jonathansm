use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Commit {
    pub id: String,
    pub date: String,
    pub subject: String,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum ContentType {
    Post,
    Link,
    Quote,
}

impl From<String> for ContentType {
    fn from(s: String) -> Self {
        match s.as_str() {
            "link" => ContentType::Link,
            "quote" => ContentType::Quote,
            _ => ContentType::Post,
        }
    }
}

impl From<ContentType> for String {
    fn from(val: ContentType) -> Self {
        match val {
            ContentType::Post => "post".into(),
            ContentType::Link => "link".into(),
            ContentType::Quote => "quote".into(),
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
    pub last_updated: Option<String>,
    pub content: String,
    #[serde(skip_serializing)]
    pub commits: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub description: Option<String>,
    pub real_commits: Option<Vec<Commit>>,
    pub related_posts: Option<Vec<SummaryPost>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SummaryPost {
    pub id: String,
    pub content_type: ContentType,
    pub title: Option<String>,
    pub link: Option<String>,
    pub via: Option<String>,
    pub quote_author: Option<String>,
    pub date: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_type_from_string_post() {
        assert!(matches!(
            ContentType::from("post".to_string()),
            ContentType::Post
        ));
    }

    #[test]
    fn content_type_from_string_link() {
        assert!(matches!(
            ContentType::from("link".to_string()),
            ContentType::Link
        ));
    }

    #[test]
    fn content_type_from_string_quote() {
        assert!(matches!(
            ContentType::from("quote".to_string()),
            ContentType::Quote
        ));
    }

    #[test]
    fn content_type_from_string_unknown_defaults_to_post() {
        assert!(matches!(
            ContentType::from("unknown".to_string()),
            ContentType::Post
        ));
    }

    #[test]
    fn content_type_from_string_uppercase_defaults_to_post() {
        assert!(matches!(
            ContentType::from("POST".to_string()),
            ContentType::Post
        ));
    }

    #[test]
    fn string_from_content_type_post() {
        assert_eq!(String::from(ContentType::Post), "post");
    }

    #[test]
    fn string_from_content_type_link() {
        assert_eq!(String::from(ContentType::Link), "link");
    }

    #[test]
    fn string_from_content_type_quote() {
        assert_eq!(String::from(ContentType::Quote), "quote");
    }
}
