---
id: mcp-server
label: MCP Server (runebook-mcp)
type: decision
community: architecture
edges:
  - target: rusqlite-data-layer
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: ipc-boundary
    relation: references
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: wal-concurrency
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: executable-steps
    relation: references
    confidence: EXTRACTED
    confidence_score: 1.0
---

Phase 7 / **D13**: `runebook-mcp` (in `mcp-server/`) is a **standalone stdio
Model Context Protocol server** that MCP clients (Claude Code, Cursor, …) spawn on
demand to read/write the runbooks. It opens `<app-data>/runebook.db` directly — no
running app, no network port — and **reuses [[the data layer|rusqlite-data-layer]]
verbatim** via `#[path = "../../src-tauri/src/db.rs"] mod db;`, so schema,
migrations, and FTS5 search are single-sourced. The binary has **no Tauri/webkit
deps** (just `rusqlite` + `serde` + `serde_json`), so it builds headless.

This does **not** break the [[UI-never-touches-DB rule|ipc-boundary]]: that governs
the *WebView*, which still goes through Rust IPC. The MCP server is a separate
trusted local process acting for the user, like the app's own Rust core. It
deliberately does **not** expose `run_command` ([[executable-steps]]) — execution
stays an explicit app-gated action, not a remotely-callable tool.

Protocol is a small **hand-rolled synchronous JSON-RPC stdio loop** (no async/`rmcp`
SDK), pairing naturally with synchronous rusqlite. Tools: `list_runbooks`,
`get_runbook`, `export_runbook_markdown`, and create/update/delete for runbooks +
steps. Tool failures return `isError: true` in-band.

Review-hardened behaviors (don't regress these):
- **update/delete of a missing id errors** ("No runbook/step with id N") instead of
  silently no-op'ing to `ok:true` — step existence reuses `db::step_owner` (made
  `pub` for this). The underlying `db.rs` UPDATE/DELETE still affect 0 rows silently;
  the guard lives in the MCP dispatch.
- `add_step` requires **title OR body** (rejects a fully-empty step).
- `initialize` echoes the client's `protocolVersion` only if known, else answers
  with the latest (`SUPPORTED_PROTOCOLS[0]`).
- **`RUNEBOOK_MCP_READONLY=1`** hides + refuses the mutating tools (search/read/export
  only) — for connecting an agent that must not change anything.

DB path: `--db` > `RUNEBOOK_DB` > default `~/.local/share/com.runebook.app/runebook.db`.
Full reference + client config: `docs/06-mcp-server.md`. Verified by `cargo build
--release` + end-to-end stdio smoke tests (normal + read-only).
