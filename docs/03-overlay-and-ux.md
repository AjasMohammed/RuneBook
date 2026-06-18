# 03 — Overlay window & UX

## Window configuration

A single Tauri window configured as a floating overlay:

```jsonc
{
  "decorations": false,     // frameless
  "transparent": true,      // rounded card floats over desktop
  "alwaysOnTop": true,
  "skipTaskbar": true,      // no taskbar entry; lives in tray
  "resizable": true,
  "visible": false,         // starts hidden; hotkey/tray reveals it
  "width": 720,
  "height": 520,
  "center": true
}
```

### X11 (confirmed environment)

On X11 transparency, always-on-top, and global shortcuts all work without
compositor workarounds. Notes:

- Ensure a compositor is running for true transparency (Pop!_OS GNOME provides one).
- Remember window position via `tauri-plugin-store` and restore on show.
- If transparency ever renders black, fall back to an opaque themed background —
  it does not block any functionality.

> If this is ever run on Wayland: global shortcuts are restricted and
> click-through / absolute positioning differ. Out of scope for v1; noted in
> [05-decisions.md](05-decisions.md).

## Global hotkey

- **Toggle overlay:** `Ctrl+Alt+Space` (configurable later).
  - Not the focused window (hidden, *or* visible on another workspace / behind
    something) → show, raise, focus, and land on **Quick-add** focused.
  - Already the focused foreground window → hide.
  - The toggle checks **focus, not just visibility** — a window mapped on another
    workspace still reports visible, so a visibility-only toggle would hide it
    where you can't see it and look broken. The overlay is also set
    visible-on-all-workspaces so summon lands on the current one.
- **Esc** hides the overlay (without quitting).
- Registered in Rust via `tauri-plugin-global-shortcut`; `toggle_overlay` in
  `lib.rs` is the single source of truth (hotkey + tray "Open").

## System tray

Tray menu: **Open (Browse)** · **Quick-add** · **Settings** · **Quit**.
Closing the window hides to tray rather than quitting (app keeps the hotkey alive).

## Two modes, one window

### Quick-add (capture)
The fast path — used *while* doing the task. A step is an optional title plus a
free-form **markdown** body, so you capture it however you think about it.

```
┌──────────────────────────────────────────┐
│  + Quick add        ▸ Runbook: [Deploy ▾] │
│  Title  [ SSH into prod (optional)      ] │
│  ┌────────────────────────────────────┐  │
│  │ need a shell on prod-1:            │  │
│  │ ```bash                            │  │
│  │ ssh deploy@host                    │  │
│  │ ```                                │  │
│  │ > key: ~/.ssh/deploy_ed25519       │  │
│  └────────────────────────────────────┘  │
│            (⌘↵ save & next · Esc hide)    │
└──────────────────────────────────────────┘
```

- Both fields optional — save anything with a title *or* a body.
- The body is markdown: headings, lists, quotes, links, and ```` ``` ```` fenced
  code blocks (which become one-click-copy in Browse).
- Saving appends to the **current runbook** (picker at top; "＋ New runbook" inline).
  The picker always offers "**— new runbook from this note —**", so you can start
  a fresh note at any time (not only on first use). It's a custom DOM dropdown —
  a native `<select>` popup can't be themed under WebKitGTK (its options render
  unreadable).
- `Ctrl/Cmd+Enter` = save and immediately start the next step (stay in flow).

### Browse / replay
The payoff — used *later* to repeat the task.

```
┌──────────────────────────────────────────┐
│  🔎 [ ssh deploy            ]   #deploy ✕ │
│  ── Deploy via SSH ──────────── ↑↓ ✎ ✕ ──│
│  1  SSH into prod                          │
│     need a shell on prod-1:                │
│     ┌────────────────────────────┐ [copy] │
│     │ ssh deploy@host            │        │
│     └────────────────────────────┘        │
│  2  Pull & build                           │
│     ┌────────────────────────────┐ [copy] │
│     │ git pull && npm run build  │        │
│     └────────────────────────────┘        │
│  ...                          [export .md] │
└──────────────────────────────────────────┘
```

- Step bodies render as markdown; **each fenced code block gets a [copy]**
  button → `copy_to_clipboard` (Rust), with a brief "copied" flash. This is the
  core replay loop: read → copy → paste → next.
- **The runbook title is editable** — click it (or the ✎ next to it) to rename
  in place; Enter or blur saves, Esc cancels (`update_runbook` title patch).
- **Steps are optional** (see [05-decisions.md](05-decisions.md) D9): a runbook
  with one note renders as a plain note (no number, no reorder); two or more
  render as the numbered list. An empty runbook shows an inline composer, and a
  **＋ Add step** button grows a note into multiple steps.
- Steps (when there are 2+) can be reordered (↑↓); any step can be edited (✎) in
  the markdown editor or deleted (✕).
- Live search across runbook title/description, tags, and step title/body
  (`LIKE` now; FTS5 ranking in Phase 3 — see [02](02-data-model.md)).
- Per-runbook **export to Markdown**.

## Advanced replay (Phase 6)

Three additions turn Browse from "searchable notes" into a working surface (see
[05-decisions.md](05-decisions.md) D10–D12):

- **▶ run** (D11) — next to each code block's `[copy]`, when execution is enabled
  in Settings → Execution (`allow_run`, **off by default**). Runs the command in
  your shell and shows captured stdout/stderr + exit code in a dismissible panel
  below the block. Off by default because it runs arbitrary code with your
  permissions; the gate is enforced in the Rust core, not just the UI.
- **▶ Replay** (D10) — a toggle on any 2+-step runbook. Each step gains a
  checkbox; a progress bar tracks `done/total`, **Reset** clears it. Progress
  **persists**, so you can close the overlay mid-task and resume; in-progress
  runbooks show a `done/total` badge in the sidebar list.
- **Variable profiles** (D12) — the `{{var}}` fill-in row gains named profiles
  (e.g. prod / staging): chips to apply, a "save as…" field to create/overwrite,
  ✕ to delete. A per-variable 🔒 masks the field and keeps that value out of any
  saved profile (secrets are retyped each session, never persisted).

## Reach & sharing (Phase 8)

- **Command palette** (D14) — `Ctrl/Cmd+K` from any mode opens a centered modal
  that filters runbooks by title/tag as you type; `↑↓` move, `↵` opens the pick in
  Browse, `Esc`/click-outside closes. Instant (client-side over the loaded list).
- **Pin to folder** (D15) — a runbook's detail header shows a 📁 pin control;
  pinning stores a directory and makes that runbook's **▶ run** commands execute
  there (so `git pull` runs in the right repo). ✕ unpins.
- **Git sync** (D16) — Settings → Git sync: pick a folder, then **Sync now**
  (export all runbooks to `runbooks/*.md` + `git commit`) or **Sync & push**. A
  status line reports the result; errors surface in the banner.

## Keyboard-first

| Key | Action |
|-----|--------|
| `Ctrl+Alt+Space` | Toggle overlay (global) |
| `Esc` | Hide overlay |
| `Ctrl/Cmd+Enter` | Save step & start next (Quick-add) |
| `Ctrl/Cmd+K` | Focus search (Browse) |
| `Tab` / `Shift+Tab` | Move between step fields |

## Styling

Reuse the existing typography system — reading text in the body face, runbook
titles in a display face, orange strictly as an accent (copy flash, active tag).
Overlay is a single rounded card with soft shadow over a translucent backdrop.
