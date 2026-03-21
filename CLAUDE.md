# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Personal website (jonathansm.com) built as a single Rust binary with embedded templates/static files, served alongside a read-only SQLite database. Content is managed in a separate repo and deployed as a `.db` file.

## Commands

Uses `just` (justfile) as the task runner. The justfile loads `.env` automatically.

- **Build:** `just self` (macOS) or `just linux` (cross-compile to x86_64-unknown-linux-gnu)
- **Dev server:** `just auto-reload` (alias: `just ar`) — watches `.rs` and `.html` files, auto-recompiles and restarts
- **Format:** `just fmt` — runs `cargo fmt` and `prettier` on CSS/HTML files
- **Lint:** `just lint` — runs `cargo fmt` then `cargo clippy` with strict pedantic rules
- **Test:** `just test` — runs `cargo test`
- **Deploy:** `just deploy` — rsync release binary to server, restart systemd service

## Architecture

**Request flow:** Axum router → route handlers (`routes.rs`) → services (`services/`) → SQLite via r2d2 connection pool → Tera templates (`templates/`)

Key source files:
- `src/main.rs` — server startup, router setup
- `src/app.rs` — AppState, Tera template initialization
- `src/db.rs` — connection pool management, hot-swap support (swap active database without restart via `arc-swap`)
- `src/routes.rs` — all route handlers
- `src/post.rs` — Post/Link/Quote data types
- `src/services/post.rs` — database queries, type conversions
- `src/services/search.rs` — full-text search (SQLite FTS)
- `src/services/search_query.rs` — search query parsing (supports `tag:`, `from:`, `to:`, `type:` syntax)
- `src/services/image.rs` — serves images stored in SQLite
- `src/services/test_helpers.rs` — shared test infrastructure: in-memory SQLite schema, canonical seed data (12 posts + about page, commits, image), and `test_db()` helper that returns a seeded `Arc<DbHandles>`
- `src/rss.rs` — RSS feed generation

**Templates:** Tera (Jinja2-like) in `templates/`. Base layout in `base.html`, reusable components in `macros.html`.

**Static assets:** Embedded at compile time via `rust-embed`. CSS in `static/css/style.css`, fonts (fbb family) in `static/fonts/`.

**Database:** Read-only SQLite with vector search via `sqlite-vec` for related post recommendations. Posts include git commit metadata for "last updated" tracking.

## Testing

Unit tests live alongside source files in `#[cfg(test)]` modules. Integration-style tests that need a database use `test_db()` from `src/services/test_helpers.rs`, which spins up a temporary SQLite file seeded with canonical fixture data (no mocking). Coverage spans `db.rs`, `post.rs`, `rss.rs`, `services/post.rs`, `services/search.rs`, and `services/search_query.rs`.

## Conventions

- Commits follow conventional commit format: `feat:`, `fix:`, `style:`, `refactor:`, `chore:`, `docs:`
- Clippy runs with `-D clippy::correctness -D clippy::perf` (deny) and `-W clippy::suspicious -W clippy::complexity -W clippy::style -W clippy::pedantic` (warn)
