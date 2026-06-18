# CLAUDE.md — Runebook

Guidance for Claude Code when working in this repository.

## What this is

**Runebook** is a system-wide **runbook scratchpad**: a frameless, always-on-top
overlay summoned by a global hotkey from anywhere on the desktop. You capture the
steps of a task *as you do it* as free-form **markdown** notes (an optional title
+ a markdown body per step), then replay them later — one-click copy each command
(every fenced code block gets a copy button) — instead of re-reading docs.

Read [README.md](README.md) and the [docs/](docs/) folder before substantial work.
`docs/04-roadmap.md` is the source of truth for what to build next; `docs/05-decisions.md`
records why things are the way they are — update it when a decision changes.

## Stack

- **Tauri v2** — Rust core + WebView UI. Native global hotkey, tray, transparent
  always-on-top overlay.
- **Svelte 4 + Vite** — frontend (Vite dev server on port **1420**, never 3000).
- **SQLite** (Phase 2+, via `rusqlite` bundled, behind Rust IPC commands — see
  `docs/05-decisions.md` D7) — local data at `<app-data>/runebook.db`.
- **Target:** Linux / Pop!_OS on **X11**.

## Project layout

```
runebook/
├── README.md            overview + doc index
├── CLAUDE.md            this file
├── docs/                the plan (architecture, data model, UX, roadmap, decisions)
├── index.html           Vite entry
├── src/                 Svelte frontend (App.svelte, app.css, main.js)
├── src-tauri/           Rust core
│   ├── src/lib.rs       app setup: global hotkey, tray, window events
│   ├── src/main.rs      entry point → runebook_lib::run()
│   ├── tauri.conf.json  overlay window config (frameless/transparent/on-top)
│   ├── capabilities/    Tauri v2 permission grants
│   └── icons/           app + tray icons (placeholder, regenerate later)
├── mcp-server/          standalone MCP (stdio) server — connects runbooks to
│   └── src/main.rs      Claude Code/Cursor; reuses src-tauri/src/db.rs via #[path]
└── .memory/             Claude's project knowledge graph (see "Memory" below)
```

The **MCP server** (`mcp-server/`, docs/06-mcp-server.md) is a second binary that
opens the same `runebook.db` directly and reuses `db.rs` verbatim, so all SQL stays
single-sourced. It has no Tauri deps. If you change `db.rs`'s schema/types, both the
app and the MCP server pick it up — rebuild both (`cargo build` in each).

## Build & run

> **Prerequisite (one time):** Tauri v2 needs the **webkit2gtk 4.1** dev headers.
> These are now **installed on this machine** — `pkg-config --exists webkit2gtk-4.1`
> succeeds and `cargo check` compiles the Rust core. On a fresh box, install with:
> ```bash
> sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev \
>   libayatana-appindicator3-dev librsvg2-dev build-essential curl wget file
> ```

Use Node 22 via nvm, then:

```bash
source ~/.nvm/nvm.sh && nvm use 22
npm install            # first time
npm run tauri dev      # runs Vite (1420) + the Tauri overlay
```

- Frontend-only check (no webkit needed): `npm run build` (Vite → `dist/`).
- The window starts **hidden**. Press **Ctrl+Alt+Space** to toggle it, **Esc** to
  hide, and use the **tray** icon for Open/Quit. Closing the window hides to tray.

## Conventions

- **Hotkey** is defined once in `src-tauri/src/lib.rs` (`toggle`). Changing it means
  updating that `Shortcut` and the hint text in `src/App.svelte`.
- **Permissions:** any new frontend → Rust capability must be granted in
  `src-tauri/capabilities/default.json`, or the call fails at runtime.
- **The UI never touches the DB/OS directly** — it calls Rust commands over IPC.
  Keep SQL, filesystem, and clipboard logic in the Rust core.
- A **step** is an optional `title` + a free-form markdown `body` (migration v2;
  see `docs/05-decisions.md` D8). The rendered view attaches a copy button to each
  fenced code block — that's the replay loop. Markdown is parsed with `marked`.
- Match the existing typography system: Hanken for reading text, display faces for
  headlines, **orange strictly as an accent**.

