---
id: browse-list-refresh
label: Browse List Refresh on External Writes
type: gotcha
community: ux-and-capture
edges:
  - target: mcp-server
    relation: references
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: wal-concurrency
    relation: conceptually_related_to
    confidence: INFERRED
    confidence_score: 0.8
  - target: ipc-boundary
    relation: references
    confidence: INFERRED
    confidence_score: 0.7
---

A runbook created through the [[MCP server|mcp-server]] (or any external writer of
the shared SQLite file) does **not** appear in the running app's Browse view until
the list is re-queried. The write itself is fine — the row is committed to the same
`<app-data>/runebook.db` both processes share ([[wal-concurrency]]) — but the app's
`runbooks` array in `App.svelte` is only loaded by `loadRunbooks()` on `onMount` and
after the app's *own* edits/searches. `setMode("browse")` did **not** re-query, and
there is no DB watcher/polling, so externally-written rows stayed invisible until
restart.

**Symptom seen:** MCP `create_runbook` returned `{ok:true}`, row verified via
`sqlite3` (table is `runbook`, not `runbooks`), but the note was missing from Browse.

**Immediate workaround (no rebuild):** type any character in the Browse search box —
`on:input={() => loadRunbooks(search)}` forces a re-query. Or restart the app.

**Fix (2026-06-22):** `setMode()` now calls `loadRunbooks(search)` when entering
`"browse"`, so MCP-written runbooks show live. Requires an app restart/rebuild to
take effect in an already-running instance. A future improvement could watch the DB
file for true live refresh.
