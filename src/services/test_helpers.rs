use crate::db::DbHandles;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_db_path() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!("jonathansm_test_{}_{}.db", std::process::id(), n))
}

const SCHEMA: &str = "
CREATE TABLE posts (
    id TEXT PRIMARY KEY,
    content_type TEXT NOT NULL DEFAULT 'post',
    title TEXT,
    link TEXT,
    via TEXT,
    quote_author TEXT,
    date TEXT NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    commits TEXT,
    tags TEXT
);
CREATE TABLE commits (
    id TEXT PRIMARY KEY,
    date TEXT NOT NULL,
    subject TEXT NOT NULL,
    body TEXT
);
CREATE VIRTUAL TABLE posts_fts USING fts5(id UNINDEXED, title, content);
CREATE TABLE post_embeddings (
    id TEXT PRIMARY KEY,
    embedding BLOB
);
CREATE TABLE images (
    filename TEXT PRIMARY KEY,
    data BLOB NOT NULL
);
";

/// Seed a connection with a canonical set of test data:
///  - 12 regular posts (post-01 … post-12) dated 2024-01-12 → 2024-01-01
///  - 1 special page  (id = "about")
///  - 2 commits referenced by post-01 and post-02
///  - 1 image  (images/test.png)
///
/// post-01: link type,  tags ["rust","web"], commits ["commit-1"]
/// post-02: quote type, tags ["rust"],       commits ["commit-2","commit-1"], quote_author "Alice"
/// post-03: post type,  tags ["go"],         content "golang article about performance"
/// post-04…12: plain posts, no tags, no commits
pub fn seed(conn: &Connection) {
    conn.execute_batch(SCHEMA).unwrap();

    // -- commits --
    conn.execute_batch(
        "INSERT INTO commits VALUES
            ('commit-1','2024-01-15T00:00:00Z','First commit',NULL),
            ('commit-2','2024-01-12T00:00:00Z','Second commit','Some body text');",
    )
    .unwrap();

    // -- posts --
    let posts: &[(
        &str,
        &str,
        Option<&str>,
        Option<&str>,
        Option<&str>,
        Option<&str>,
        &str,
        &str,
        Option<&str>,
        Option<&str>,
    )] = &[
        (
            "post-01",
            "link",
            Some("Rust Link"),
            Some("https://rust-lang.org"),
            None,
            None,
            "2024-01-12T00:00:00Z",
            "link post content",
            Some(r#"["commit-1"]"#),
            Some(r#"["rust","web"]"#),
        ),
        (
            "post-02",
            "quote",
            Some("Wise Words"),
            None,
            None,
            Some("Alice"),
            "2024-01-11T00:00:00Z",
            "quoted text here",
            Some(r#"["commit-2","commit-1"]"#),
            Some(r#"["rust"]"#),
        ),
        (
            "post-03",
            "post",
            Some("Golang Post"),
            None,
            None,
            None,
            "2024-01-10T00:00:00Z",
            "golang article about performance",
            None,
            Some(r#"["go"]"#),
        ),
        (
            "post-04",
            "post",
            Some("Post Four"),
            None,
            None,
            None,
            "2024-01-09T00:00:00Z",
            "regular post",
            None,
            None,
        ),
        (
            "post-05",
            "post",
            Some("Post Five"),
            None,
            None,
            None,
            "2024-01-08T00:00:00Z",
            "regular post",
            None,
            None,
        ),
        (
            "post-06",
            "post",
            Some("Post Six"),
            None,
            None,
            None,
            "2024-01-07T00:00:00Z",
            "regular post",
            None,
            None,
        ),
        (
            "post-07",
            "post",
            Some("Post Seven"),
            None,
            None,
            None,
            "2024-01-06T00:00:00Z",
            "regular post",
            None,
            None,
        ),
        (
            "post-08",
            "post",
            Some("Post Eight"),
            None,
            None,
            None,
            "2024-01-05T00:00:00Z",
            "regular post",
            None,
            None,
        ),
        (
            "post-09",
            "post",
            Some("Post Nine"),
            None,
            None,
            None,
            "2024-01-04T00:00:00Z",
            "regular post",
            None,
            None,
        ),
        (
            "post-10",
            "post",
            Some("Post Ten"),
            None,
            None,
            None,
            "2024-01-03T00:00:00Z",
            "regular post",
            None,
            None,
        ),
        (
            "post-11",
            "post",
            Some("Post Eleven"),
            None,
            None,
            None,
            "2024-01-02T00:00:00Z",
            "regular post",
            None,
            None,
        ),
        (
            "post-12",
            "post",
            Some("Post Twelve"),
            None,
            None,
            None,
            "2024-01-01T00:00:00Z",
            "regular post",
            None,
            None,
        ),
        (
            "about",
            "special",
            Some("About Me"),
            None,
            None,
            None,
            "2023-01-01T00:00:00Z",
            "about page content",
            None,
            None,
        ),
    ];

    for (id, ct, title, link, via, qa, date, content, commits, tags) in posts {
        conn.execute(
            "INSERT INTO posts(id,content_type,title,link,via,quote_author,date,content,commits,tags)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            rusqlite::params![id, ct, title, link, via, qa, date, content, commits, tags],
        )
        .unwrap();

        // Populate FTS table for searchable posts
        if *ct != "special" {
            conn.execute(
                "INSERT INTO posts_fts(id,title,content) VALUES (?1,?2,?3)",
                rusqlite::params![id, title, content],
            )
            .unwrap();
        }
    }

    // -- image --
    conn.execute(
        "INSERT INTO images(filename,data) VALUES ('images/test.png', ?1)",
        rusqlite::params![b"\x89PNG".to_vec()],
    )
    .unwrap();
}

/// Create a temporary SQLite file, seed it, and return an `Arc<DbHandles>`
/// backed by an r2d2 read-write pool.  Also returns the path so tests that
/// need it (e.g. `init_pool`) can use it.
pub fn test_db() -> (Arc<DbHandles>, PathBuf) {
    let path = unique_db_path();

    {
        let conn = Connection::open(&path).unwrap();
        seed(&conn);
    }

    let manager = SqliteConnectionManager::file(&path);
    let pool = Pool::builder().max_size(2).build(manager).unwrap();
    let db = DbHandles::new(pool, path.clone());
    (db, path)
}
