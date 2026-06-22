# Runebook — Memory Graph Index

The curated map of this project's knowledge graph. One line per node, grouped into
communities. Read this first each session. Node bodies live in `nodes/`, never here.

See [CLAUDE.md](../CLAUDE.md) → "Memory" for the node/edge schema and conventions.

## Communities

### Architecture
- [[tauri-over-electron]] — why Tauri (not Electron/native): light, native overlay, web-skill reuse
- [[overlay-window]] — single frameless/transparent/always-on-top window, two modes
- [[global-hotkey]] — Ctrl+Alt+Space toggles the overlay; defined in `lib.rs`
- [[overlay-show-event]] — Rust emits `overlay:show` on summon; UI lands on Quick-add, focused
- [[hotkey-toggle-workspace]] — summon toggle checks focus (not just visibility) + sticks to all workspaces; the grab itself was always fine
- [[system-tray]] — Open/Quit; window hides to tray instead of quitting
- [[ipc-boundary]] — UI never touches DB/OS; calls Rust commands over IPC
- [[mcp-server]] — Phase 7/D13: standalone stdio MCP server; reuses `db.rs` via `#[path]`; connects runbooks to Claude Code/Cursor

### Data & Storage
- [[sqlite-over-json]] — searchable local store (FTS5) beats flat JSON
- [[markdown-step-model]] — a step is optional title + free-form markdown body (D8); migration v2 dropped the fixed fields
- [[rusqlite-data-layer]] — Phase 2: rusqlite (bundled) behind IPC commands, not `tauri-plugin-sql` (D7)
- [[current-runbook-setting]] — `setting` kv table (migration v3) persists the Quick-add target runbook
- [[backup-restore]] — Settings → Data: back up / restore the whole SQLite file (online backup API)
- [[fts5-search]] — Phase 3: FTS5 + bm25 ranked search over step title/body (migration v4), LIKE fallback
- [[bm25-materialized-gotcha]] — bm25() fails in aggregates/joins; score in an `AS MATERIALIZED` CTE
- [[wal-concurrency]] — WAL + busy_timeout in `db::open` so the app + MCP server share the file without `SQLITE_BUSY`
- [[git-sync]] — Phase 8/D16: export all runbooks → `<dir>/runbooks/*.md` + git commit (opt push); export-only; deletes only our own `<digits>-*.md`

