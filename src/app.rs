use anyhow::Result;
use axum::response::{IntoResponse, Response};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_embed::RustEmbed;
use tera::{Context, Tera};

#[derive(Debug, Clone)]
pub struct AppState {
    pub post_service: crate::services::post::PostService,
    pub search_service: crate::services::search::SearchService,
    pub tera: Tera,
}

impl AppState {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        let tera = Self::load_templates().unwrap();
        AppState {
            post_service: crate::services::post::PostService::new(pool.clone()),
            search_service: crate::services::search::SearchService::new(pool),
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
