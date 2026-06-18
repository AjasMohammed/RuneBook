//! SQLite-backed data layer for Runebook.
//!
//! All SQL lives here, behind plain functions that take a `&Connection`. The
//! Tauri command wrappers in `lib.rs` lock the shared connection and call these.
//! This keeps the project's IPC boundary intact: the WebView never speaks SQL,
//! it calls Rust commands (see docs/01-architecture.md).
//!
//! A step is an optional `title` plus a free-form **markdown** `body` — users
//! write whatever they want and revise it later, rather than filling fixed
//! fields. Markdown is rendered in the UI with a copy button per code block
//! (see docs/05-decisions.md D8).

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

/// Migration v1 — the original schema. Kept verbatim so existing v1 databases
/// match before v2 upgrades them. New databases run v1 then v2 in sequence.
const MIGRATION_V1: &str = r#"
CREATE TABLE IF NOT EXISTS runbook (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  title       TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  created_at  TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS step (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  runbook_id  INTEGER NOT NULL REFERENCES runbook(id) ON DELETE CASCADE,
  position    INTEGER NOT NULL,
  title       TEXT NOT NULL,
  command     TEXT NOT NULL DEFAULT '',
  why         TEXT NOT NULL DEFAULT '',
  where_ctx   TEXT NOT NULL DEFAULT '',
  example     TEXT NOT NULL DEFAULT '',
  note        TEXT NOT NULL DEFAULT '',
  created_at  TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS tag (
  id    INTEGER PRIMARY KEY AUTOINCREMENT,
  name  TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS runbook_tag (
  runbook_id INTEGER NOT NULL REFERENCES runbook(id) ON DELETE CASCADE,
  tag_id     INTEGER NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
  PRIMARY KEY (runbook_id, tag_id)
);

CREATE INDEX IF NOT EXISTS idx_step_runbook ON step(runbook_id, position);
"#;

/// Open (creating if needed) the database at `path`, enable foreign keys, and
/// run migrations. Called once at startup; the resulting connection is shared.
///
/// The same database file is also opened by the standalone `runebook-mcp`
/// process (see `mcp-server/`, docs/06-mcp-server.md), so two processes may read
/// and write it at once. **WAL** lets readers and a single writer proceed
/// without blocking each other, and **busy_timeout** makes a momentarily-locked
/// write wait (up to 5s) instead of failing with `SQLITE_BUSY`. Both are
/// per-database/connection PRAGMAs that are harmless for the single-process case.
pub fn open(path: &std::path::Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    // Foreign keys are off by default in SQLite and must be set per-connection,
    // so ON DELETE CASCADE actually fires. WAL + busy_timeout let the app and the
    // MCP server share the file concurrently (see doc comment above).
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 5000;",
    )?;
    migrate(&conn)?;
    Ok(conn)
}

/// Apply migrations in order, tracked by `PRAGMA user_version`.
fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;

    if version < 1 {
        conn.execute_batch(MIGRATION_V1)?;
        conn.execute_batch("PRAGMA user_version = 1;")?;
    }
    if version < 2 {
        migrate_v2(conn)?;
        conn.execute_batch("PRAGMA user_version = 2;")?;
    }
    if version < 3 {
        // A small key/value store for app settings — first use is the persistent
        // "current runbook" that Quick-add appends to (Phase 4); later: custom
        // hotkey, theme, window position.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS setting (
                 key   TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );",
        )?;
        conn.execute_batch("PRAGMA user_version = 3;")?;
    }
    if version < 4 {
        // FTS5 mirror of step(title, body) for ranked search (Phase 3). If this
        // SQLite build lacks FTS5, skip without bumping the version so search
        // falls back to LIKE and a future FTS-capable build can still add it.
        if create_fts(conn).is_ok() {
            conn.execute_batch("PRAGMA user_version = 4;")?;
        }
    }
    if version < 5 {
        // Replay progress (docs D10): a per-step "done" flag so a multi-step
        // runbook can be worked through as a checklist and resumed later. Keyed
        // by step id and cascades when a step (or its runbook) is deleted. Each
        // table is created IF NOT EXISTS, so this is safe even if the optional
        // FTS step above was skipped and left the version counter behind.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS step_progress (
                 step_id    INTEGER PRIMARY KEY REFERENCES step(id) ON DELETE CASCADE,
                 done       INTEGER NOT NULL DEFAULT 0,
                 updated_at TEXT NOT NULL DEFAULT (datetime('now'))
             );",
        )?;
        conn.execute_batch("PRAGMA user_version = 5;")?;
    }
    if version < 6 {
        // Variable profiles (docs D12): named saved value sets per runbook for
        // the {{var}} placeholders. `data` is a JSON object of name->value;
        // secret-marked variables are excluded by the UI before saving, so a
        // secret value never reaches the database.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS var_profile (
                 runbook_id INTEGER NOT NULL REFERENCES runbook(id) ON DELETE CASCADE,
                 name       TEXT NOT NULL,
                 data       TEXT NOT NULL DEFAULT '{}',
                 PRIMARY KEY (runbook_id, name)
             );",
        )?;
        conn.execute_batch("PRAGMA user_version = 6;")?;
    }
    if version < 7 {
        // Project pinning (docs D15): an optional directory a runbook belongs to,
        // used as the working directory for executed commands. ALTER ADD COLUMN
        // is safe/idempotent enough here because user_version gates re-runs.
        conn.execute_batch(
            "ALTER TABLE runbook ADD COLUMN project_dir TEXT NOT NULL DEFAULT '';",
        )?;
        conn.execute_batch("PRAGMA user_version = 7;")?;
    }
    Ok(())
}