### UX & Capture
- [[copy-per-code-block]] — replay loop preserved: copy button on every fenced code block, via Rust `copy_to_clipboard`
- [[markdown-editor-component]] — `MarkdownEditor.svelte`: true WYSIWYG (TipTap) storing markdown; single notepad + toolbar; killed the "Untitled" label
- [[quick-add-capture]] — Phase 4: hotkey lands on a keyboard-only composer; ⌘↵ = save & next
- [[tag-filtering]] — assign tags per runbook (detail editor); filter the list client-side by tag chip
- [[markdown-export]] — Phase 5: `export_markdown` → Copy .md / Save .md (dialog + Rust file write)
- [[settings-and-hotkey]] — Phase 5: Settings mode — live custom hotkey, accent theme, launch-on-login
- [[command-variables]] — `{{name}}` placeholders filled at copy time (in-memory; export keeps the template)
- [[optional-steps]] — D9: note shape is emergent from step count (0 = inline composer, 1 = plain note, 2+ = numbered); editable runbook title; no migration
- [[select-option-color]] — native `<select>` popup can't be themed in WebKitGTK → replaced with a custom DOM dropdown (don't retry option CSS)
- [[executable-steps]] — Phase 6/D11: opt-in ▶ run per code block (`run_command`), output inline; gated by `allow_run`, re-checked in core
- [[replay-progress]] — Phase 6/D10: Replay toggle = persistent per-step checklist (`step_progress`, v5); resumes where you left off; card badge
- [[variable-profiles]] — Phase 6/D12: named prod/staging value sets per runbook (`var_profile`, v6); secret vars masked & never persisted
- [[command-palette]] — Phase 8/D14: ⌘K modal, client-side title/tag filter, ↑↓/↵ to jump; not FTS (that's the Browse search)
- [[project-pinning]] — Phase 8/D15: pin a runbook to a dir (`project_dir`, v7); used as the Run cwd; no auto-surface-by-cwd
- [[markdown-render-key-guard]] — the `markdown` action re-rendered (and wiped run-output) on every var keystroke; now skips render unless the visible output changes
- [[ui-theming-tokens]] — 2026-06: bundled Hanken/Space Grotesk via @fontsource; accent tints are JS-set `--accent-soft`/`--accent-line` so they follow the runtime accent; `--ok`/`--err` are semantic, not the accent
- [[ai-reports]] — Phase 9/D17: an MCP AI writes a `kind='report'` runbook (migration v8); read-only reading view renders it richly (DOMPurify-sanitized markdown + callouts + TOC, copy-only code blocks) instead of a standalone HTML file
- [[browse-list-refresh]] — externally-written runbooks (MCP) didn't show in Browse until reload; `setMode("browse")` now re-queries (fixed 2026-06-22)

### Environment & Build
- [[x11-target]] — X11 confirmed; transparency + global shortcuts work, no Wayland hacks
- [[webkit-41-dev-missing]] — RESOLVED: 4.1 dev headers now installed; `cargo check` compiles
- [[production-readiness]] — 2026-06-18: verified `.deb` bundle + real icons + run_command timeout; the one gate left is that the live app has never been run (needs X11)
- [[release-ci-and-self-update]] — GitHub Actions (ci.yml + release.yml on `v*` tag) + `scripts/update.sh`/`release.sh`; CLI self-update because Tauri's updater is AppImage-only; tauri-action has no `includeFiles`; dpkg orders `-rc.1` above GA
- [[svelte-dynamic-input-type]] — Svelte 4 rejects dynamic `type` + `bind:value`; set `.type` via a `use:` action
- [[db-rs-shared-with-mcp]] — `db.rs` is `#[path]`-included by the MCP binary; changing a shared struct can break the MCP build (check both crates)
- [[pipe-masks-exit-code]] — `cargo … | tail` reports tail's exit code, not cargo's; verify from output text, not exit code
- [[app-svelte-nul-bytes]] — `src/App.svelte` holds 2 literal NUL bytes (in `keyOf`), so `file` calls it `data` and plain `grep` skips it — use `grep -a` or Read
- [[local-rustfmt-mismatch]] — this box's rustfmt differs from the project's; repo HEAD already "fails" `cargo fmt --check`, so don't run `fmt --write` — match file style, verify via build/test/clippy

### Process
- [[manual-entry-first]] — v1 captures steps manually; auto-capture deferred
- [[local-first-no-account]] — all data local, sharing via Markdown export

## Notable cross-community edges

- [[overlay-window]] —depends_on→ [[global-hotkey]] (EXTRACTED)
- [[x11-target]] —enables→ [[overlay-window]] transparency (EXTRACTED)
- [[webkit-41-dev-missing]] —blocks→ build/run until installed (EXTRACTED)
- [[manual-entry-first]] —conceptually_related_to→ [[sqlite-over-json]] (INFERRED, 0.6)
- [[rusqlite-data-layer]] —implements→ [[ipc-boundary]] (EXTRACTED) — the DB choice is driven by the IPC rule, not vice versa
- [[markdown-step-model]] —depends_on→ [[copy-per-code-block]] (EXTRACTED) — free-form markdown only works as a runbook because code blocks stay one-click-copyable
- [[markdown-editor-component]] —implements→ [[markdown-step-model]] (EXTRACTED) — the single notepad + live preview is how the body-only step is captured/edited; on edit it omits `title` so COALESCE preserves legacy titles
- [[copy-per-code-block]] —implements→ [[ipc-boundary]] (EXTRACTED) — clipboard goes through a Rust command, not `navigator.clipboard`
- [[quick-add-capture]] —depends_on→ [[overlay-show-event]] (EXTRACTED) — summon must signal the UI to land focused on capture
- [[current-runbook-setting]] —implements→ [[ipc-boundary]] (EXTRACTED) — pref persisted in SQLite, not a frontend store plugin
- [[markdown-export]] —implements→ [[ipc-boundary]] (EXTRACTED) — save dialog picks the path, but the file write is a Rust command
- [[settings-and-hotkey]] —part_of→ [[current-runbook-setting]] (EXTRACTED) — all prefs (hotkey, accent) reuse the same `setting` kv table
- [[optional-steps]] —depends_on→ [[markdown-step-model]] (EXTRACTED) — "optional steps" only works because a step is already a self-contained markdown note, so one note needs no step scaffolding
- [[replay-progress]] —depends_on→ [[optional-steps]] (EXTRACTED) — replay only runs for 2+ step runbooks, and reuses the self-contained-step model
- [[executable-steps]] —depends_on→ [[copy-per-code-block]] (EXTRACTED) — the ▶ run button is the copy affordance's sibling; both hang off the per-code-block render
- [[executable-steps]] —implements→ [[ipc-boundary]] (EXTRACTED) — the `allow_run` gate is re-checked in the Rust command, not just the UI (hiding a button isn't a boundary)
- [[variable-profiles]] —depends_on→ [[command-variables]] (EXTRACTED) — profiles are saved value sets for the existing `{{var}}` placeholders; secrets stay in-memory like the originals
- [[mcp-server]] —depends_on→ [[rusqlite-data-layer]] (EXTRACTED) — reuses `db.rs` verbatim via `#[path]`; one data layer, not two
- [[mcp-server]] —references→ [[ipc-boundary]] (EXTRACTED) — a separate trusted process, not the WebView, so "UI never touches the DB" still holds
- [[wal-concurrency]] —caused_by→ [[mcp-server]] (EXTRACTED) — two processes now share the file, so `db::open` switches to WAL
- [[project-pinning]] —depends_on→ [[executable-steps]] (EXTRACTED) — the pinned dir's only behavior is to be the cwd for run_command
- [[git-sync]] —depends_on→ [[markdown-export]] (EXTRACTED) — sync is `export_markdown` per runbook written into a git repo
- [[command-palette]] —conceptually_related_to→ [[fts5-search]] (EXTRACTED) — deliberately NOT FTS: instant client-side title/tag filter for jumping, FTS stays in Browse
- [[db-rs-shared-with-mcp]] —references→ [[mcp-server]] (EXTRACTED) — the `#[path]` include is why a `db.rs` struct change can break `runebook-mcp`
- [[project-pinning]] —caused_by→ [[db-rs-shared-with-mcp]] (EXTRACTED) — adding `RunbookPatch.project_dir` broke the MCP struct literal until fixed
- [[ai-reports]] —depends_on→ [[mcp-server]] (EXTRACTED) — reports are authored over MCP (`create_report`); the app only ever reads them
- [[ai-reports]] —depends_on→ [[markdown-step-model]] (EXTRACTED) — a report *is* a runbook whose single markdown step body is the whole document; no new entity
- [[ai-reports]] —part_of→ [[copy-per-code-block]] (EXTRACTED) — reuses the same `marked` + per-code-block-copy render path, now DOMPurify-sanitized for AI-authored content
- [[app-svelte-nul-bytes]] —caused_by→ [[ai-reports]] (EXTRACTED) — discovered while grepping App.svelte to wire the report renderer
