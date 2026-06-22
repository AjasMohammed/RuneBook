# 04 — Roadmap

Phased so each milestone is independently runnable and demoable.

**Status (2026-06-16):** Phases 0–5 ✅ done — the full v1 plan is implemented —
plus **Phase 6 (Advanced, Tier 1)** ✅: executable steps, replay sessions, and
variable profiles; **Phase 7 (MCP server)** ✅: a standalone stdio server that
connects the runbooks to Claude Code / Cursor; and **Phase 8 (Advanced, Tier 2)** ✅:
command palette, project pinning, and git-backed sync (AI assist deferred by
request). Remaining work is iterative polish + the "Later" backlog below.
Note: every check here is `cargo test`/`cargo check` + `npm run build`; a live
`npm run tauri dev` needs a graphical X11 session (not run in this environment).

## Phase 0 — Scaffold ✅
**Goal:** a Tauri app that launches.
- `create-tauri-app` (pick UI framework: React / Svelte / vanilla — see open Q).
- App builds and runs an empty window on X11.
- **Done when:** `npm run tauri dev` opens a window.

## Phase 1 — Overlay shell ✅
**Goal:** the floating overlay you can summon from anywhere.
- Frameless + transparent + always-on-top + skip-taskbar window config.
- `tauri-plugin-global-shortcut`: `Ctrl+Alt+Space` toggles show/hide.
- System tray with Open / Quit; window hides to tray instead of quitting.
- Restore last window position via `tauri-plugin-store`.
- **Done when:** hotkey reliably toggles a translucent overlay over any app;
  Esc hides; tray quit works.

## Phase 2 — Data + CRUD ✅
**Goal:** persist runbooks and steps.
- SQLite via `rusqlite` (bundled) behind Rust IPC commands — *not* `tauri-plugin-sql`
  (see [05-decisions.md](05-decisions.md) D7). Migrations tracked by `PRAGMA user_version`.
- Rust commands: create/list/get/update/delete for runbook & step; reorder.
- Browse list proving data round-trips.
- **Done when:** create a runbook, add steps, restart app, data persists.

## Phase 3 — Replay UX ✅
**Goal:** find and reuse a procedure fast.
- ✅ Per-code-block **copy to clipboard** (Rust `copy_to_clipboard`) on rendered markdown.
- ✅ **FTS5 ranked search** over step title/body (migration v4, bm25), also matching
  runbook title/description/tags; graceful `LIKE` fallback if FTS5 is unavailable.
  Covered by unit tests in `src-tauri/src/db.rs`.
- ✅ **Tag filtering**: assign tags per runbook (detail editor), filter the list by
  clicking a tag chip (client-side over the loaded list).
- **Done when:** search "ssh" → open runbook → copy each command in one click.

## Phase 4 — Quick-add capture ✅
**Goal:** capture a step in seconds mid-task.
- ✅ Quick-add mode is the hotkey landing (Rust emits `overlay:show`; UI focuses
  the composer). Step = optional title + markdown body.
- ✅ "Current runbook" persisted via a `setting` kv table (`current_runbook`);
  inline new-runbook in the picker.
- ✅ `Ctrl/Cmd+Enter` = save & next (clears + refocuses, with a "Saved" flash).
- **Done when:** summon overlay → type a step → save & next without touching mouse.

## Phase 5 — Polish & export ✅
**Goal:** make it pleasant and portable.
- ✅ Export runbook → Markdown: `export_markdown` (Rust, unit-tested) with **Copy .md**
  (clipboard) and **Save .md** (native save dialog → `save_text_file`, write in core).
- ✅ Autostart on login (`tauri-plugin-autostart`, toggled from Settings via Rust commands).
- ✅ Settings panel (third mode): **custom hotkey** (re-registered live + persisted,
  applied at startup), **accent theme** presets (persisted, applied to `--accent`).
- ◑ Styling: accent theming, tabbed modes, overlay card. Typography still uses
  Hanken/system fallback throughout — a dedicated display face for titles + bundling
  the font files is the remaining cosmetic polish.
- **Done when:** looks finished, starts on login, a runbook exports to `RUNBOOK.md`. ✅

## Phase 6 — Advanced (Tier 1) ✅
**Goal:** go beyond searchable notes — execute, track, and parameterize a runbook.
- ✅ **Executable steps** ([05-decisions.md](05-decisions.md) D11): an opt-in **▶ run**
  button on every fenced code block runs the command in the user's shell
  (`run_command`, Rust) and renders captured stdout/stderr + exit code inline.
  Gated by an `allow_run` setting (default off), re-checked in the core so the UI
  can't bypass it. Output is set via `textContent` (no markup injection).
- ✅ **Replay sessions** (D10): a **Replay** toggle turns a 2+-step runbook into a
  checklist — per-step checkboxes, a progress bar, and **Reset**. Done flags
  persist (`step_progress` table, migration v5) so closing/reopening resumes where
  you left off; in-progress runbooks show a `done/total` badge on their card.
- ✅ **Variable profiles** (D12): named saved value sets per runbook (`var_profile`
  table, migration v6) for the `{{var}}` placeholders — click a profile chip to
  apply, "save as…" to create/overwrite, ✕ to delete. A per-variable **secret**
  toggle masks the field and excludes it from saved profiles (only secret *names*
  persist, never values), so keys/passwords are retyped each session.
- **Done when:** enable Run → run a command and see its output; replay a runbook as
  a checklist and resume it; switch between a "prod" and "staging" profile. ✅
- Verified via `cargo test` (incl. `step_progress_tracks_and_resets`,
  `var_profiles_crud_roundtrip`) + `npm run build`.

