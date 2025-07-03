use anyhow::Result;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use tokio::task;

#[derive(Clone, Debug)]
pub struct ImageService {
    pool: Pool<SqliteConnectionManager>,
}

impl ImageService {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    pub async fn get_image_data(&self, filename: &str) -> Result<Option<Vec<u8>>> {
        let filename = filename.to_owned();
        let pool = self.pool.clone();

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
