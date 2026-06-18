//! `runebook-mcp` — a Model Context Protocol server for Runebook.
//!
//! Runebook stores your runbooks (named procedures) and free-form markdown notes
//! in a single local SQLite file. This binary exposes that database to MCP
//! clients — Claude Code, Cursor, and any other tool that can spawn a stdio MCP
//! server — so an AI agent can search your saved procedures, read their steps,
//! and capture new ones while you work. See docs/06-mcp-server.md.
//!
//! ## Design
//!
//! - **Same data, one source of truth.** The whole data layer (schema,
//!   migrations, FTS5 search, CRUD) is Runebook's own `db.rs`, included verbatim
//!   via `#[path]`. There is no second copy of the SQL to drift out of sync.
//! - **Direct DB access, no running app required.** The server opens the same
//!   file the overlay app uses (`<app-data>/runebook.db`); WAL + busy_timeout
//!   (set in `db::open`) let both processes share it.
//! - **Hand-rolled stdio JSON-RPC.** MCP over stdio is newline-delimited
//!   JSON-RPC 2.0. The loop below is synchronous (rusqlite is synchronous and
//!   every operation is a quick local query), so there's no async runtime and
//!   the dependency tree stays tiny. stdout is the protocol channel — all
//!   diagnostics go to stderr.

#![forbid(unsafe_code)]

// Reuse Runebook's data layer verbatim. `allow(dead_code)` because the MCP server
// exposes a subset of db.rs's functions (e.g. backup/restore stay app-only).
#[path = "../../src-tauri/src/db.rs"]
#[allow(dead_code)]
mod db;

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use rusqlite::Connection;
use serde_json::{json, Value};

/// Reported to clients in `initialize`.
const SERVER_NAME: &str = "runebook";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
/// MCP protocol versions we understand. `initialize` echoes the client's
/// requested version when it's one of these (for compat), else answers with our
/// latest — `SUPPORTED_PROTOCOLS[0]`. Never claim a version we don't know.
const SUPPORTED_PROTOCOLS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];
/// The Tauri bundle identifier — the app's data dir is `<data>/<identifier>`.
const APP_IDENTIFIER: &str = "com.runebook.app";
/// When set to a non-empty value other than `0`/`false`, the server advertises
/// and serves only the read-only tools (search / read / export) — never create,
/// update, or delete. Lets you connect an agent that can't mutate your runbooks.
const READONLY_ENV: &str = "RUNEBOOK_MCP_READONLY";

fn main() {
    if std::env::args().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }

    let db_path = resolve_db_path();
    // The app normally creates this dir, but the MCP server may run first.
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let conn = match db::open(&db_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "[runebook-mcp] failed to open database at {}: {e}",
                db_path.display()
            );
            std::process::exit(1);
        }
    };
    let read_only = read_only_from_env();
    eprintln!(
        "[runebook-mcp] ready — serving '{SERVER_NAME}'{} over stdio (db: {})",
        if read_only { " (read-only)" } else { "" },
        db_path.display()
    );

    serve(&conn, read_only);
}

/// Whether `RUNEBOOK_MCP_READONLY` requests read-only mode.
fn read_only_from_env() -> bool {
    std::env::var(READONLY_ENV)
        .map(|v| env_is_truthy(&v))
        .unwrap_or(false)
}

/// Truthy for any value except the usual falsey words (empty, `0`, `false`, `no`,
/// `off`, case-insensitive), so `=1`/`=true`/`=yes` enable it and `=0`/`=off` don't.
fn env_is_truthy(v: &str) -> bool {
    let v = v.trim().to_ascii_lowercase();
    !matches!(v.as_str(), "" | "0" | "false" | "no" | "off")
}

