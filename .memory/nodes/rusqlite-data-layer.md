---
id: rusqlite-data-layer
label: rusqlite data layer behind IPC
type: decision
community: Data & Storage
edges:
  - target: ipc-boundary
    relation: implements
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: sqlite-over-json
    relation: implements
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: markdown-step-model
    relation: references
    confidence: EXTRACTED
    confidence_score: 1.0
---

Phase 2 backs the database with **`rusqlite` (bundled SQLite)** in
`src-tauri/src/db.rs`, exposed through Rust `#[tauri::command]`s in `lib.rs` —
**not** `tauri-plugin-sql`, which the docs originally named. Reason: that plugin
runs SQL straight from the WebView, breaking the [[ipc-boundary]] rule that the
UI only ever calls Rust commands. rusqlite keeps all SQL in one place and matches
the documented IPC surface (`list_runbooks`, `get_runbook`, `create_runbook`,
`add_step`, `reorder_steps`, `quick_add`, …). `bundled` compiles SQLite from
source, so no system libsqlite is needed. Recorded as **D7** in
`docs/05-decisions.md`; supersedes the `tauri-plugin-sql` mentions in docs 01/02/04.

Connection: a single `Mutex<Connection>` in Tauri-managed state (rusqlite's
`Connection` isn't `Sync`); each command locks it briefly. DB opened in the
app-data dir at startup; `PRAGMA foreign_keys = ON` per connection so
`ON DELETE CASCADE` fires. Migrations are tracked by `PRAGMA user_version`
(v1 = fixed fields, v2 = [[markdown-step-model]] — collapse to title + markdown
`body`). The `copy_to_clipboard` command was added here too (Rust clipboard).
