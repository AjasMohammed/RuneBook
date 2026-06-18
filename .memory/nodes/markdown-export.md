---
id: markdown-export
label: Markdown export
type: concept
community: UX & Capture
edges:
  - target: markdown-step-model
    relation: references
    confidence: EXTRACTED
    confidence_score: 1.0
  - target: ipc-boundary
    relation: implements
    confidence: EXTRACTED
    confidence_score: 1.0
---

Phase 5 export (done 2026-06-16). `db::export_markdown(id)` renders a runbook to
portable Markdown — `# title`, optional `_Tags: …_`, then a numbered `##` section
per step (untitled steps fall back to "Step N") with the step's markdown body
inline. Unit-tested in `db.rs`.

The Browse detail offers **Copy .md** (→ `copy_to_clipboard`) and **Save .md**.
Save uses `@tauri-apps/plugin-dialog`'s `save()` for the native path picker
(needs `dialog:allow-save` in capabilities), then writes via the Rust
`save_text_file(path, contents)` command — the file write stays in the core, the
UI only supplies the chosen path.
