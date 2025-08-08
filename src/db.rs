use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::Result;
use arc_swap::ArcSwap;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OpenFlags;
use tokio::{fs, sync::RwLock};

#[derive(Debug)]
pub struct DbHandles {
    pub primary: ArcSwap<Pool<SqliteConnectionManager>>,
    pub draining: RwLock<Option<Arc<Pool<SqliteConnectionManager>>>>,
    pub primary_path: RwLock<PathBuf>,
}

impl DbHandles {
    pub fn new(initial_pool: Pool<SqliteConnectionManager>, initial_path: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            primary: ArcSwap::from(Arc::new(initial_pool)),
            draining: RwLock::new(None),
            primary_path: RwLock::new(initial_path),
        })
    }

    pub async fn swap_primary(
        self: &Arc<Self>,
        new_pool: Pool<SqliteConnectionManager>,
        new_path: PathBuf,
    ) {
        let old_pool = self.primary.swap(Arc::new(new_pool));

        {
            let mut draining_guard = self.draining.write().await;
            *draining_guard = Some(old_pool.clone());
        }

        let old_path = {
            let mut path_guard = self.primary_path.write().await;
            let old = path_guard.clone();
            *path_guard = new_path;
            old
        };

        tokio::spawn(Self::drain_and_delete(self.clone(), old_pool, old_path));
    }

    async fn drain_and_delete(
        self: Arc<Self>,
        pool: Arc<Pool<SqliteConnectionManager>>,
        path: PathBuf,
    ) {
        let mut attempts = 0;
        loop {
            let state = pool.state();
            if state.connections == state.idle_connections {
                // No active connections – close idle ones, drop the pool, and remove the file.

                // First clear the reference stored in `self.draining` so it no longer keeps the
                // pool alive.
                {
                    let mut draining_guard = self.draining.write().await;
                    *draining_guard = None;
                }

                // Drop the Arc we hold. This will close any idle connections.
                drop(pool);

                // Now it should be safe to delete the underlying file.
                if let Err(e) = fs::remove_file(&path).await {
                    eprintln!("Failed to delete old DB file {path:?}: {e}");
                } else {
                    println!("Deleted old DB file {path:?}");
                }

                break;
            }
            attempts += 1;
            if attempts > 60 {
                eprintln!("Timeout draining old pool for {path:?}. File not deleted.");
                break;
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }
}

pub fn init_pool(path: &Path) -> Result<Pool<SqliteConnectionManager>> {
    let manager = SqliteConnectionManager::file(path)
        .with_flags(OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI);
    let pool = Pool::builder().max_size(16).build(manager)?;
    Ok(pool)
}

pub async fn update_database_url_env(new_path: &std::path::Path) -> anyhow::Result<()> {
    const ENV_FILE: &str = ".env";
    let env_path = std::path::Path::new(ENV_FILE);

    // Read existing contents (if any)
    let contents = if env_path.exists() {
        fs::read_to_string(env_path).await?
    } else {
        String::new()
    };

    // Split into lines, update or append DATABASE_URL
    let mut lines: Vec<String> = contents
        .lines()
        .map(std::string::ToString::to_string)
        .collect();
    let mut updated = false;
    for line in &mut lines {
        if line.starts_with("DATABASE_URL=") {
            *line = format!("DATABASE_URL={}", new_path.display());
            updated = true;
            break;
        }
    }
    if !updated {
        lines.push(format!("DATABASE_URL={}", new_path.display()));
    }

    // Write back
    fs::write(env_path, lines.join("\n")).await?;
    Ok(())
}