/// Create the external-content FTS5 table over `step(title, body)`, the triggers
/// that keep it in sync, and backfill existing rows. Errors (e.g. FTS5 not
/// compiled in) propagate so the caller can fall back to LIKE search.
fn create_fts(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS step_fts USING fts5(
             title, body, content='step', content_rowid='id'
         );
         CREATE TRIGGER IF NOT EXISTS step_ai AFTER INSERT ON step BEGIN
             INSERT INTO step_fts(rowid, title, body) VALUES (new.id, new.title, new.body);
         END;
         CREATE TRIGGER IF NOT EXISTS step_ad AFTER DELETE ON step BEGIN
             INSERT INTO step_fts(step_fts, rowid, title, body)
             VALUES ('delete', old.id, old.title, old.body);
         END;
         CREATE TRIGGER IF NOT EXISTS step_au AFTER UPDATE ON step BEGIN
             INSERT INTO step_fts(step_fts, rowid, title, body)
             VALUES ('delete', old.id, old.title, old.body);
             INSERT INTO step_fts(rowid, title, body) VALUES (new.id, new.title, new.body);
         END;",
    )?;
    // Rebuild the index from the content table (idempotent backfill).
    conn.execute_batch("INSERT INTO step_fts(step_fts) VALUES ('rebuild');")?;
    Ok(())
}

/// Whether the FTS5 mirror exists (created by migration v4).
fn fts_available(conn: &Connection) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='step_fts'",
        [],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Turn a free-text query into a safe FTS5 MATCH expression: each whitespace
/// token becomes a quoted prefix term (implicitly AND-ed), so incremental typing
/// matches and user punctuation can't break the query syntax.
fn fts_match_query(q: &str) -> String {
    q.split_whitespace()
        .map(|tok| format!("\"{}\"*", tok.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// v2 — collapse the fixed step fields (command/why/where_ctx/example/note) into
/// a single free-form markdown `body`. Existing rows are backfilled into sensible
/// markdown, then the legacy columns are dropped (SQLite >= 3.35).
fn migrate_v2(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("ALTER TABLE step ADD COLUMN body TEXT NOT NULL DEFAULT '';")?;

    let legacy: Vec<(i64, String, String, String, String, String)> = {
        let mut stmt =
            conn.prepare("SELECT id, command, why, where_ctx, example, note FROM step")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };

    for (id, command, why, where_ctx, example, note) in legacy {
        let body = legacy_body(&command, &why, &where_ctx, &example, &note);
        if !body.is_empty() {
            conn.execute("UPDATE step SET body = ?1 WHERE id = ?2", params![body, id])?;
        }
    }

    conn.execute_batch(
        "ALTER TABLE step DROP COLUMN command;
         ALTER TABLE step DROP COLUMN why;
         ALTER TABLE step DROP COLUMN where_ctx;
         ALTER TABLE step DROP COLUMN example;
         ALTER TABLE step DROP COLUMN note;",
    )?;
    Ok(())
}

/// Render the old fixed fields as a markdown body so no captured data is lost.
fn legacy_body(command: &str, why: &str, where_ctx: &str, example: &str, note: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !why.is_empty() {
        parts.push(why.to_string());
    }
    if !command.is_empty() {
        parts.push(format!("```\n{command}\n```"));
    }
    let mut meta: Vec<String> = Vec::new();
    if !where_ctx.is_empty() {
        meta.push(format!("- **where:** {where_ctx}"));
    }
    if !example.is_empty() {
        meta.push(format!("- **example:** {example}"));
    }
    if !meta.is_empty() {
        parts.push(meta.join("\n"));
    }
    if !note.is_empty() {
        parts.push(format!("> {note}"));
    }
    parts.join("\n\n")
}

// ----------------------------------------------------------------------------
// Wire types (serialized to the UI)
// ----------------------------------------------------------------------------

#[derive(Serialize)]
pub struct Step {
    pub id: i64,
    #[serde(rename = "runbookId")]
    pub runbook_id: i64,
    pub position: i64,
    /// Optional short label for navigation/search; may be empty.
    pub title: String,
    /// Free-form markdown — the actual content of the step.
    pub body: String,
    /// Replay progress: whether this step is checked off (docs D10). False for
    /// steps with no `step_progress` row (the LEFT JOIN COALESCEs to 0).
    pub done: bool,
}

#[derive(Serialize)]
pub struct Runbook {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    /// Optional absolute path this runbook is pinned to (docs D15). Used as the
    /// working directory for executed commands; "" when unpinned. Empty in list
    /// views; populated by `get_runbook`.
    #[serde(rename = "projectDir")]
    pub project_dir: String,
    /// Empty in list views; populated by `get_runbook`.
    pub steps: Vec<Step>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

/// New-step payload from the UI. Both fields optional-ish: `title` may be empty,
/// `body` carries the markdown (docs/03-overlay-and-ux.md).
#[derive(Deserialize)]
pub struct StepInput {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
}

/// Partial update for a runbook — only present fields are written.
#[derive(Deserialize)]
pub struct RunbookPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    #[serde(rename = "projectDir")]
    pub project_dir: Option<String>,
}

/// Partial update for a step — only present fields are written.
#[derive(Deserialize)]
pub struct StepPatch {
    pub title: Option<String>,
    pub body: Option<String>,
}

// ----------------------------------------------------------------------------
// Row mapping
// ----------------------------------------------------------------------------

fn step_from_row(row: &Row) -> rusqlite::Result<Step> {
    Ok(Step {
        id: row.get("id")?,
        runbook_id: row.get("runbook_id")?,
        position: row.get("position")?,
        title: row.get("title")?,
        body: row.get("body")?,
        // `done` is supplied by a LEFT JOIN onto step_progress in get_runbook.
        done: row.get::<_, i64>("done")? != 0,
    })
}

// ----------------------------------------------------------------------------
// Tags
// ----------------------------------------------------------------------------

fn tags_for(conn: &Connection, runbook_id: i64) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tag t
         JOIN runbook_tag rt ON rt.tag_id = t.id
         WHERE rt.runbook_id = ?1
         ORDER BY t.name",
    )?;
    let rows = stmt.query_map([runbook_id], |r| r.get::<_, String>(0))?;
    rows.collect()
}

/// Replace a runbook's tag set: upsert each tag name, then rewrite the links.
fn set_tags(conn: &Connection, runbook_id: i64, tags: &[String]) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM runbook_tag WHERE runbook_id = ?1", [runbook_id])?;
    for raw in tags {
        let name = raw.trim();
        if name.is_empty() {
            continue;
        }
        conn.execute("INSERT OR IGNORE INTO tag(name) VALUES (?1)", [name])?;
        let tag_id: i64 =
            conn.query_row("SELECT id FROM tag WHERE name = ?1", [name], |r| r.get(0))?;
        conn.execute(
            "INSERT OR IGNORE INTO runbook_tag(runbook_id, tag_id) VALUES (?1, ?2)",
            params![runbook_id, tag_id],
        )?;
    }
    Ok(())
}