fn print_help() {
    println!(
        "runebook-mcp {SERVER_VERSION} — MCP (stdio) server for the Runebook database

USAGE:
    runebook-mcp [--db <PATH>]

The server speaks the Model Context Protocol over stdin/stdout; it is meant to be
spawned by an MCP client (Claude Code, Cursor, …), not run interactively.

DATABASE PATH (highest precedence first):
    --db <PATH>            explicit path
    RUNEBOOK_DB=<PATH>     environment variable
    default                $XDG_DATA_HOME/{APP_IDENTIFIER}/runebook.db
                           (falls back to ~/.local/share/{APP_IDENTIFIER}/runebook.db)"
    );
}

/// Resolve the database path: `--db` flag, then `RUNEBOOK_DB`, then the app's
/// default data-dir location.
fn resolve_db_path() -> PathBuf {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--db" {
            if let Some(p) = args.next() {
                return PathBuf::from(p);
            }
        } else if let Some(rest) = a.strip_prefix("--db=") {
            return PathBuf::from(rest);
        }
    }
    if let Some(p) = std::env::var_os("RUNEBOOK_DB") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    default_db_path()
}

/// `<app-data>/<identifier>/runebook.db`, matching what Tauri's `app_data_dir()`
/// resolves to on Linux.
fn default_db_path() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
            home.join(".local").join("share")
        });
    base.join(APP_IDENTIFIER).join("runebook.db")
}

// ----------------------------------------------------------------------------
// stdio JSON-RPC transport
// ----------------------------------------------------------------------------

/// Read newline-delimited JSON-RPC messages from stdin until EOF, replying on
/// stdout. Blocks the calling thread for the process lifetime.
fn serve(conn: &Connection, read_only: bool) {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break, // stdin closed/errored — client is gone.
        };
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                write_msg(&mut out, &parse_error());
                continue;
            }
        };
        // JSON-RPC permits batches (arrays); handle each element.
        if let Some(arr) = msg.as_array() {
            for item in arr {
                if let Some(resp) = handle_message(conn, read_only, item) {
                    write_msg(&mut out, &resp);
                }
            }
        } else if let Some(resp) = handle_message(conn, read_only, &msg) {
            write_msg(&mut out, &resp);
        }
    }
}

fn write_msg(out: &mut impl Write, v: &Value) {
    if let Ok(s) = serde_json::to_string(v) {
        let _ = writeln!(out, "{s}");
        let _ = out.flush();
    }
}

/// Dispatch one JSON-RPC message. Returns `Some(response)` for requests (those
/// with an `id`) and `None` for notifications (no `id` → no reply).
fn handle_message(conn: &Connection, read_only: bool, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let is_notification = id.is_none();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);

    match method {
        "initialize" => Some(success(id, initialize_result(&params))),
        "ping" => Some(success(id, json!({}))),
        "tools/list" => Some(success(id, json!({ "tools": tool_definitions(read_only) }))),
        "tools/call" => Some(success(id, call_tool(conn, read_only, &params))),
        // Notifications we simply acknowledge by doing nothing.
        "notifications/initialized" | "notifications/cancelled" => None,
        _ => {
            if is_notification {
                None
            } else {
                Some(error_resp(id, -32601, &format!("Method not found: {method}")))
            }
        }
    }
}

fn success(id: Option<Value>, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "result": result })
}

fn error_resp(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "error": { "code": code, "message": message } })
}

fn parse_error() -> Value {
    json!({ "jsonrpc": "2.0", "id": Value::Null, "error": { "code": -32700, "message": "Parse error" } })
}

fn initialize_result(params: &Value) -> Value {
    // Echo the client's protocol version only if we actually support it; else
    // answer with our latest. Never claim a version we don't know.
    let requested = params.get("protocolVersion").and_then(Value::as_str);
    let protocol = match requested {
        Some(v) if SUPPORTED_PROTOCOLS.contains(&v) => v,
        _ => SUPPORTED_PROTOCOLS[0],
    };
    json!({
        "protocolVersion": protocol,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION },
        "instructions": "Runebook is the user's personal scratchpad of runbooks \
            (named, step-by-step procedures) and markdown notes. Use list_runbooks \
            to search before reinventing a procedure; get_runbook to read one; and \
            create_runbook / add_step to capture a workflow the user just performed."
    })
}