## Phase 7 — MCP server (integrations) ✅
**Goal:** reach the runbooks from AI tools, not just the overlay.
- ✅ **`runebook-mcp`** ([`mcp-server/`](../mcp-server/), [06-mcp-server.md](06-mcp-server.md),
  [05-decisions.md](05-decisions.md) D13): a standalone stdio Model Context Protocol
  server that MCP clients (Claude Code, Cursor, …) spawn on demand. Opens the same
  `<app-data>/runebook.db` directly — **no running app, no network port** — and
  reuses `db.rs` verbatim (`#[path]` include) so the SQL stays single-sourced; the
  binary has no Tauri/webkit deps.
- ✅ Tools: `list_runbooks` (FTS search), `get_runbook`, `export_runbook_markdown`,
  and create/update/delete for runbooks + steps. Deliberately **omits** `run_command`
  (execution stays app-gated, D11).
- ✅ Concurrency: `db::open` enables **WAL** + `busy_timeout` so the app and the MCP
  server share the file without `SQLITE_BUSY`.
- **Done when:** `claude mcp add runebook -- …/runebook-mcp`, then an agent can
  search and capture runbooks from the editor. ✅
- Verified via `cargo build --release` + an end-to-end stdio JSON-RPC smoke test
  (initialize → tools/list → create/add/search/get/export/update/delete).

## Phase 8 — Advanced (Tier 2, AI excluded) ✅
**Goal:** reach, parameterize, and share — minus the AI assistant (deferred by request).
- ✅ **Command palette / quick switcher** ([05-decisions.md](05-decisions.md) D14):
  `Ctrl/Cmd+K` opens a modal that filters the loaded runbooks by title/tag (instant,
  client-side) with ↑↓ / ↵ to jump; opens the pick in Browse.
- ✅ **Project pinning** (D15): pin a runbook to a directory (`project_dir`, migration
  v7); a pinned runbook's **Run** commands execute with that dir as cwd. (No
  auto-surface-by-cwd — a global overlay has no cwd to match.)
- ✅ **Git-backed sync** (D16): Settings → Git sync exports every runbook to
  `<dir>/runbooks/*.md` and `git add`/commit (optional push). One-way/export-only;
  the `runbooks/` subdir is rebuilt each sync so deletions propagate.
- ⨯ **AI assist** — explicitly deferred for now (not implemented).
- **Done when:** ⌘K jumps to any runbook; a pinned runbook runs commands in its repo;
  Settings → Sync writes a committed `runbooks/` tree. ✅
- Verified via `cargo test` (incl. `run_gate_defaults_off_and_tracks_setting`,
  `project_dir_pins_and_clears`) + `npm run build` + MCP `cargo check`.

## Phase 9 — AI reports (in progress)
**Goal:** let an MCP-connected AI write a **report** the user reads *inside* Runebook
— a project intro, a "what changed" summary, an analysis — instead of dumping a
standalone HTML file ([05-decisions.md](05-decisions.md) D17).
- ✅ **Data:** migration **v8** adds a `kind` column to `runbook`; a report is a
  `kind='report'` runbook whose single step body is the Markdown document.
  `db::create_report` creates one. `list`/`get` return `kind`.
- ✅ **MCP tool:** `create_report(title, body, tags?, description?)` (mutating;
  hidden in read-only mode), with a verbose description that teaches the agent the
  house format (callouts, tables, `<details>`, headings → TOC, fenced code blocks).
- ✅ **Safe rich rendering:** the report body renders through the shared `marked`
  pipeline — now **DOMPurify-sanitized** because the content is AI-authored — plus a
  report-only pass for GitHub-style callouts and an auto table of contents. A
  `kind='report'` runbook opens in a read-only reading view (badge, TOC, Copy/Save
  `.md`); code blocks are **copy-only** (no ▶ run).
- ⨯ **Deferred:** Mermaid diagrams (dynamic-import later, keeps the bundle light);
  live push of a new report to an open overlay (separate processes — appears on next
  list load for now); a sandboxed-iframe "raw AI HTML" escape hatch.
- **Done when:** an agent calls `create_report`, and opening it in Browse shows a
  clean, sanitized, on-brand document with a working TOC and copy buttons.
- Verified via `cargo test` (db `create_report`/`kind`, MCP tool list) +
  `npm run build` + MCP `cargo check`.

## Later / nice-to-have
- ✅ Variables/placeholders in commands (`ssh deploy@{{host}}`) filled at copy time —
  per-runbook fill-in fields in Browse; live substitution in the rendered preview
  and in copied commands; values kept in memory only (never persisted). Export
  keeps the raw `{{…}}` template.
- ✅ Backup/restore the single SQLite file — Settings → Data: **Back up database…**
  (online backup API, safe on a live connection) and **Restore…** (validates the
  file is a Runebook DB, copies it over the live connection in place, UI reloads).
  Unit-tested. Cloud sync still deferred.
- ✅ Pin a runbook to a project directory (Phase 8 / D15) — also used as the run cwd.
- Semi-auto capture from shell history (the deferred idea from planning).
- AI assist (generate/clean/explain/NL-search via Claude API) — **deferred by request.**
- Encrypted DB option for secrets in steps.
- Two-way git sync (read `.md` back into the DB) — D16 is export-only for now.
- Bundle a display font + apply it to titles (the remaining typography polish).

## Suggested order to start
Phase 0 → 1 first (proves the hardest/native part: overlay + hotkey on X11),
then 2 → 4 → 3 → 5. Capturing (4) before perfecting replay (3) lets you start
using it to dogfood immediately.
