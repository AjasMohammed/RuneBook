# 05 — Decision log & open questions

## Decisions

### D1 — Tauri over Electron / native
Lightweight (~5MB vs ~150MB), low RAM, native global-hotkey/tray/overlay support,
and the UI reuses existing web + typography skills. Native GTK/Qt rejected: too
much UI rebuilding, no design-system reuse.

### D2 — Manual entry first
Capture model is manual step entry for v1. Semi-auto (shell-history) and full
auto-record are deferred — they add shell-integration complexity and a privacy
surface that aren't needed to prove the core value.

### D3 — SQLite over JSON store
Search across runbooks is the core payoff, so a queryable store (with FTS5) beats
flat JSON. Single-file DB stays easy to back up and move.

### D4 — Local-first, no account
All data in `~/.local/share/runebook/runebook.db`. No cloud, no login. Sharing
happens via Markdown export and (later) raw DB file export.

### D5 — Single overlay window, two modes
Quick-add and Browse are UI states in one window, not separate OS windows —
simpler window/hotkey management.

### D7 — rusqlite in the Rust core, not `tauri-plugin-sql`
Phase 2 backs the database with **`rusqlite`** (bundled SQLite) behind Rust
`#[tauri::command]`s, rather than `tauri-plugin-sql`. That plugin is
frontend-facing — it runs SQL straight from the WebView — which contradicts the
project's hard rule that *the UI never touches the DB and calls Rust commands
over IPC* (CLAUDE.md, [01-architecture.md](01-architecture.md)). rusqlite keeps
all SQL in one auditable place and matches the documented IPC command surface
exactly. `bundled` compiles SQLite from source, so no system libsqlite is
needed. Earlier docs naming `tauri-plugin-sql` are superseded by this entry.

### D8 — Free-form markdown steps, not fixed fields
A step is an **optional title + a free-form markdown `body`**, not the original
fixed fields (command/why/where_ctx/example/note). Rationale: users want a
customizable scratchpad to capture and revise a workflow however they think
about it, not a rigid form. Markdown keeps it expressive (headings, lists,
quotes, links). The core replay loop is preserved by rendering markdown and
attaching a **copy button to every fenced code block** — so commands stay
one-click-copyable while everything around them is free text. This supersedes
the fixed-field step model in docs 02/03; migration v2 collapses any existing
fixed fields into a markdown body and drops the old columns. `where` being a
SQL reserved word no longer matters — that column is gone.

### D6 — Target X11 for v1
Environment confirmed X11 (`XDG_SESSION_TYPE=x11`); transparency, always-on-top,
and global shortcuts work without compositor hacks. Wayland support is explicitly
out of scope for v1.

### D9 — Steps are optional; emergent single-note vs. multi-step
A runbook no longer always presents as a numbered step list. The shape is
**emergent from the step count**, not a stored type or schema flag:

- **0 steps** → Browse shows an inline note composer, so a runbook created from
  the "New runbook title…" form can be filled in immediately (no detour to
  Quick-add).
- **1 step** → renders as a **plain note**: no "Step N" number, no reorder
  arrows, just the rendered body with Edit/Delete on hover. This is the "simple
  note" case.
- **2+ steps** → the numbered list with full reorder/edit/delete tools.

An always-present **＋ Add step** button grows a note into a multi-step runbook,
so "notes with steps" stays one click away. Chosen over a stored `is_simple` /
note-type column because it needs **no migration**, keeps one data model
(runbook → steps), and a note converts between the two shapes just by gaining or
losing steps. Also in this change: the open runbook's **title is editable** in
Browse (inline rename via the existing `update_runbook` title patch) — previously
a title could only be set at creation. Supersedes the "No steps yet — capture
some in Quick-add" empty state described in [03-overlay-and-ux.md](03-overlay-and-ux.md).

