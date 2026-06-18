---
id: backup-restore
label: Database backup & restore
type: concept
community: Data & Storage
edges:
  - target: local-first-no-account
    relation: implements
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: rusqlite-data-layer
    relation: part_of
    confidence: EXTRACTED
    confidence_score: 1.0
---

Backlog item, built 2026-06-16. Settings → Data offers **Back up database…** and
**Restore…**, leaning on the whole DB being one portable SQLite file (D4 /
[[local-first-no-account]]).

- Backup: `db::backup_to` uses rusqlite's online **backup API** (`features =
  ["backup"]`) to copy the live DB to a save-dialog path — safe while the
  connection is open, overwrites the target.
- Restore: `db::restore_from` opens the chosen file, **validates** it has a
  `runbook` table (rejects non-Runebook files), then backs it up *over* the live
  connection in place; the UI resets selection and reloads. No reconnect needed.

Dialogs use `@tauri-apps/plugin-dialog` (`dialog:allow-save` + `dialog:allow-open`
in capabilities); the file I/O itself is in the Rust core. Roundtrip + reject-junk
covered by a unit test in `db.rs`. Cloud sync remains deferred.
