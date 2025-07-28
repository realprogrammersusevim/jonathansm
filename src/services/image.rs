use crate::db::DbHandles;
use anyhow::Result;
use rusqlite::params;
use std::sync::Arc;
use tokio::task;

#[derive(Clone, Debug)]
pub struct ImageService {
    db: Arc<DbHandles>,
}

impl ImageService {
    pub fn new(db: Arc<DbHandles>) -> Self {
        Self { db }
    }

    pub async fn get_image_data(&self, filename: &str) -> Result<Option<Vec<u8>>> {
        let filename = filename.to_owned();
        let pool = self.db.primary.load();

        task::spawn_blocking(move || {
            let conn = pool.get()?;
            let mut stmt = conn.prepare("SELECT data FROM images WHERE filename = ?")?;

            stmt.query_row(params![filename], |row| {
                let data: Vec<u8> = row.get(0)?;
                Ok(data)
            })
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                _ => Err(e.into()),
            })
        })
        .await?
    }
}