// ----------------------------------------------------------------------------
// Runbook CRUD
// ----------------------------------------------------------------------------

/// A runbook header row before tags/steps are hydrated.
type RunbookRow = (i64, String, String, String, String);

/// List runbooks (without steps). With no `query`, returns all newest-first.
/// With a query, uses ranked FTS5 search over step title/body when available
/// (falling back to `LIKE`), and always also matches the runbook
/// title/description/tags.
pub fn list_runbooks(conn: &Connection, query: Option<&str>) -> rusqlite::Result<Vec<Runbook>> {
    let q = query.map(str::trim).filter(|s| !s.is_empty());

    let rows: Vec<RunbookRow> = match q {
        None => fetch_all(conn)?,
        Some(q) => {
            let m = fts_match_query(q);
            if !m.is_empty() && fts_available(conn)? {
                fetch_fts(conn, &m, &format!("%{q}%"))?
            } else {
                fetch_like(conn, &format!("%{q}%"))?
            }
        }
    };

    let mut out = Vec::with_capacity(rows.len());
    for (id, title, description, created_at, updated_at) in rows {
        out.push(Runbook {
            id,
            title,
            description,
            tags: tags_for(conn, id)?,
            // Pinned dir is only needed in the detail view; left empty in lists
            // (like `steps`) to keep the list/FTS queries untouched.
            project_dir: String::new(),
            steps: Vec::new(),
            created_at,
            updated_at,
        });
    }
    Ok(out)
}

fn row_to_header(r: &Row) -> rusqlite::Result<RunbookRow> {
    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
}

fn fetch_all(conn: &Connection) -> rusqlite::Result<Vec<RunbookRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, description, created_at, updated_at
         FROM runbook ORDER BY updated_at DESC, id DESC",
    )?;
    let rows = stmt
        .query_map([], row_to_header)?
        .collect::<rusqlite::Result<Vec<RunbookRow>>>()?;
    Ok(rows)
}

/// LIKE fallback (no FTS5): match runbook title/description/tags and step
/// title/body, newest-first.
fn fetch_like(conn: &Connection, like: &str) -> rusqlite::Result<Vec<RunbookRow>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT r.id, r.title, r.description, r.created_at, r.updated_at
         FROM runbook r
         LEFT JOIN runbook_tag rt ON rt.runbook_id = r.id
         LEFT JOIN tag t ON t.id = rt.tag_id
         WHERE r.title LIKE ?1
            OR r.description LIKE ?1
            OR t.name LIKE ?1
            OR EXISTS (
                 SELECT 1 FROM step s
                 WHERE s.runbook_id = r.id
                   AND (s.title LIKE ?1 OR s.body LIKE ?1)
               )
         ORDER BY r.updated_at DESC, r.id DESC",
    )?;
    let rows = stmt
        .query_map(params![like], row_to_header)?
        .collect::<rusqlite::Result<Vec<RunbookRow>>>()?;
    Ok(rows)
}

