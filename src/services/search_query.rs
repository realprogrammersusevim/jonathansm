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