### D10 — Replay progress is a persistent per-step checklist, not a run history
A multi-step runbook can be worked through as a checklist: each step gets a
"done" flag in a `step_progress` table (migration v5), keyed by step id and
cascading on delete. We deliberately store **one current checklist state per
runbook**, *not* a log of historical runs. Rationale: the value is "resume where
I left off" — close the overlay mid-task, come back, and the boxes are still
ticked — which a single mutable state delivers with far less schema and UI than
timestamped run records. A **Reset** clears it to start over. Progress is exposed
two ways: a `done` flag on each step (a LEFT JOIN in `get_runbook`) for the
checkboxes, and `list_progress()` (grouped counts, only where `done > 0`) for the
`done/total` badge on cards. Replay is a UI mode toggled per open runbook and only
offered for 2+ step runbooks (a one-note runbook has nothing to track).

### D11 — Command execution is opt-in and gated in the core
The **▶ run** button executes a code block in the user's login shell
(`run_command` → `$SHELL -c`, capturing stdout/stderr/exit code). This is the
only command that runs arbitrary code, so it is **off by default** behind an
`allow_run` setting. Crucially the gate is **re-checked inside the Rust command**,
not just in the UI — hiding a button is not a security boundary, refusing the IPC
call is. The trust model is deliberately simple: a Runebook is the user's own
notes, commands run with the user's own permissions, so the meaningful boundary is
the explicit enable, not per-command sandboxing. Output is captured with a
blocking `.output()` (fine for the short ops a runbook holds; Tauri runs sync
commands off the main thread) and rendered via `textContent` so command output
can never inject markup into the overlay. Streaming/long-running processes are out
of scope for Tier 1. This narrows open question **Q4** for the execution surface;
encrypted-DB storage of secrets is still open (see D12 for the in-memory partial).

### D12 — Variable profiles are per-runbook value sets; secrets never persist
The `{{var}}` placeholders (the earlier in-memory fill-ins) gain **named profiles**
per runbook — e.g. "prod" and "staging" — stored in a `var_profile` table
(migration v6) as a JSON `name→value` map per profile. Chosen as one JSON column
rather than a fully normalized value table: a profile is a small bag of strings,
so the blob keeps the CRUD and the IPC (`HashMap<String,String>`) trivial without
losing anything queryable we actually need. Saving the same name overwrites (so
"save" doubles as "update"). A per-variable **secret** mark (UI-side) masks the
input and **excludes that variable when a profile is saved**, so a key/password
never reaches the database; only the set of secret *names* persists (in the
`setting` kv under `secret_vars:<runbookId>`) so the mask is remembered, and the
value is retyped each session. This is the pragmatic, no-new-dependency partial
answer to Q4 for secrets — a full encrypted DB / OS-keyring integration remains
deferred.

### D13 — MCP server is a standalone process sharing the SQLite file
The Model Context Protocol integration ([06-mcp-server.md](06-mcp-server.md)) is a
**separate `runebook-mcp` binary** ([`mcp-server/`](../mcp-server/)) that MCP
clients (Claude Code, Cursor, …) spawn over **stdio**, not an endpoint embedded in
the Tauri app. Rationale:

- Those clients already spawn stdio MCP servers on demand — the natural fit. It
  needs **no running app and no network port**, and it builds/runs headless.
- It opens the same `<app-data>/runebook.db` directly and **reuses the app's
  `db.rs` verbatim** (`#[path]` include), so schema/migrations/FTS5 stay
  single-sourced. The MCP binary pulls in only `rusqlite` + `serde` — **no
  Tauri/webkit** — keeping with the lightweight ethos (D1).
- This does *not* violate the "UI never touches the DB" IPC rule (D7): that rule
  governs the **WebView UI**, which still goes through Rust IPC commands. The MCP
  server is a separate trusted local process acting on the user's behalf, the same
  way the app's own Rust core does. It deliberately does **not** expose `run_command`
  (D11) — execution stays an explicit, app-gated action, not a remotely-callable tool.