/// FTS5 ranked search: runbooks whose steps match (best bm25 score) plus those
/// whose title/description/tags match. Title matches rank first, then step
/// relevance (bm25 ascending = better), then recency.
fn fetch_fts(conn: &Connection, match_expr: &str, like: &str) -> rusqlite::Result<Vec<RunbookRow>> {
    let mut stmt = conn.prepare(
        "WITH matches AS MATERIALIZED (
             -- bm25() must be evaluated in a query that directly MATCHes the FTS
             -- table (no join / aggregate around it). MATERIALIZED stops SQLite
             -- from flattening this CTE back into the join below.
             SELECT step_fts.rowid AS sid, bm25(step_fts) AS score
             FROM step_fts
             WHERE step_fts MATCH ?1
         ),
         step_hits AS (
             -- ...then fold the materialized scores up to the parent runbook.
             SELECT s.runbook_id AS rid, MIN(m.score) AS score
             FROM matches m JOIN step s ON s.id = m.sid
             GROUP BY s.runbook_id
         )
         SELECT r.id, r.title, r.description, r.created_at, r.updated_at
         FROM runbook r
         LEFT JOIN step_hits sh ON sh.rid = r.id
         WHERE sh.rid IS NOT NULL
            OR r.id IN (
                 SELECT r2.id FROM runbook r2
                 LEFT JOIN runbook_tag rt ON rt.runbook_id = r2.id
                 LEFT JOIN tag t ON t.id = rt.tag_id
                 WHERE r2.title LIKE ?2 OR r2.description LIKE ?2 OR t.name LIKE ?2
               )
         ORDER BY (CASE WHEN r.title LIKE ?2 THEN 0 ELSE 1 END),
                  (sh.score IS NULL),
                  sh.score,
                  r.updated_at DESC",
    )?;
    let rows = stmt
        .query_map(params![match_expr, like], row_to_header)?
        .collect::<rusqlite::Result<Vec<RunbookRow>>>()?;
    Ok(rows)
}

/// Fetch one runbook with its ordered steps and tags, or `None` if missing.
pub fn get_runbook(conn: &Connection, id: i64) -> rusqlite::Result<Option<Runbook>> {
    let base = conn
        .query_row(
            "SELECT title, description, created_at, updated_at, project_dir
             FROM runbook WHERE id = ?1",
            [id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?;

    let Some((title, description, created_at, updated_at, project_dir)) = base else {
        return Ok(None);
    };

    let mut stmt = conn.prepare(
        "SELECT s.id, s.runbook_id, s.position, s.title, s.body,
                COALESCE(sp.done, 0) AS done
         FROM step s
         LEFT JOIN step_progress sp ON sp.step_id = s.id
         WHERE s.runbook_id = ?1 ORDER BY s.position, s.id",
    )?;
    let steps = stmt
        .query_map([id], step_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(Some(Runbook {
        id,
        title,
        description,
        tags: tags_for(conn, id)?,
        project_dir,
        steps,
        created_at,
        updated_at,
    }))
}

pub fn create_runbook(
    conn: &Connection,
    title: &str,
    tags: Option<&[String]>,
    description: Option<&str>,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO runbook(title, description) VALUES (?1, ?2)",
        params![title, description.unwrap_or("")],
    )?;
    let id = conn.last_insert_rowid();
    if let Some(tags) = tags {
        set_tags(conn, id, tags)?;
    }
    Ok(id)
}

pub fn update_runbook(conn: &Connection, id: i64, patch: RunbookPatch) -> rusqlite::Result<()> {
    if let Some(title) = patch.title {
        conn.execute(
            "UPDATE runbook SET title = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![title, id],
        )?;
    }
    if let Some(description) = patch.description {
        conn.execute(
            "UPDATE runbook SET description = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![description, id],
        )?;
    }
    if let Some(tags) = patch.tags {
        set_tags(conn, id, &tags)?;
        conn.execute(
            "UPDATE runbook SET updated_at = datetime('now') WHERE id = ?1",
            [id],
        )?;
    }
    if let Some(project_dir) = patch.project_dir {
        conn.execute(
            "UPDATE runbook SET project_dir = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![project_dir, id],
        )?;
    }
    Ok(())
}

pub fn delete_runbook(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    // Steps and tag links cascade via the foreign keys (PRAGMA enabled in open).
    conn.execute("DELETE FROM runbook WHERE id = ?1", [id])?;
    Ok(())
}

// ----------------------------------------------------------------------------
// Step CRUD
// ----------------------------------------------------------------------------

/// Touch a runbook's `updated_at` so list ordering reflects step edits too.
fn touch_runbook(conn: &Connection, runbook_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE runbook SET updated_at = datetime('now') WHERE id = ?1",
        [runbook_id],
    )?;
    Ok(())
}

/// Append a step to the end of a runbook (position = current max + 1).
pub fn add_step(conn: &Connection, runbook_id: i64, step: StepInput) -> rusqlite::Result<i64> {
    let next_pos: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM step WHERE runbook_id = ?1",
        [runbook_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO step(runbook_id, position, title, body) VALUES (?1, ?2, ?3, ?4)",
        params![runbook_id, next_pos, step.title, step.body],
    )?;
    let id = conn.last_insert_rowid();
    touch_runbook(conn, runbook_id)?;
    Ok(id)
}

pub fn update_step(conn: &Connection, id: i64, patch: StepPatch) -> rusqlite::Result<()> {
    // Only write provided fields; COALESCE keeps the existing value otherwise.
    conn.execute(
        "UPDATE step SET
            title = COALESCE(?1, title),
            body  = COALESCE(?2, body),
            updated_at = datetime('now')
         WHERE id = ?3",
        params![patch.title, patch.body, id],
    )?;
    if let Some(runbook_id) = step_owner(conn, id)? {
        touch_runbook(conn, runbook_id)?;
    }
    Ok(())
}

/// The runbook a step belongs to, or `None` if the step id doesn't exist —
/// which also makes this a cheap step-existence check (used by `runebook-mcp` to
/// reject edits/deletes of a missing step instead of silently no-op'ing).
pub fn step_owner(conn: &Connection, step_id: i64) -> rusqlite::Result<Option<i64>> {
    conn.query_row("SELECT runbook_id FROM step WHERE id = ?1", [step_id], |r| {
        r.get(0)
    })
    .optional()
}

/// Cheap existence check for a runbook (no steps/tags hydrated) — used by
/// `runebook-mcp` to reject edits/deletes of a missing runbook up front.
///
/// `allow(dead_code)`: consumed by the sibling `mcp-server` crate (which includes
/// this file via `#[path]`), not by the Tauri app — so it reads as unused when
/// `db.rs` is compiled into the app. Keeping the SQL here keeps it single-sourced.
#[allow(dead_code)]
pub fn runbook_exists(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    Ok(conn
        .query_row("SELECT 1 FROM runbook WHERE id = ?1", [id], |_| Ok(()))
        .optional()?
        .is_some())
}

/// Rewrite step positions to match `ordered_ids` (index = new position) in a
/// single transaction.
pub fn reorder_steps(
    conn: &mut Connection,
    runbook_id: i64,
    ordered_ids: &[i64],
) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    for (pos, step_id) in ordered_ids.iter().enumerate() {
        tx.execute(
            "UPDATE step SET position = ?1 WHERE id = ?2 AND runbook_id = ?3",
            params![pos as i64, step_id, runbook_id],
        )?;
    }
    tx.execute(
        "UPDATE runbook SET updated_at = datetime('now') WHERE id = ?1",
        [runbook_id],
    )?;
    tx.commit()
}

