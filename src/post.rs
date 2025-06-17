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
            "post" => ContentType::Post,
            "link" => ContentType::Link,
            "quote" => ContentType::Quote,
            _ => ContentType::Post,
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
    #[serde(skip_serializing)]
    pub commits: Option<String>,
    pub real_commits: Option<Vec<Commit>>,
}
