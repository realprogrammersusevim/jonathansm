use crate::{AppState, IntoResponse, QueryPost};
use axum::{
    extract::State,
    http::{header, StatusCode},
};

struct RssEntry {
    title: String,
    link: String,
    content: String,
    pub_date: String,
    author: String,
    guid: String,
}

impl From<QueryPost> for RssEntry {
    fn from(post: QueryPost) -> Self {
        let full_url = format!("https://jonathansm.com/post/{}", post.id);
        Self {
            title: post.title.unwrap_or("Untitled".to_string()),
            link: full_url.clone(),
            content: post.content,
            author: "Jonathan".to_string(),
            pub_date: post.date,
            guid: full_url,
        }
    }
}

impl RssEntry {
    fn to_xml(&self) -> String {
        format!(
            r#"
            <item>
                <title>{}</title>
                <link>{}</link>
                <content type="html">{}</content>
                <pubDate>{}</pubDate>
                <author>{}</author>
                <guid isPermalink="true">{}</guid>
            </item>
            "#,
            self.title, self.link, self.content, self.pub_date, self.author, self.guid
        )
        .trim()
        .to_string()
    }
}

pub async fn feed(app: State<AppState>) -> impl IntoResponse {
    let rss_items: String = sqlx::query_as!(
        QueryPost,
        r#"
        SELECT *
        FROM posts
        ORDER BY date DESC
        LIMIT 20
        "#,
    )
    .fetch_all(&app.pool)
    .await
    .unwrap()
    .into_iter()
    .map(|post| RssEntry::from(post).to_xml())
    .collect();

    let rss = format!(
        r#"
        <?xml version="1.0" encoding="UTF-8" ?>
        <rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
            <channel>
                <title>Jonathan's Blog</title>
                <link>https://jonathansm.com</link>
                <description>Jonathan's Blog</description>
                <language>en-us</language>
                <atom:link href="https://jonathansm.com/feed" rel="self" type="application/rss+xml" />
                {}
            </channel>
        </rss>
        "#,
        rss_items
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/atom+xml")],
        rss,
    )
}
