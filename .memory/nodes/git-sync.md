---
id: git-sync
label: Git-backed Sync (export-only)
type: decision
community: data-storage
edges:
  - target: markdown-export
    relation: depends_on
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: sqlite-over-json
    relation: references
    confidence: EXTRACTED
    confidence_score: 0.8
  - target: local-first-no-account
    relation: conceptually_related_to
    confidence: INFERRED
    confidence_score: 0.7
---

Phase 8 / D16. Settings → Git sync writes every runbook to
`<dir>/runbooks/<id>-<slug>.md` via [[export_markdown|markdown-export]], then
`git init`/`add`/`commit` (optional `push`) in that folder. `git_sync(dir, push)`
is `#[tauri::command(async)]` (spawns git off the main thread).

Decisions:
- **One-way / export-only.** Never reads `.md` back; the SQLite DB stays the single
  source of truth ([[sqlite-over-json]]). Avoids a Markdown↔DB merge problem.
- **Deletions propagate WITHOUT data loss.** The dangerous first cut was
  `remove_dir_all(runbooks/)` — which would wipe a user's pre-existing files in
  that folder. Fixed to delete only our own `<digits>-*.md` exports
  (`is_export_filename`, unit-tested) then rewrite. `slugify` is also tested to
  never emit path separators (an export filename can't escape the dir).
- **`git init` if needed; push is opt-in** (network/auth only on "Sync & push").
- Shells out to `git` (a fixed binary, not arbitrary shell like
  [[executable-steps]]); not behind `allow_run`. A malicious repo's commit hooks
  could run — acceptable since the user chose the folder.