pub fn delete_step(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    let owner = step_owner(conn, id)?;
    conn.execute("DELETE FROM step WHERE id = ?1", [id])?;
    if let Some(runbook_id) = owner {
        touch_runbook(conn, runbook_id)?;
    }
    Ok(())
}

// ----------------------------------------------------------------------------
// Settings (key/value)
// ----------------------------------------------------------------------------

pub fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM setting WHERE key = ?1", [key], |r| r.get(0))
        .optional()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO setting(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// Whether command execution is enabled (docs D11). The gate for `run_command`,
/// kept here (not just in the UI) so it's testable and can't be bypassed from
/// the frontend. Off unless the `allow_run` setting is exactly "1".
pub fn run_allowed(conn: &Connection) -> rusqlite::Result<bool> {
    Ok(get_setting(conn, "allow_run")?.as_deref() == Some("1"))
}

/// Append a step to `runbook_id`, or to a fresh "Untitled runbook" if none is
/// given. The "current runbook" landing flow is fleshed out in Phase 4; this is
/// the persistence primitive it builds on.
pub fn quick_add(
    conn: &Connection,
    step: StepInput,
    runbook_id: Option<i64>,
) -> rusqlite::Result<i64> {
    let runbook_id = match runbook_id {
        Some(id) => id,
        None => create_runbook(conn, "Untitled runbook", None, None)?,
    };
    add_step(conn, runbook_id, step)
}

// ----------------------------------------------------------------------------
// Replay progress (docs D10) — a per-step "done" flag, keyed by step id
// ----------------------------------------------------------------------------

/// Per-runbook replay progress summary, for the in-progress badge on cards.
#[derive(Serialize)]
pub struct Progress {
    #[serde(rename = "runbookId")]
    pub runbook_id: i64,
    pub done: i64,
    pub total: i64,
}

/// Progress for every runbook that has at least one step checked off. Runbooks
/// with no progress are omitted so the UI only badges in-progress ones.
pub fn list_progress(conn: &Connection) -> rusqlite::Result<Vec<Progress>> {
    let mut stmt = conn.prepare(
        "SELECT s.runbook_id,
                SUM(CASE WHEN sp.done = 1 THEN 1 ELSE 0 END) AS done,
                COUNT(*) AS total
         FROM step s
         LEFT JOIN step_progress sp ON sp.step_id = s.id
         GROUP BY s.runbook_id
         HAVING done > 0",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Progress {
            runbook_id: r.get(0)?,
            done: r.get(1)?,
            total: r.get(2)?,
        })
    })?;
    rows.collect()
}