// ----------------------------------------------------------------------------
// Tools
// ----------------------------------------------------------------------------

/// Names of the tools that mutate the database. In read-only mode these are
/// neither advertised (`tool_definitions`) nor executed (`dispatch`).
const MUTATING_TOOLS: &[&str] = &[
    "create_runbook",
    "update_runbook",
    "delete_runbook",
    "add_step",
    "update_step",
    "delete_step",
];

/// The `tools/list` payload: each tool's name, description, and JSON-Schema for
/// its arguments. Kept verbose so an agent picks the right tool unprompted. In
/// read-only mode the mutating tools are omitted entirely.
fn tool_definitions(read_only: bool) -> Value {
    // Read tools — always available.
    let mut tools = vec![
        json!({
            "name": "list_runbooks",
            "description": "List or search the user's runbooks (named procedures) and notes. \
                With `query`, runs ranked full-text search across runbook titles, descriptions, \
                tags, and step bodies; without it, returns everything newest-first. Returns \
                runbook metadata only (no step bodies) — call get_runbook for the contents.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Optional search terms; omit to list all." }
                }
            }
        }),
        json!({
            "name": "get_runbook",
            "description": "Fetch one runbook by id, including all of its steps. Each step has an \
                optional title and a free-form markdown body (commands live in fenced code blocks). \
                Use export_runbook_markdown instead if you just want a single readable document.",
            "inputSchema": {
                "type": "object",
                "properties": { "id": { "type": "integer", "description": "Runbook id." } },
                "required": ["id"]
            }
        }),
        json!({
            "name": "export_runbook_markdown",
            "description": "Render a runbook to a single portable Markdown document (title heading, \
                tags/description, then a numbered section per step). Ideal for dropping into a \
                repo's RUNBOOK.md or pasting into a chat.",
            "inputSchema": {
                "type": "object",
                "properties": { "id": { "type": "integer", "description": "Runbook id." } },
                "required": ["id"]
            }
        }),
    ];

    if read_only {
        return Value::Array(tools);
    }

    // Mutating tools — hidden in read-only mode so an agent never attempts them.
    tools.extend([
        json!({
            "name": "create_runbook",
            "description": "Create a new, empty runbook and return its id. Add steps afterward with \
                add_step.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Runbook title, e.g. \"Deploy via SSH\"." },
                    "description": { "type": "string", "description": "Optional one-line description." },
                    "tags": { "type": "array", "items": { "type": "string" }, "description": "Optional tags for filtering." }
                },
                "required": ["title"]
            }
        }),
        json!({
            "name": "update_runbook",
            "description": "Update a runbook's title, description, and/or tags. Only the fields you \
                pass are changed; passing `tags` replaces the whole tag set.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer" },
                    "title": { "type": "string" },
                    "description": { "type": "string" },
                    "tags": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["id"]
            }
        }),
        json!({
            "name": "delete_runbook",
            "description": "Delete a runbook and all of its steps. Destructive and irreversible.",
            "inputSchema": {
                "type": "object",
                "properties": { "id": { "type": "integer" } },
                "required": ["id"]
            }
        }),
        json!({
            "name": "add_step",
            "description": "Append a step to a runbook and return the new step id. A step is an \
                optional short title plus a free-form markdown `body` (provide at least one) — put \
                shell commands in fenced ``` code blocks so they stay one-click-copyable in the app.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "runbook_id": { "type": "integer" },
                    "title": { "type": "string", "description": "Optional short label for the step." },
                    "body": { "type": "string", "description": "Markdown content of the step. Optional only if `title` is given." }
                },
                "required": ["runbook_id"]
            }
        }),
        json!({
            "name": "update_step",
            "description": "Update a step's title and/or body by step id. Only provided fields change.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer" },
                    "title": { "type": "string" },
                    "body": { "type": "string" }
                },
                "required": ["id"]
            }
        }),
        json!({
            "name": "delete_step",
            "description": "Delete a single step by id. Destructive and irreversible.",
            "inputSchema": {
                "type": "object",
                "properties": { "id": { "type": "integer" } },
                "required": ["id"]
            }
        }),
    ]);

    Value::Array(tools)
}

