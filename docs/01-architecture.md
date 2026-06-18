# 01 — Architecture

## Process model

Tauri runs two cooperating processes:

```
┌─────────────────────────────────────────────┐
│ Rust core (the "backend")                    │
│  - window lifecycle (overlay show/hide)      │
│  - global hotkey registration                │
│  - system tray                               │
│  - SQLite access (commands invoked via IPC)  │
│  - clipboard writes                          │
└───────────────▲──────────────────────────────┘
                │  Tauri IPC (invoke / events)
┌───────────────▼──────────────────────────────┐
│ WebView UI (the "frontend")                   │
│  - Quick-add form                             │
│  - Browse / search / replay views             │
│  - styled with existing typography system     │
└───────────────────────────────────────────────┘
```

The UI never touches the DB or OS directly — it calls Rust **commands** over IPC.
This keeps SQL, filesystem, and clipboard logic in one auditable place.

## Crates & plugins

| Concern | Crate / plugin |
|---------|----------------|
| App shell | `tauri` (v2) |
| Global hotkey | `tauri-plugin-global-shortcut` |
| Local DB | `tauri-plugin-sql` (sqlite feature) |
| Key-value / settings | `tauri-plugin-store` (window pos, last runbook, prefs) |
| Clipboard | `tauri-plugin-clipboard-manager` |
| Autostart on login (later) | `tauri-plugin-autostart` |

Migrations: define SQL migrations in the `tauri-plugin-sql` builder so the schema
is created/updated on launch. See [02-data-model.md](02-data-model.md).

## IPC surface (Rust commands)

Frontend calls these via `invoke()`:

```
list_runbooks(query?: string)            -> Runbook[]
get_runbook(id)                          -> Runbook + Step[]
create_runbook(title, tags?, desc?)      -> id
update_runbook(id, patch)                -> ok
delete_runbook(id)                       -> ok

add_step(runbook_id, step)               -> id
update_step(id, patch)                   -> ok
reorder_steps(runbook_id, ordered_ids[]) -> ok
delete_step(id)                          -> ok

quick_add(step, runbook_id?)             -> id   // append to current/new runbook
copy_to_clipboard(text)                  -> ok
export_markdown(runbook_id)              -> string

// Phase 6 — Advanced (docs/05-decisions.md D10–D12)
run_command(text, cwd?)                  -> { stdout, stderr, exitCode }  // gated by allow_run (D11)
list_progress()                          -> [{ runbookId, done, total }]  // in-progress badges (D10)
set_step_done(step_id, done)             -> ok
reset_progress(runbook_id)               -> ok
list_var_profiles(runbook_id)            -> string[]                      // variable profiles (D12)
get_var_profile(runbook_id, name)        -> { [name]: value } | null
save_var_profile(runbook_id, name, values) -> ok
delete_var_profile(runbook_id, name)     -> ok

// Phase 8 — Advanced Tier 2 (docs/05-decisions.md D15–D16)
//   project pinning reuses update_runbook's patch (projectDir); run_command takes cwd.
git_sync(dir, push)                      -> string  // export all → git commit (+ optional push)
```

(Plus `get_setting`/`set_setting`, `get_hotkey`/`set_hotkey`, `get_autostart`/
`set_autostart`, `backup_database`/`restore_database`, `save_text_file` — the
full registered surface is in `src-tauri/src/lib.rs`.)

Events emitted Rust → UI:

```
hotkey:toggle      // global shortcut fired; UI focuses quick-add
tray:open-browse   // user clicked tray "Browse"
```

## Window strategy

Single overlay window, two logical modes (Quick-add / Browse) switched in the UI —
not separate OS windows. Details and X11 specifics in
[03-overlay-and-ux.md](03-overlay-and-ux.md).

## Data location

`~/.local/share/com.runebook.app/runebook.db` (Tauri app-data dir, derived from
the bundle identifier). Single SQLite file = trivially backup-able and portable.
No cloud, no account — local-first by default.

## External access — the MCP server

A second, optional process — **`runebook-mcp`** ([`mcp-server/`](../mcp-server/),
[06-mcp-server.md](06-mcp-server.md)) — opens the *same* SQLite file directly to
serve it over the Model Context Protocol to AI tools (Claude Code, Cursor, …). It
reuses the app's `db.rs` verbatim, so there's one data layer, not two. Because two
processes now share the file, `db::open` runs it in **WAL** mode with a
`busy_timeout` (see [05-decisions.md](05-decisions.md) D13). This is the one path
to the data that doesn't go through the Tauri app — a separate trusted local
process, not the WebView — so it doesn't break the "UI never touches the DB" rule.
