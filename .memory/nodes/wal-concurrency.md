---
id: wal-concurrency
label: WAL for App + MCP Concurrency
type: decision
community: data-storage
edges:
  - target: mcp-server
    relation: caused_by
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: rusqlite-data-layer
    relation: part_of
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: backup-restore
    relation: references
    confidence: INFERRED
    confidence_score: 0.7
---

`db::open` runs the database in **WAL** journal mode with `PRAGMA busy_timeout =
5000`. Reason: once [[the MCP server|mcp-server]] exists, the Tauri app and
`runebook-mcp` open the *same* SQLite file at the same time. WAL lets readers and a
single writer proceed without blocking; `busy_timeout` makes a momentarily-locked
write wait up to 5s instead of failing with `SQLITE_BUSY`. Both are set in the one
shared `open()`, so the app and the MCP process get them identically.

Notes / gotchas:
- WAL is **persisted in the DB file header** (write/read version byte = 2) once any
  connection sets it — so it sticks for both processes.
- WAL creates `-wal` / `-shm` sidecar files while a connection is open; SQLite
  **checkpoints and removes them on clean close** of the last connection (so seeing
  no `-wal` after the process exits is normal, not a failure).
- Compatible with [[backup-restore]] — the online backup API checkpoints WAL, so
  D4 back up / restore still work.
- The in-memory test DBs call `migrate()` directly (not `open()`), so this PRAGMA
  doesn't affect the unit tests.