/// Run a `tools/call`. Always returns a tool-result object; failures are reported
/// in-band with `isError: true` (per MCP) so the model sees what went wrong.
fn call_tool(conn: &Connection, read_only: bool, params: &Value) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    // Defense in depth: even though read-only mode hides mutating tools from
    // tools/list, refuse them here too in case a client calls one anyway.
    if read_only && MUTATING_TOOLS.contains(&name) {
        return json!({
            "content": [ { "type": "text", "text":
                format!("Server is read-only ({READONLY_ENV}); '{name}' is disabled.") } ],
            "isError": true
        });
    }
    match dispatch(conn, name, &args) {
        Ok(text) => json!({ "content": [ { "type": "text", "text": text } ] }),
        Err(msg) => json!({ "content": [ { "type": "text", "text": msg } ], "isError": true }),
    }
}

fn dispatch(conn: &Connection, name: &str, args: &Value) -> Result<String, String> {
    match name {
        "list_runbooks" => {
            let query = args
                .get("query")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let rbs = db::list_runbooks(conn, query).map_err(|e| e.to_string())?;
            // Trimmed view — list omits step bodies (get_runbook returns them).
            let view: Vec<Value> = rbs
                .iter()
                .map(|r| {
                    json!({
                        "id": r.id,
                        "title": r.title,
                        "description": r.description,
                        "tags": r.tags,
                        "createdAt": r.created_at,
                        "updatedAt": r.updated_at
                    })
                })
                .collect();
            Ok(pretty(&json!(view)))
        }
        "get_runbook" => {
            let id = require_i64(args, "id")?;
            match db::get_runbook(conn, id).map_err(|e| e.to_string())? {
                Some(rb) => Ok(pretty(&json!({
                    "id": rb.id,
                    "title": rb.title,
                    "description": rb.description,
                    "tags": rb.tags,
                    "createdAt": rb.created_at,
                    "updatedAt": rb.updated_at,
                    "steps": rb.steps.iter().map(|s| json!({
                        "id": s.id,
                        "position": s.position,
                        "title": s.title,
                        "body": s.body,
                        "done": s.done
                    })).collect::<Vec<_>>()
                }))),
                None => Err(format!("No runbook with id {id}.")),
            }
        }
        "export_runbook_markdown" => {
            let id = require_i64(args, "id")?;
            match db::export_markdown(conn, id).map_err(|e| e.to_string())? {
                Some(md) => Ok(md),
                None => Err(format!("No runbook with id {id}.")),
            }
        }
        "create_runbook" => {
            let title = require_str(args, "title")?;
            let description = args.get("description").and_then(Value::as_str);
            let tags = parse_tags(args);
            let id = db::create_runbook(conn, title, tags.as_deref(), description)
                .map_err(|e| e.to_string())?;
            Ok(json!({ "ok": true, "id": id }).to_string())
        }
        "update_runbook" => {
            let id = require_i64(args, "id")?;
            ensure_runbook(conn, id)?;
            let patch = db::RunbookPatch {
                title: opt_string(args, "title"),
                description: opt_string(args, "description"),
                tags: parse_tags(args),
                // Project pinning is an app-side concept (the run cwd); the MCP
                // server leaves it untouched. See src-tauri/src/db.rs (D15).
                project_dir: None,
            };
            db::update_runbook(conn, id, patch).map_err(|e| e.to_string())?;
            Ok(json!({ "ok": true, "id": id }).to_string())
        }
        "delete_runbook" => {
            let id = require_i64(args, "id")?;
            ensure_runbook(conn, id)?;
            db::delete_runbook(conn, id).map_err(|e| e.to_string())?;
            Ok(json!({ "ok": true, "id": id }).to_string())
        }
        "add_step" => {
            let runbook_id = require_i64(args, "runbook_id")?;
            ensure_runbook(conn, runbook_id)?;
            // A step is an optional title + optional markdown body, but at least
            // one must be non-empty — an empty step is never intended.
            let title = args.get("title").and_then(Value::as_str).unwrap_or("");
            let body = args.get("body").and_then(Value::as_str).unwrap_or("");
            if title.trim().is_empty() && body.trim().is_empty() {
                return Err("A step needs a title or a body (both were empty).".to_string());
            }
            let step = db::StepInput {
                title: title.to_string(),
                body: body.to_string(),
            };
            let id = db::add_step(conn, runbook_id, step).map_err(|e| e.to_string())?;
            Ok(json!({ "ok": true, "id": id }).to_string())
        }
        "update_step" => {
            let id = require_i64(args, "id")?;
            ensure_step(conn, id)?;
            let patch = db::StepPatch {
                title: opt_string(args, "title"),
                body: opt_string(args, "body"),
            };
            db::update_step(conn, id, patch).map_err(|e| e.to_string())?;
            Ok(json!({ "ok": true, "id": id }).to_string())
        }
        "delete_step" => {
            let id = require_i64(args, "id")?;
            ensure_step(conn, id)?;
            db::delete_step(conn, id).map_err(|e| e.to_string())?;
            Ok(json!({ "ok": true, "id": id }).to_string())
        }
        other => Err(format!("Unknown tool: {other}")),
    }
}