Because the app and the MCP server now open the file at the same time, `db::open`
switches the database to **WAL** journal mode with a 5s `busy_timeout` so readers
and a single writer don't block (and a momentary lock waits instead of erroring).
WAL is backup-API-compatible (D4 backup/restore still work). The protocol layer is
a small hand-rolled synchronous JSON-RPC stdio loop rather than the async `rmcp`
SDK — fewer dependencies, and a good match for synchronous rusqlite.

The server exposes read + write CRUD (incl. delete); MCP clients gate each call
behind user approval. Secrets in steps (Q4) remain plaintext and readable by any
agent the server is connected to — noted in the doc's security section.

### D14 — Command palette is a client-side title/tag filter, not server search
The Ctrl/Cmd+K quick switcher (Phase 8) filters the **already-loaded** runbook
list by title/tag substring in the frontend — no IPC per keystroke — and opens the
chosen runbook in Browse. Deliberately *not* wired to FTS5: the palette is for
fast *jumping* between runbooks you know exist, where instant client-side
filtering beats a round-trip; deep step-body search already lives in the Browse
search box (which does use FTS). Keeping it client-only also means zero new Rust
surface for the feature. Capped at 50 results to keep the modal short.

### D15 — Project pinning: a directory column, used as the run cwd
A runbook can be pinned to a directory via a `project_dir` column on `runbook`
(migration v7; "" = unpinned). Its one concrete behavior is to become the
**working directory for executed commands** — `run_command(text, cwd)` receives
`selected.projectDir`, so a pinned runbook's `git pull` / `npm run build` run in
the right repo. We do **not** auto-surface a runbook by the "current directory":
a global desktop overlay has no cwd context to match against, so that would be
fiction. `project_dir` is populated only by `get_runbook` (the detail view), left
"" in list/FTS results so those queries stay untouched. Composes with D11
(execution) — pinning is what makes Run useful for project-scoped commands.

### D16 — Git sync exports Markdown into a repo; it is not a live two-way sync
"Sync to Git" (Phase 8) writes every runbook to `<dir>/runbooks/<id>-<slug>.md`
via the existing `export_markdown`, then runs `git init`/`add`/`commit` (and an
optional `push`) in that folder. Decisions:
- **One-way, export-only.** It snapshots the DB into a repo for versioning/sharing;
  it never reads `.md` back in. This keeps the SQLite DB the single source of
  truth (D3) and avoids a Markdown↔DB merge problem. Round-tripping is a possible
  future, explicitly out of scope now.
- **Propagate deletions without data loss.** A deleted runbook's `.md` must drop
  out of the repo, but the user may have picked a folder that already holds
  unrelated files. So instead of `remove_dir_all(runbooks/)` (which would destroy
  them), we delete only files matching our own export naming `<digits>-*.md`, then
  rewrite the current set. `is_export_filename`/`slugify` are unit-tested, the
  latter also guaranteeing a slug can't contain path separators (no escape).
- **`git init` if needed; push is opt-in.** Committing is local and safe by
  default; pushing (network, auth) only happens on the explicit "Sync & push".
- Shells out to `git` (a fixed binary, not arbitrary shell like D11), gated only
  by the user picking a folder + clicking sync. Note: a malicious repo's git hooks
  could run on commit — acceptable since the user chose the folder.

### D17 — AI reports are richly-rendered Markdown documents stored as runbooks
An AI connected over the MCP server (D13) can author a **report** — a project
introduction, a "what changed" summary, an analysis — that the user reads *inside*
Runebook, instead of generating a throwaway standalone HTML file. Decisions:

- **A report is a `kind='report'` runbook, not a new entity.** Migration v8 adds a
  `kind TEXT NOT NULL DEFAULT 'runbook'` column to `runbook`; a report is a runbook
  with `kind='report'` whose single step body holds the whole Markdown document.
  This reuses *all* of runbook storage, FTS search, and Markdown export rather than
  forking a parallel `report` table — matching the "shape is emergent" philosophy
  of D9. It is created with `db::create_report` (one INSERT + one `add_step`).
