use crate::db::DbHandles;
use crate::services::image::ImageService;
use anyhow::Result;
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;
use std::collections::HashMap;
use std::sync::Arc;
use tera::{Context, Tera, Value};

fn humandate_filter(value: &Value, _args: &HashMap<String, Value>) -> tera::Result<Value> {
    let s = value
        .as_str()
        .ok_or_else(|| tera::Error::msg("humandate: expected a string"))?;
    let date = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|e| tera::Error::msg(format!("humandate: invalid date '{s}': {e}")))?;
    Ok(Value::String(date.format("%b. %-d, %Y").to_string()))
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub db: Arc<DbHandles>,
    pub post_service: crate::services::post::PostService,
    pub search_service: crate::services::search::SearchService,
    pub image_service: ImageService,
    pub tera: Tera,
    pub build_id: String,
}

impl AppState {
    pub fn new(db: Arc<DbHandles>) -> Self {
        let tera = Self::load_templates().unwrap();
        AppState {
            post_service: crate::services::post::PostService::new(db.clone()),
            search_service: crate::services::search::SearchService::new(db.clone()),
            image_service: ImageService::new(db.clone()),
            tera,
            build_id: build_id::get().to_string(),
            db,
        }
    }

    fn load_templates() -> Result<Tera> {
        #[derive(RustEmbed)]
        #[folder = "templates/"]
        struct TemplateAssets;

        let mut tera = Tera::default();
        let mut templates = vec![];

        for path in TemplateAssets::iter() {
            let content_bytes = TemplateAssets::get(&path)
                .ok_or_else(|| anyhow::anyhow!("Template {} not found", path))?
                .data;

            let content = std::str::from_utf8(&content_bytes)
                .map_err(|_| anyhow::anyhow!("Template {} not valid UTF-8", path))?;
            templates.push((path.into_owned(), content.to_owned()));
        }

        tera.add_raw_templates(templates)
            .expect("Can't parse templates.");

        tera.register_filter("humandate", humandate_filter);

        Ok(tera)
    }

    pub fn render(&self, template: &str, context: &Context) -> Result<Response> {
        let mut context = context.clone();
        context.insert("build_id", &self.build_id);
        match self.tera.render(template, &context) {
            Ok(rendered) => Ok(axum::response::Html(rendered).into_response()),
            Err(e) => {
                tracing::error!("Template rendering failed: {}", e);
                Err(anyhow::anyhow!("Template error: {}", e))
            }
        }
    }
}
