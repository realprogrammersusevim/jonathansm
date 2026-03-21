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
                let title = format!("Link: {link_title}");
                let link_html = post.link.map_or_else(String::new, |link| {
                    format!(r#"<p>Link: <a href="{link}">{link_title}</a></p>"#)
                });
                (title, format!("{}{}", link_html, post.content))
            }
            ContentType::Quote => {
                let author = post.quote_author.as_deref().unwrap_or("an unknown source");
                let title = post
                    .title
                    .clone()
                    .unwrap_or_else(|| format!("Quote from {author}"));
                let attribution = post.quote_author.map_or_else(String::new, |name| {
                    format!("<figcaption>— {name}</figcaption>")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::post::{ContentType, Post};

    fn base_post(content_type: ContentType) -> Post {
        Post {
            id: "test-post".to_string(),
            content_type,
            title: Some("My Title".to_string()),
            link: None,
            via: None,
            quote_author: None,
            date: "2023-01-01T00:00:00Z".to_string(),
            last_updated: None,
            content: "<p>Body text</p>".to_string(),
            commits: None,
            tags: None,
            real_commits: None,
            related_posts: None,
        }
    }

    #[test]
    fn rss_entry_from_post_type() {
        let entry = RssEntry::from(base_post(ContentType::Post));
        assert_eq!(entry.title, "My Title");
        assert_eq!(entry.content, "<p>Body text</p>");
        assert!(entry.guid.contains("test-post"));
    }

    #[test]
    fn rss_entry_from_post_type_untitled() {
        let mut post = base_post(ContentType::Post);
        post.title = None;
        let entry = RssEntry::from(post);
        assert_eq!(entry.title, "Untitled");
    }

    #[test]
    fn rss_entry_from_link_type() {
        let mut post = base_post(ContentType::Link);
        post.link = Some("https://example.com".to_string());
        let entry = RssEntry::from(post);
        assert_eq!(entry.title, "Link: My Title");
        assert!(entry.content.contains(r#"href="https://example.com""#));
        assert!(entry.content.contains("<p>Body text</p>"));
    }

    #[test]
    fn rss_entry_from_quote_type() {
        let mut post = base_post(ContentType::Quote);
        post.title = None;
        post.quote_author = Some("Alice".to_string());
        let entry = RssEntry::from(post);
        assert_eq!(entry.title, "Quote from Alice");
        assert!(entry.content.contains("<blockquote>"));
        assert!(entry.content.contains("<figcaption>"));
        assert!(entry.content.contains("Alice"));
    }

    #[test]
    fn rss_entry_to_xml_structure() {
        let entry = RssEntry::from(base_post(ContentType::Post));
        let xml = entry.to_xml();
        assert!(xml.contains("<item>"));
        assert!(xml.contains("</item>"));
        assert!(xml.contains(r#"isPermalink="true""#));
        assert!(xml.contains("<![CDATA["));
        assert!(xml.contains("]]>"));
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
                {rss_items}
            </channel>
        </rss>
        "#
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/atom+xml")],
        rss,
    )
}