- **One new MCP tool: `create_report(title, body, tags?, description?)`.** Its
  verbose description teaches the agent the house format so "make a dev intro / a
  summary of my changes" reliably produces a self-contained Markdown doc using the
  blocks Runebook renders richly. It is a mutating tool (hidden under
  `RUNEBOOK_MCP_READONLY=1`, like the other writers).
- **Rendering is rich but safe.** The app renders the report body through the same
  `marked` + per-code-block-copy pipeline as steps, plus a report-only enrichment
  pass: GitHub-style callouts (`> [!NOTE]`/`[!TIP]`/`[!IMPORTANT]`/`[!WARNING]`/
  `[!CAUTION]`), a table of contents auto-built from headings, and `<details>`
  collapsibles — all styled with the existing design tokens so a report is
  automatically on-brand (unlike a raw AI HTML file, which clashes). **Crucially,
  because a report body is AI/agent-authored, the parsed HTML is sanitized with
  DOMPurify before it ever reaches `innerHTML`** — `<script>`, inline event
  handlers, and other injection vectors are stripped. Sanitization was added to the
  shared `markdown` action, so ordinary steps (also writable over MCP) are hardened
  too. This is the security boundary the feature turns on.
- **A report is a read-only reading view, copy-only.** Opening a `kind='report'`
  runbook shows a dedicated reading layout (badge, title, TOC, rendered body, Copy/
  Save `.md`) instead of the step/replay/variable chrome. Code blocks keep their
  one-click **copy** button but **not** the ▶ run button (D11): an AI-authored
  document is to be read and copied from, not executed wholesale. **Copy/Save `.md`
  exports the report's raw Markdown (title + body), not the per-step `## N. Step`
  scaffolding a procedure export gets** (`export_markdown` special-cases
  `kind='report'`). Links in any rendered markdown open in the **system browser**
  via the `open_external` command — an in-content link would otherwise navigate the
  overlay's WebView off the app, a dead end with no titlebar/back/Esc.
- **Surfacing.** The MCP server and the overlay are separate processes (D13) with no
  IPC channel, so a freshly written report can't be pushed to the overlay live; it
  appears the next time the Browse list loads (re-query on summon). A live DB-watch /
  tray notification is a deliberate future, out of scope for the first slice.
- **Mermaid diagrams** (the biggest extra visual win) render in reports: a
  ` ```mermaid ` fenced block becomes an SVG. Mermaid is heavy, so it is pulled in
  via **dynamic `import()`** only when a report actually contains a diagram — Vite
  splits it into its own chunk (`mermaid.core-*.js`), so the base bundle stays light
  (D1). It renders with `securityLevel: "strict"` (mermaid escapes label HTML in its
  own output); an invalid diagram or a missing module falls back to showing the
  diagram source rather than failing silently.
- **Still deferred:** an "AI HTML in a sandboxed iframe" escape hatch for fully
  free-form output — rejected as the default (large security surface, ignores the
  design tokens, breaks the IPC-routed copy buttons).

This narrows open question **Q4** for AI-authored content: reports render sanitized,
and code blocks in them are copy-only (never auto-run).

## Open questions

### Q1 — UI framework
React, Svelte, or vanilla TS for the WebView? Needed before Phase 0.
*Leaning:* Svelte (tiny, fast, good fit for a small reactive overlay) — but React
if broader familiarity matters more.

### Q2 — Default global hotkey
`Ctrl+Alt+Space` proposed. Confirm it doesn't collide with an existing Pop!_OS /
GNOME shortcut on this machine before hardcoding.

### Q3 — Project name
"Runebook" is a placeholder (rune + runbook). Keep or rename before scaffolding
(affects bundle id, e.g. `com.<name>.app`).

### Q4 — Secrets in steps
Steps may contain keys/hosts. v1 stores plaintext locally. Decide whether an
encrypted-DB option is needed before real secrets go in.