/// Mark a step done / not-done (upsert one `step_progress` row).
pub fn set_step_done(conn: &Connection, step_id: i64, done: bool) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO step_progress(step_id, done, updated_at)
         VALUES (?1, ?2, datetime('now'))
         ON CONFLICT(step_id)
         DO UPDATE SET done = excluded.done, updated_at = datetime('now')",
        params![step_id, done as i64],
    )?;
    Ok(())
}

/// Clear all replay progress for a runbook (start the checklist over).
pub fn reset_progress(conn: &Connection, runbook_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM step_progress
         WHERE step_id IN (SELECT id FROM step WHERE runbook_id = ?1)",
        [runbook_id],
    )?;
    Ok(())
}

// ----------------------------------------------------------------------------
// Variable profiles (docs D12) — named value sets per runbook for {{vars}}.
// `data` is a JSON object of name->value; secrets are excluded by the UI.
// ----------------------------------------------------------------------------

/// Names of a runbook's saved variable profiles, alphabetical.
pub fn list_var_profiles(conn: &Connection, runbook_id: i64) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT name FROM var_profile WHERE runbook_id = ?1 ORDER BY name")?;
    let rows = stmt.query_map([runbook_id], |r| r.get(0))?;
    rows.collect()
}

/// The name->value map for one profile, or `None` if it doesn't exist.
pub fn get_var_profile(
    conn: &Connection,
    runbook_id: i64,
    name: &str,
) -> rusqlite::Result<Option<HashMap<String, String>>> {
    let json: Option<String> = conn
        .query_row(
            "SELECT data FROM var_profile WHERE runbook_id = ?1 AND name = ?2",
            params![runbook_id, name],
            |r| r.get(0),
        )
        .optional()?;
    Ok(json.map(|s| serde_json::from_str(&s).unwrap_or_default()))
}

/// Create or overwrite a profile. `values` should already exclude secrets.
pub fn save_var_profile(
    conn: &Connection,
    runbook_id: i64,
    name: &str,
    values: &HashMap<String, String>,
) -> rusqlite::Result<()> {
    let data = serde_json::to_string(values).unwrap_or_else(|_| "{}".to_string());
    conn.execute(
        "INSERT INTO var_profile(runbook_id, name, data) VALUES (?1, ?2, ?3)
         ON CONFLICT(runbook_id, name) DO UPDATE SET data = excluded.data",
        params![runbook_id, name, data],
    )?;
    Ok(())
}

pub fn delete_var_profile(conn: &Connection, runbook_id: i64, name: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM var_profile WHERE runbook_id = ?1 AND name = ?2",
        params![runbook_id, name],
    )?;
    Ok(())
}

// ----------------------------------------------------------------------------
// Backup & restore (the whole SQLite database is one portable file — D4)
// ----------------------------------------------------------------------------

/// Copy the live database to `dest` using SQLite's online backup API (safe to
/// run while the connection is open). Overwrites `dest` if it exists.
pub fn backup_to(conn: &Connection, dest: &std::path::Path) -> rusqlite::Result<()> {
    let mut out = Connection::open(dest)?;
    let backup = rusqlite::backup::Backup::new(conn, &mut out)?;
    backup.run_to_completion(1000, std::time::Duration::from_millis(0), None)
}

/// Replace the live database contents with those of the backup file at `src`.
/// Validates that `src` looks like a Runebook database first, then copies it
/// over the current connection in place — callers should reload afterwards.
pub fn restore_from(conn: &mut Connection, src: &std::path::Path) -> rusqlite::Result<()> {
    let source = Connection::open(src)?;
    let looks_valid: i64 = source.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='runbook'",
        [],
        |r| r.get(0),
    )?;
    if looks_valid == 0 {
        return Err(rusqlite::Error::InvalidParameterName(
            "not a Runebook database".to_string(),
        ));
    }
    let backup = rusqlite::backup::Backup::new(&source, conn)?;
    backup.run_to_completion(1000, std::time::Duration::from_millis(0), None)
}

