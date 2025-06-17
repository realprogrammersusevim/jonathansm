use anyhow::Result;
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;
use sqlx::{Pool, Sqlite};
use tera::{Context, Tera};

#[derive(Clone)]
pub struct AppState {
    pub post_service: crate::services::post::PostService,
    pub tera: Tera,
}

impl AppState {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        let tera = Self::load_templates().unwrap();
        AppState {
            post_service: crate::services::post::PostService::new(pool),
            tera,
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
            templates.push((path.into_owned(), content.to_owned()))
        }

        tera.add_raw_templates(templates)
            .expect("Can't parse templates.");

        Ok(tera)
    }

    pub fn render(&self, template: &str, context: &Context) -> Result<Response> {
        let rendered = self.tera.render(template, context)?;
        Ok(axum::response::Html(rendered).into_response())
    }
}