/// Error out if no runbook has this id — so update/delete report a missing
/// target instead of silently affecting 0 rows and returning `ok`. Uses the
/// lightweight `runbook_exists` (no steps/tags loaded).
fn ensure_runbook(conn: &Connection, id: i64) -> Result<(), String> {
    if db::runbook_exists(conn, id).map_err(|e| e.to_string())? {
        Ok(())
    } else {
        Err(format!("No runbook with id {id}."))
    }
}

/// Error out if no step has this id (uses `db::step_owner` as the existence check).
fn ensure_step(conn: &Connection, id: i64) -> Result<(), String> {
    match db::step_owner(conn, id).map_err(|e| e.to_string())? {
        Some(_) => Ok(()),
        None => Err(format!("No step with id {id}.")),
    }
}

// ----------------------------------------------------------------------------
// Argument helpers — lenient (accept stringified numbers) with clear errors.
// ----------------------------------------------------------------------------

fn require_i64(args: &Value, key: &str) -> Result<i64, String> {
    args.get(key)
        .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        .ok_or_else(|| format!("Missing or invalid required integer argument: '{key}'."))
}

fn require_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Missing or invalid required string argument: '{key}'."))
}

fn opt_string(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_string)
}

fn parse_tags(args: &Value) -> Option<Vec<String>> {
    args.get("tags").and_then(Value::as_array).map(|arr| {
        arr.iter()
            .filter_map(|t| t.as_str().map(str::to_string))
            .collect()
    })
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A fresh migrated database in a unique temp file. `db::open` needs a path
    /// (it sets WAL etc.), so tests use a real file rather than `:memory:`.
    fn temp_db() -> (Connection, std::path::PathBuf) {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("runebook_mcp_test_{}_{n}.db", std::process::id()));
        cleanup(&path);
        let conn = db::open(&path).unwrap();
        (conn, path)
    }

    /// Remove a temp DB and its WAL sidecars.
    fn cleanup(path: &std::path::Path) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    fn call(conn: &Connection, name: &str, args: Value) -> Result<String, String> {
        dispatch(conn, name, &args)
    }

    #[test]
    fn tool_list_filtered_by_read_only() {
        assert_eq!(tool_definitions(false).as_array().unwrap().len(), 9);
        let ro = tool_definitions(true);
        assert_eq!(ro.as_array().unwrap().len(), 3);
        for t in ro.as_array().unwrap() {
            let name = t["name"].as_str().unwrap();
            assert!(!MUTATING_TOOLS.contains(&name), "read-only leaked '{name}'");
        }
        // Guard against MUTATING_TOOLS drifting from dispatch: every name in the
        // list must be a real, dispatchable tool (not "Unknown tool"). Empty args
        // make each fail at argument validation before touching the DB.
        let (conn, path) = temp_db();
        for &m in MUTATING_TOOLS {
            let err = call(&conn, m, json!({})).unwrap_err();
            assert!(!err.starts_with("Unknown tool"), "'{m}' missing from dispatch");
        }
        cleanup(&path);
    }

    #[test]
    fn initialize_clamps_protocol_version() {
        let known = initialize_result(&json!({ "protocolVersion": "2025-03-26" }));
        assert_eq!(known["protocolVersion"], "2025-03-26");
        let unknown = initialize_result(&json!({ "protocolVersion": "1999-01-01" }));
        assert_eq!(unknown["protocolVersion"], SUPPORTED_PROTOCOLS[0]);
        let missing = initialize_result(&json!({}));
        assert_eq!(missing["protocolVersion"], SUPPORTED_PROTOCOLS[0]);
    }

    #[test]
    fn crud_round_trip_and_missing_ids() {
        let (conn, path) = temp_db();

        let created = call(&conn, "create_runbook", json!({ "title": "Deploy" })).unwrap();
        let rid = serde_json::from_str::<Value>(&created).unwrap()["id"]
            .as_i64()
            .unwrap();
        assert!(call(&conn, "get_runbook", json!({ "id": rid }))
            .unwrap()
            .contains("Deploy"));

        // add_step: title-only ok; fully-empty rejected; missing runbook rejected.
        assert!(call(&conn, "add_step", json!({ "runbook_id": rid, "title": "t" })).is_ok());
        assert!(call(&conn, "add_step", json!({ "runbook_id": rid })).is_err());
        assert!(call(&conn, "add_step", json!({ "runbook_id": 9999, "body": "x" }))
            .unwrap_err()
            .contains("No runbook with id 9999"));

        // update/delete of a missing id must error, not silently succeed.
        assert!(call(&conn, "update_runbook", json!({ "id": 9999, "title": "x" }))
            .unwrap_err()
            .contains("No runbook"));
        assert!(call(&conn, "delete_runbook", json!({ "id": 9999 }))
            .unwrap_err()
            .contains("No runbook"));
        assert!(call(&conn, "update_step", json!({ "id": 9999, "body": "x" }))
            .unwrap_err()
            .contains("No step"));
        assert!(call(&conn, "delete_step", json!({ "id": 9999 }))
            .unwrap_err()
            .contains("No step"));
        assert!(call(&conn, "get_runbook", json!({ "id": 9999 })).is_err());

        // Happy delete leaves it gone.
        assert!(call(&conn, "delete_runbook", json!({ "id": rid })).is_ok());
        assert!(call(&conn, "get_runbook", json!({ "id": rid })).is_err());

        cleanup(&path);
    }

    #[test]
    fn read_only_guard_blocks_mutations() {
        let (conn, path) = temp_db();

        let blocked = call_tool(
            &conn,
            true,
            &json!({ "name": "create_runbook", "arguments": { "title": "x" } }),
        );
        assert_eq!(blocked["isError"], json!(true));
        assert!(blocked["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("read-only"));
        // Nothing was created.
        assert_eq!(call(&conn, "list_runbooks", json!({})).unwrap(), "[]");
        // Read tools still work under read-only.
        let listed = call_tool(&conn, true, &json!({ "name": "list_runbooks", "arguments": {} }));
        assert!(listed.get("isError").is_none());

        cleanup(&path);
    }

    #[test]
    fn read_only_truthiness() {
        for t in ["1", "true", "yes", "on", "enabled", "TRUE"] {
            assert!(env_is_truthy(t), "'{t}' should be truthy");
        }
        for f in ["", " ", "0", "false", "FALSE", "no", "off", " off "] {
            assert!(!env_is_truthy(f), "'{f}' should be falsey");
        }
    }
}