/// Render a runbook to portable Markdown for `RUNBOOK.md` export / sharing.
/// Returns `None` if the runbook doesn't exist. Step bodies are already markdown,
/// so this is mostly framing: title heading, optional tags/description, then a
/// numbered `##` section per step with its body inline.
pub fn export_markdown(conn: &Connection, id: i64) -> rusqlite::Result<Option<String>> {
    let Some(rb) = get_runbook(conn, id)? else {
        return Ok(None);
    };

    let mut out = format!("# {}\n", rb.title);
    if !rb.tags.is_empty() {
        let tags = rb
            .tags
            .iter()
            .map(|t| format!("#{t}"))
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&format!("\n_Tags: {tags}_\n"));
    }
    if !rb.description.trim().is_empty() {
        out.push_str(&format!("\n{}\n", rb.description.trim()));
    }
    for (i, s) in rb.steps.iter().enumerate() {
        let n = i + 1;
        let heading = if s.title.trim().is_empty() {
            format!("Step {n}")
        } else {
            s.title.trim().to_string()
        };
        out.push_str(&format!("\n## {n}. {heading}\n"));
        if !s.body.trim().is_empty() {
            out.push_str(&format!("\n{}\n", s.body.trim_end()));
        }
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A migrated in-memory database, exercising the full migration chain.
    fn mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        migrate(&conn).unwrap();
        conn
    }

    fn step(title: &str, body: &str) -> StepInput {
        StepInput {
            title: title.to_string(),
            body: body.to_string(),
        }
    }

    #[test]
    fn fts5_compiled_and_ranks_results() {
        let conn = mem();
        assert!(
            fts_available(&conn).unwrap(),
            "FTS5 should be compiled into bundled SQLite — otherwise search degrades to LIKE"
        );

        let rb = create_runbook(&conn, "Deploy via SSH", None, None).unwrap();
        add_step(&conn, rb, step("SSH into prod", "```\nssh deploy@host\n```")).unwrap();
        create_runbook(&conn, "Unrelated", None, None).unwrap();

        let hits = list_runbooks(&conn, Some("ssh")).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, rb);

        assert!(list_runbooks(&conn, Some("zzznotpresent")).unwrap().is_empty());
        // Empty query lists everything.
        assert_eq!(list_runbooks(&conn, None).unwrap().len(), 2);
    }

    #[test]
    fn triggers_keep_fts_in_sync() {
        let conn = mem();
        let rb = create_runbook(&conn, "R", None, None).unwrap();
        let s = add_step(&conn, rb, step("", "alpha bravo")).unwrap();
        assert_eq!(list_runbooks(&conn, Some("bravo")).unwrap().len(), 1);

        update_step(
            &conn,
            s,
            StepPatch {
                title: None,
                body: Some("charlie".to_string()),
            },
        )
        .unwrap();
        assert!(list_runbooks(&conn, Some("bravo")).unwrap().is_empty());
        assert_eq!(list_runbooks(&conn, Some("charlie")).unwrap().len(), 1);

        delete_step(&conn, s).unwrap();
        assert!(list_runbooks(&conn, Some("charlie")).unwrap().is_empty());
    }

    #[test]
    fn search_matches_tags_and_title() {
        let conn = mem();
        let rb = create_runbook(
            &conn,
            "Postgres rotation",
            Some(&["db".to_string(), "secrets".to_string()]),
            None,
        )
        .unwrap();
        // Matches by tag even with no step body hit.
        let by_tag = list_runbooks(&conn, Some("secrets")).unwrap();
        assert_eq!(by_tag.len(), 1);
        assert_eq!(by_tag[0].id, rb);
        assert_eq!(by_tag[0].tags, vec!["db".to_string(), "secrets".to_string()]);

        // Matches by runbook title.
        assert_eq!(list_runbooks(&conn, Some("postgres")).unwrap().len(), 1);
    }

    #[test]
    fn export_markdown_renders_runbook() {
        let conn = mem();
        let rb = create_runbook(&conn, "Deploy", Some(&["ops".to_string()]), None).unwrap();
        add_step(&conn, rb, step("SSH in", "```\nssh host\n```")).unwrap();
        add_step(&conn, rb, step("", "just a note")).unwrap();

        let md = export_markdown(&conn, rb).unwrap().unwrap();
        assert!(md.starts_with("# Deploy\n"));
        assert!(md.contains("_Tags: #ops_"));
        assert!(md.contains("## 1. SSH in"));
        assert!(md.contains("```\nssh host\n```"));
        assert!(md.contains("## 2. Step 2")); // untitled step gets a fallback heading
        assert!(md.contains("just a note"));

        assert!(export_markdown(&conn, 9999).unwrap().is_none());
    }

    #[test]
    fn backup_and_restore_roundtrip() {
        let src = mem();
        let rb = create_runbook(&src, "Backed up", Some(&["x".to_string()]), None).unwrap();
        add_step(&src, rb, step("one", "alpha")).unwrap();

        let path =
            std::env::temp_dir().join(format!("runebook_backup_test_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        backup_to(&src, &path).unwrap();
        assert!(path.exists());

        // A fresh database, then restore from the backup.
        let mut dest = mem();
        assert!(list_runbooks(&dest, None).unwrap().is_empty());
        restore_from(&mut dest, &path).unwrap();

        let all = list_runbooks(&dest, None).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].title, "Backed up");
        // FTS still works after restore (search hits the restored step).
        assert_eq!(list_runbooks(&dest, Some("alpha")).unwrap().len(), 1);

        // Restoring a non-Runebook file is rejected.
        let junk = std::env::temp_dir().join(format!("runebook_junk_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&junk);
        Connection::open(&junk).unwrap(); // empty db, no runbook table
        assert!(restore_from(&mut dest, &junk).is_err());

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&junk);
    }

    #[test]
    fn step_progress_tracks_and_resets() {
        let conn = mem();
        let rb = create_runbook(&conn, "Deploy", None, None).unwrap();
        let s1 = add_step(&conn, rb, step("one", "a")).unwrap();
        let s2 = add_step(&conn, rb, step("two", "b")).unwrap();

        // Fresh steps are not done, and there's no progress to report yet.
        let got = get_runbook(&conn, rb).unwrap().unwrap();
        assert!(got.steps.iter().all(|s| !s.done));
        assert!(list_progress(&conn).unwrap().is_empty());

        // Checking one step shows up on the step and in the summary.
        set_step_done(&conn, s1, true).unwrap();
        let got = get_runbook(&conn, rb).unwrap().unwrap();
        assert!(got.steps.iter().find(|s| s.id == s1).unwrap().done);
        assert!(!got.steps.iter().find(|s| s.id == s2).unwrap().done);
        let prog = list_progress(&conn).unwrap();
        assert_eq!(prog.len(), 1);
        assert_eq!(prog[0].runbook_id, rb);
        assert_eq!((prog[0].done, prog[0].total), (1, 2));

        // Unchecking, then reset, both clear progress.
        set_step_done(&conn, s1, false).unwrap();
        assert!(list_progress(&conn).unwrap().is_empty());
        set_step_done(&conn, s1, true).unwrap();
        set_step_done(&conn, s2, true).unwrap();
        assert_eq!(list_progress(&conn).unwrap()[0].done, 2);
        reset_progress(&conn, rb).unwrap();
        assert!(list_progress(&conn).unwrap().is_empty());
        assert!(get_runbook(&conn, rb).unwrap().unwrap().steps.iter().all(|s| !s.done));

        // Deleting a step cascades its progress row away.
        set_step_done(&conn, s1, true).unwrap();
        delete_step(&conn, s1).unwrap();
        assert_eq!(list_progress(&conn).unwrap().len(), 0);
    }

    #[test]
    fn var_profiles_crud_roundtrip() {
        let conn = mem();
        let rb = create_runbook(&conn, "Deploy", None, None).unwrap();
        assert!(list_var_profiles(&conn, rb).unwrap().is_empty());
        assert!(get_var_profile(&conn, rb, "prod").unwrap().is_none());

        let mut prod = HashMap::new();
        prod.insert("host".to_string(), "prod-1".to_string());
        prod.insert("user".to_string(), "deploy".to_string());
        save_var_profile(&conn, rb, "prod", &prod).unwrap();

        let mut staging = HashMap::new();
        staging.insert("host".to_string(), "stg-1".to_string());
        save_var_profile(&conn, rb, "staging", &staging).unwrap();

        assert_eq!(
            list_var_profiles(&conn, rb).unwrap(),
            vec!["prod".to_string(), "staging".to_string()]
        );
        let got = get_var_profile(&conn, rb, "prod").unwrap().unwrap();
        assert_eq!(got.get("host").map(String::as_str), Some("prod-1"));
        assert_eq!(got.get("user").map(String::as_str), Some("deploy"));

        // Saving the same name overwrites (acts as update).
        let mut prod2 = HashMap::new();
        prod2.insert("host".to_string(), "prod-2".to_string());
        save_var_profile(&conn, rb, "prod", &prod2).unwrap();
        let got = get_var_profile(&conn, rb, "prod").unwrap().unwrap();
        assert_eq!(got.get("host").map(String::as_str), Some("prod-2"));
        assert_eq!(got.get("user"), None); // replaced, not merged

        delete_var_profile(&conn, rb, "prod").unwrap();
        assert_eq!(list_var_profiles(&conn, rb).unwrap(), vec!["staging".to_string()]);

        // Profiles cascade away with their runbook.
        delete_runbook(&conn, rb).unwrap();
        assert!(list_var_profiles(&conn, rb).unwrap().is_empty());
    }

    #[test]
    fn run_gate_defaults_off_and_tracks_setting() {
        let conn = mem();
        assert!(!run_allowed(&conn).unwrap(), "execution must be off until enabled");
        set_setting(&conn, "allow_run", "0").unwrap();
        assert!(!run_allowed(&conn).unwrap());
        set_setting(&conn, "allow_run", "1").unwrap();
        assert!(run_allowed(&conn).unwrap());
        // Anything other than exactly "1" is off (no accidental truthiness).
        set_setting(&conn, "allow_run", "true").unwrap();
        assert!(!run_allowed(&conn).unwrap());
    }

    #[test]
    fn project_dir_pins_and_clears() {
        let conn = mem();
        let rb = create_runbook(&conn, "Deploy", None, None).unwrap();
        // Unpinned by default.
        assert_eq!(get_runbook(&conn, rb).unwrap().unwrap().project_dir, "");

        update_runbook(
            &conn,
            rb,
            RunbookPatch {
                title: None,
                description: None,
                tags: None,
                project_dir: Some("/home/me/project".to_string()),
            },
        )
        .unwrap();
        assert_eq!(
            get_runbook(&conn, rb).unwrap().unwrap().project_dir,
            "/home/me/project"
        );

        // Clearing the pin sets it back to empty.
        update_runbook(
            &conn,
            rb,
            RunbookPatch {
                title: None,
                description: None,
                tags: None,
                project_dir: Some(String::new()),
            },
        )
        .unwrap();
        assert_eq!(get_runbook(&conn, rb).unwrap().unwrap().project_dir, "");
    }
}