## Verifying a change

`npm run build` validates the frontend (Vite → `dist/`) and `cargo check` (in
`src-tauri/`) validates the Rust core — both work on this machine now that the
webkit 4.1 dev headers are installed. A full interactive run (`npm run tauri dev`)
needs a graphical X11 session, so it may not be possible in a headless context;
state plainly in any summary which checks you actually ran versus could not.

---

## Memory — graphify architecture (md files)

Maintain a persistent project memory in [.memory/](.memory/) as a **knowledge graph
built from Markdown files**, following the [graphify](~/.claude/skills/graphify/SKILL.md)
model: **nodes → typed edges → clustered communities → an index**, with an honest
audit trail. This is the project's long-term memory — read it at the start of a
session, grow it as you learn.

### When to write a node (do this, don't wait to be asked)

At the **end of any session** — before wrapping up — scan what happened and flush
it to `.memory/`. Write or update a node whenever, this session, you:

- **made or changed a decision** (a choice with a rationale) → `type: decision`
- **hit a gotcha** (a surprise, an error, an environment quirk, a workaround) → `type: gotcha`
- **learned how a part works** that isn't obvious from the code → `type: concept`
- **left work unfinished** or set up a next step → `type: task`
- **learned a user preference** → `type: decision` or `reference`

If nothing in those categories happened, write nothing — empty churn is worse than
silence. After writing nodes, update `INDEX.md` to match. This is the project's
memory: it is only as good as your discipline in keeping it current.

### Structure

```
.memory/
├── INDEX.md          the graph index: communities, node list, edge summary
└── nodes/
    └── <slug>.md     one node per file — a single decision, concept, task, or gotcha
```

### A node file

Each file in `.memory/nodes/` is **one node = one idea**, with frontmatter and a body:

```markdown
---
id: tauri-overlay-window
label: Tauri Overlay Window
type: decision            # decision | concept | task | gotcha | reference
community: architecture   # cluster label (see INDEX.md)
edges:
  - target: global-hotkey
    relation: depends_on        # depends_on | implements | references | caused_by | part_of | conceptually_related_to
    confidence: EXTRACTED       # EXTRACTED | INFERRED | AMBIGUOUS
    confidence_score: 1.0       # 1.0 extracted · 0.6–0.9 inferred · 0.1–0.3 ambiguous
---

The overlay is one frameless transparent always-on-top window. It [[depends on
the global hotkey|global-hotkey]] to toggle visibility and lives in the [[system
tray|system-tray]] so it survives window close.
```

Rules, mirroring graphify's honesty discipline:

- **One fact per file.** Keep nodes small and atomic — like the user's auto-memory.
- **Link liberally** with `[[slug]]` (or `[[text|slug]]`). A link to a node that
  doesn't exist yet is fine — it marks a node worth writing later.
- **Every edge carries a confidence tag.** `EXTRACTED` = stated explicitly in code,
  docs, or by the user (score 1.0). `INFERRED` = a reasonable deduction (0.6–0.9).
  `AMBIGUOUS` = uncertain, flagged not omitted (0.1–0.3). **Never invent an edge** —
  if unsure, mark it `AMBIGUOUS`.
- **Don't duplicate the repo.** Record what is *not* derivable from code or git:
  decisions and their rationale, gotchas, the user's preferences, where work stands.
- **Update, don't fork.** If a node exists, edit it. If a decision is reversed,
  update the node and the relevant entry in `docs/05-decisions.md`.

### The index

Keep [.memory/INDEX.md](.memory/INDEX.md) current: group nodes into **communities**
(2–5 word cluster names like "Architecture", "Data & Storage", "UX & Capture"),
list each node under its community with a one-line hook, and note the most important
cross-community edges (graphify's "surprising connections"). The index is what you
load first each session — one line per node, never put node bodies in it.

### When the graph grows

Once `.memory/nodes/` has many files, you can run `/graphify .memory` to
auto-cluster, detect communities, find surprising connections, and generate an
interactive `graph.html` + audit report. The hand-maintained INDEX.md and the
graphify output are complementary: INDEX.md is the curated map, graphify is the
automated re-clustering and visualization.
