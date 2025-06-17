use crate::{post::ContentType, post::Post, AppState};
use axum::response::IntoResponse;
use axum::{
    extract::State,
    http::{header, StatusCode},
};

struct RssEntry {
    title: String,
    link: String,
    content: String,
    pub_date: String,
    guid: String,
}

impl From<Post> for RssEntry {
    fn from(post: Post) -> Self {
        let full_url = format!("https://jonathansm.com/post/{}", post.id);

        let (title, content) = match post.content_type {
            ContentType::Post => (
                post.title.unwrap_or_else(|| "Untitled".to_string()),
                post.content,
            ),
            ContentType::Link => {
                let link_title = post.title.as_deref().unwrap_or("this link");
                let title = format!("Link: {}", link_title);
                let link_html = post.link.map_or_else(String::new, |link| {
                    format!(r#"<p>Link: <a href="{}">{}</a></p>"#, link, link_title)
                });
                (title, format!("{}{}", link_html, post.content))
            }
            ContentType::Quote => {
                let author = post.quote_author.as_deref().unwrap_or("an unknown source");
                let title = post
                    .title
                    .clone()
                    .unwrap_or_else(|| format!("Quote from {}", author));
                let attribution = post.quote_author.map_or_else(String::new, |name| {
                    format!("<figcaption>— {}</figcaption>", name)
                });
                let blockquote =
                    format!("<blockquote>{}</blockquote>{}", post.content, attribution);
                (title, blockquote)
            }
        };

        Self {
            title,
            link: full_url.clone(),
            content,
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
                <guid isPermalink="true">{}</guid>
                <pubDate>{}</pubDate>
                <content:encoded><![CDATA[{}]]></content:encoded>
            </item>
            "#,
            self.title, self.link, self.guid, self.pub_date, self.content
        )
        .trim()
        .to_string()
    }
}

pub async fn feed(app: State<AppState>) -> impl IntoResponse {
    let entries = app.0.post_service.get_rss_entries().await.unwrap();
    let rss_items: String = entries
        .into_iter()
        .map(RssEntry::from)
        .map(|entry| entry.to_xml())
        .collect();

    let rss = format!(
        r#"
        <?xml version="1.0" encoding="UTF-8" ?>
        <rss version="2.0" xmlns:content="http://purl.org/rss/1.0/modules/content/" xmlns:atom="http://www.w3.org/2005/Atom">
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
