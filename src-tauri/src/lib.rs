mod db;

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Mutex;

use rusqlite::Connection;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, State, WindowEvent,
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Shared SQLite handle. rusqlite's `Connection` isn't `Sync`, so it lives
/// behind a `Mutex` in Tauri-managed state; every command locks it briefly.
type Db = Mutex<Connection>;

/// Toggle hotkey used until the user customizes it (docs/03-overlay-and-ux.md).
const DEFAULT_HOTKEY: &str = "Control+Alt+Space";

/// Parse `spec` (e.g. "Control+Alt+Space") and make it the sole registered global
/// shortcut, replacing any previous one. Errors if the spec is unparseable.
fn register_hotkey(app: &tauri::AppHandle, spec: &str) -> Result<(), String> {
    let shortcut = Shortcut::from_str(spec).map_err(|_| format!("invalid hotkey: {spec}"))?;
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    gs.register(shortcut).map_err(|e| e.to_string())
}

// ----------------------------------------------------------------------------
// IPC commands (the surface in docs/01-architecture.md). Each locks the shared
// connection and delegates to `db`, mapping errors to strings for the UI.
// ----------------------------------------------------------------------------

#[tauri::command]
fn list_runbooks(db: State<'_, Db>, query: Option<String>) -> Result<Vec<db::Runbook>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::list_runbooks(&conn, query.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_runbook(db: State<'_, Db>, id: i64) -> Result<Option<db::Runbook>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::get_runbook(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_runbook(
    db: State<'_, Db>,
    title: String,
    tags: Option<Vec<String>>,
    description: Option<String>,
) -> Result<i64, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::create_runbook(&conn, &title, tags.as_deref(), description.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn update_runbook(db: State<'_, Db>, id: i64, patch: db::RunbookPatch) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::update_runbook(&conn, id, patch).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_runbook(db: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::delete_runbook(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_step(db: State<'_, Db>, runbook_id: i64, step: db::StepInput) -> Result<i64, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::add_step(&conn, runbook_id, step).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_step(db: State<'_, Db>, id: i64, patch: db::StepPatch) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::update_step(&conn, id, patch).map_err(|e| e.to_string())
}

#[tauri::command]
fn reorder_steps(db: State<'_, Db>, runbook_id: i64, ordered_ids: Vec<i64>) -> Result<(), String> {
    let mut conn = db.lock().map_err(|e| e.to_string())?;
    db::reorder_steps(&mut conn, runbook_id, &ordered_ids).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_step(db: State<'_, Db>, id: i64) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::delete_step(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
fn quick_add(
    db: State<'_, Db>,
    step: db::StepInput,
    runbook_id: Option<i64>,
) -> Result<i64, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::quick_add(&conn, step, runbook_id).map_err(|e| e.to_string())
}

/// Read a persisted setting (e.g. the current runbook id). Returns `None` if
/// unset.
#[tauri::command]
fn get_setting(db: State<'_, Db>, key: String) -> Result<Option<String>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::get_setting(&conn, &key).map_err(|e| e.to_string())
}

/// Write a persisted setting.
#[tauri::command]
fn set_setting(db: State<'_, Db>, key: String, value: String) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::set_setting(&conn, &key, &value).map_err(|e| e.to_string())
}

/// Write text to the system clipboard. Clipboard access stays in the Rust core
/// (the UI calls this over IPC) per the project's IPC boundary. Backs the
/// per-code-block copy buttons in the rendered markdown.
#[tauri::command]
fn copy_to_clipboard(app: tauri::AppHandle, text: String) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

/// Render a runbook to portable Markdown (Phase 5 export).
#[tauri::command]
fn export_markdown(db: State<'_, Db>, runbook_id: i64) -> Result<Option<String>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::export_markdown(&conn, runbook_id).map_err(|e| e.to_string())
}

/// Write text to a path chosen by the user via the native save dialog. The file
/// write lives in the Rust core; the UI only supplies the picked path.
#[tauri::command]
fn save_text_file(path: String, contents: String) -> Result<(), String> {
    std::fs::write(&path, contents).map_err(|e| e.to_string())
}

/// Current toggle hotkey spec (persisted, or the default).
#[tauri::command]
fn get_hotkey(db: State<'_, Db>) -> Result<String, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    Ok(db::get_setting(&conn, "hotkey")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| DEFAULT_HOTKEY.to_string()))
}

/// Re-register the global toggle hotkey and persist it. Takes effect immediately.
#[tauri::command]
fn set_hotkey(app: tauri::AppHandle, db: State<'_, Db>, spec: String) -> Result<(), String> {
    register_hotkey(&app, &spec)?;
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::set_setting(&conn, "hotkey", &spec).map_err(|e| e.to_string())
}

/// Back up the whole database to a user-chosen file (the DB is one portable
/// SQLite file — D4). The path comes from the UI's save dialog.
#[tauri::command]
fn backup_database(db: State<'_, Db>, path: String) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::backup_to(&conn, std::path::Path::new(&path)).map_err(|e| e.to_string())
}

/// Restore the database from a backup file, replacing current contents. The UI
/// should reload afterwards.
#[tauri::command]
fn restore_database(db: State<'_, Db>, path: String) -> Result<(), String> {
    let mut conn = db.lock().map_err(|e| e.to_string())?;
    db::restore_from(&mut conn, std::path::Path::new(&path)).map_err(|e| e.to_string())
}

/// Output of a step command run via the optional Run buttons.
#[derive(serde::Serialize)]
struct RunResult {
    stdout: String,
    stderr: String,
    #[serde(rename = "exitCode")]
    exit_code: Option<i32>,
}

/// Lossy-decode captured output, capped so a runaway command (`cat biglog`,
/// `find /`) can't balloon memory or jank the WebView. `from_utf8_lossy` makes
/// slicing at a byte boundary safe (invalid bytes become replacement chars).
fn cap_output(bytes: &[u8]) -> String {
    const MAX: usize = 256 * 1024; // 256 KB per stream
    if bytes.len() > MAX {
        let mut s = String::from_utf8_lossy(&bytes[..MAX]).into_owned();
        s.push_str("\n…output truncated…");
        s
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// Hard cap on a single `run_command` execution. A runbook holds short
/// operational commands; anything still alive after this is almost certainly
/// interactive or never-exiting (a dev server, `tail -f`), so we kill it rather
/// than leave the Run button stuck on "running…" forever.
const RUN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// Execute `text` in the user's login shell and capture its output (docs D11).
///
/// This is the one command that runs arbitrary code, so it's gated by the
/// `allow_run` setting: it refuses unless the user has explicitly enabled
/// command execution in Settings. The check is re-read from the DB here (not
/// just hidden in the UI) so the frontend can't bypass the gate. Commands run
/// with the user's own permissions — the trust boundary is the user's intent.
///
/// `(async)` is load-bearing: a plain sync `#[tauri::command]` runs on the **main
/// thread**, so the blocking wait below would freeze the whole overlay until the
/// command exits. `(async)` makes Tauri run this synchronous body on a worker
/// thread instead, keeping the UI live. The DB lock is released (scoped block)
/// before we spawn. A never-exiting command is killed after `RUN_TIMEOUT`.
#[tauri::command(async)]
fn run_command(
    db: State<'_, Db>,
    text: String,
    cwd: Option<String>,
) -> Result<RunResult, String> {
    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        if !db::run_allowed(&conn).map_err(|e| e.to_string())? {
            return Err("Command execution is disabled — enable it in Settings → Execution.".into());
        }
    }
    use std::io::Read;
    // `sh -c` (or $SHELL) so pipes, redirects, and env expansion behave like a
    // terminal.
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let mut cmd = std::process::Command::new(shell);
    cmd.arg("-c").arg(&text);
    // Run in the pinned project directory when one is supplied (docs D15).
    if let Some(dir) = cwd.as_deref().filter(|d| !d.is_empty()) {
        cmd.current_dir(dir);
    }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;

    // Drain each stream on its own thread: reading to EOF can't block the exit
    // poll, and it stops a command with >64KB of output from wedging on a full
    // OS pipe buffer (the classic capture deadlock).
    let mut out_pipe = child.stdout.take().unwrap();
    let mut err_pipe = child.stderr.take().unwrap();
    let out_h = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = out_pipe.read_to_end(&mut b);
        b
    });
    let err_h = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = err_pipe.read_to_end(&mut b);
        b
    });

    // Poll for exit until the deadline; kill on overrun so the button can't hang.
    let deadline = std::time::Instant::now() + RUN_TIMEOUT;
    let (status, timed_out) = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(s) => break (Some(s), false),
            None => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break (None, true);
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    };

    // The reader threads finish once the pipes close (on exit or kill), so the
    // joins yield whatever output was captured up to that point.
    let stdout = out_h.join().unwrap_or_default();
    let mut stderr = cap_output(&err_h.join().unwrap_or_default());
    if timed_out {
        if !stderr.is_empty() {
            stderr.push('\n');
        }
        stderr.push_str(&format!(
            "…timed out after {}s and was terminated…",
            RUN_TIMEOUT.as_secs()
        ));
    }
    Ok(RunResult {
        stdout: cap_output(&stdout),
        stderr,
        // None (rendered as "exit ?") when we killed it — the stderr note says why.
        exit_code: status.and_then(|s| s.code()),
    })
}

/// Replay progress per runbook (the in-progress badge on cards).
#[tauri::command]
fn list_progress(db: State<'_, Db>) -> Result<Vec<db::Progress>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::list_progress(&conn).map_err(|e| e.to_string())
}

/// Check / uncheck a step during a replay (docs D10).
#[tauri::command]
fn set_step_done(db: State<'_, Db>, step_id: i64, done: bool) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::set_step_done(&conn, step_id, done).map_err(|e| e.to_string())
}

/// Clear all replay progress for a runbook.
#[tauri::command]
fn reset_progress(db: State<'_, Db>, runbook_id: i64) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::reset_progress(&conn, runbook_id).map_err(|e| e.to_string())
}

/// Names of a runbook's saved variable profiles (docs D12).
#[tauri::command]
fn list_var_profiles(db: State<'_, Db>, runbook_id: i64) -> Result<Vec<String>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::list_var_profiles(&conn, runbook_id).map_err(|e| e.to_string())
}

/// The name->value map for one profile, or `None`.
#[tauri::command]
fn get_var_profile(
    db: State<'_, Db>,
    runbook_id: i64,
    name: String,
) -> Result<Option<HashMap<String, String>>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::get_var_profile(&conn, runbook_id, &name).map_err(|e| e.to_string())
}

/// Create or overwrite a profile. `values` must already exclude secret vars.
#[tauri::command]
fn save_var_profile(
    db: State<'_, Db>,
    runbook_id: i64,
    name: String,
    values: HashMap<String, String>,
) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::save_var_profile(&conn, runbook_id, &name, &values).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_var_profile(db: State<'_, Db>, runbook_id: i64, name: String) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::delete_var_profile(&conn, runbook_id, &name).map_err(|e| e.to_string())
}

// ----------------------------------------------------------------------------
// Git-backed sync (docs D16) — export all runbooks to a git repo and commit.
// ----------------------------------------------------------------------------

/// Run `git <args>` in `dir`, capturing output.
fn run_git(dir: &std::path::Path, args: &[&str]) -> Result<std::process::Output, String> {
    std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| format!("could not run git (is it installed?): {e}"))
}

/// `git <args>` that must succeed, surfacing stderr on failure.
fn git_ok(dir: &std::path::Path, args: &[&str]) -> Result<(), String> {
    let out = run_git(dir, args)?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Whether `name` looks like one of our exports (`<digits>-*.md`), so we only
/// ever delete files we created — never a user's own files in the sync folder.
fn is_export_filename(name: &str) -> bool {
    name.ends_with(".md")
        && name
            .split_once('-')
            .is_some_and(|(num, _)| !num.is_empty() && num.bytes().all(|b| b.is_ascii_digit()))
}

/// A filesystem-safe slug for a runbook filename.
fn slugify(title: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "runbook".to_string()
    } else {
        s
    }
}

/// Export every runbook to `<dir>/runbooks/<id>-<slug>.md`, then `git add`/commit
/// (and optionally `push`). To propagate deletions we first remove only our own
/// previously-exported files (`<digits>-*.md`) — never the whole directory — so a
/// deleted runbook drops out without clobbering any unrelated files the user keeps
/// there. `git init` runs first (a no-op on an existing repo). Returns a
/// human-readable summary for the UI.
#[tauri::command(async)]
fn git_sync(db: State<'_, Db>, dir: String, push: bool) -> Result<String, String> {
    let base = std::path::PathBuf::from(&dir);
    if !base.is_dir() {
        return Err(format!("Not a directory: {dir}"));
    }

    // Snapshot all runbooks as markdown, then drop the DB lock before touching
    // the filesystem / spawning git.
    let exports: Vec<(i64, String, String)> = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let runbooks = db::list_runbooks(&conn, None).map_err(|e| e.to_string())?;
        let mut v = Vec::with_capacity(runbooks.len());
        for rb in runbooks {
            if let Some(md) = db::export_markdown(&conn, rb.id).map_err(|e| e.to_string())? {
                v.push((rb.id, rb.title, md));
            }
        }
        v
    };

    // Write into a runbooks/ subdir. To propagate deletions, we remove our OWN
    // previously-exported files first — but ONLY those matching the export naming
    // (`<digits>-*.md`), never the whole directory. This is deliberate: the user
    // may have picked a folder that already holds unrelated files (even a
    // `runbooks/README.md`), and a blind `remove_dir_all` would destroy them.
    let books_dir = base.join("runbooks");
    std::fs::create_dir_all(&books_dir).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(&books_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if is_export_filename(&entry.file_name().to_string_lossy()) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    for (id, title, md) in &exports {
        let fname = format!("{id:04}-{}.md", slugify(title));
        std::fs::write(books_dir.join(fname), md).map_err(|e| e.to_string())?;
    }

    // init (idempotent) → add → commit (tolerate "nothing to commit").
    git_ok(&base, &["init", "-q"])?;
    git_ok(&base, &["add", "-A"])?;
    let msg = format!("runebook sync: {} runbook(s)", exports.len());
    let commit = run_git(&base, &["commit", "-m", &msg])?;
    let mut summary = if commit.status.success() {
        format!("Synced & committed {} runbook(s).", exports.len())
    } else {
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&commit.stdout),
            String::from_utf8_lossy(&commit.stderr)
        );
        if combined.contains("nothing to commit") {
            format!("Synced {} runbook(s) — no changes since last sync.", exports.len())
        } else {
            return Err(format!("git commit failed: {}", combined.trim()));
        }
    };

    if push {
        // Push even when nothing was committed this run — there may be earlier
        // local commits to send. The message avoids claiming a commit happened.
        let p = run_git(&base, &["push"])?;
        if p.status.success() {
            summary.push_str(" Pushed.");
        } else {
            return Err(format!(
                "{summary} But git push failed: {}",
                String::from_utf8_lossy(&p.stderr).trim()
            ));
        }
    }
    Ok(summary)
}

/// Whether the app is set to launch on login.
#[tauri::command]
fn get_autostart(app: tauri::AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

/// Enable/disable launch on login.
#[tauri::command]
fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let m = app.autolaunch();
    if enabled {
        m.enable().map_err(|e| e.to_string())
    } else {
        m.disable().map_err(|e| e.to_string())
    }
}

/// Summon the overlay if it isn't the focused foreground window; hide it only
/// when it already is. The single source of truth for the global hotkey and the
/// tray "Open" action.
///
/// Checking focus — not just visibility — fixes the summon-from-another-workspace
/// case: an overlay mapped on a *different* workspace still reports
/// `is_visible() == true`, so a visibility-only toggle would *hide* it (off where
/// you can't see it) instead of bringing it forward, which looks exactly like
/// "the hotkey does nothing". So: focused → hide; otherwise show + raise + focus.
fn toggle_overlay(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let visible = win.is_visible().unwrap_or(false);
        let focused = win.is_focused().unwrap_or(false);
        if visible && focused {
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = win.unminimize();
            let _ = win.set_focus();
            // Tell the UI it was just summoned so it lands on Quick-add with the
            // cursor in the composer (Phase 4 capture flow).
            let _ = app.emit("overlay:show", ());
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                // Only one shortcut is ever registered (the toggle, possibly
                // customized), so any press should toggle the overlay.
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_overlay(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            list_runbooks,
            get_runbook,
            create_runbook,
            update_runbook,
            delete_runbook,
            add_step,
            update_step,
            reorder_steps,
            delete_step,
            quick_add,
            get_setting,
            set_setting,
            copy_to_clipboard,
            export_markdown,
            save_text_file,
            get_hotkey,
            set_hotkey,
            get_autostart,
            set_autostart,
            backup_database,
            restore_database,
            run_command,
            list_progress,
            set_step_done,
            reset_progress,
            list_var_profiles,
            get_var_profile,
            save_var_profile,
            delete_var_profile,
            git_sync,
        ])
        .setup(move |app| {
            // Open the local SQLite database in the app-data dir, run migrations,
            // and share the connection with the IPC commands. Data persists at
            // <app-data>/runebook.db (docs/02-data-model.md).
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let conn = db::open(&data_dir.join("runebook.db"))
                .map_err(|e| format!("failed to open database: {e}"))?;
            app.manage(Mutex::new(conn));

            // Register the global toggle hotkey from the persisted setting, or the
            // default, falling back to the default if a saved spec is unparseable.
            let spec = {
                let db = app.state::<Db>();
                let conn = db.lock().map_err(|e| e.to_string())?;
                db::get_setting(&conn, "hotkey")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| DEFAULT_HOTKEY.to_string())
            };
            match register_hotkey(app.handle(), &spec) {
                Ok(()) => eprintln!("[runebook] global hotkey registered: {spec}"),
                Err(e) => {
                    eprintln!(
                        "[runebook] failed to register hotkey '{spec}': {e}; \
                         falling back to {DEFAULT_HOTKEY}"
                    );
                    if let Err(e2) = register_hotkey(app.handle(), DEFAULT_HOTKEY) {
                        eprintln!("[runebook] fallback hotkey registration also failed: {e2}");
                    }
                }
            }

            // Make the overlay appear on whatever workspace is active when summoned
            // (like a launcher), so the hotkey can't "open" it onto a workspace you
            // aren't looking at. Without this, an overlay shown on its original
            // workspace stays there and the summon looks like it did nothing.
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.set_visible_on_all_workspaces(true);
            }

            // System tray: Open / Quit. Lives on after the window hides.
            let open_i = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_i, &quit_i])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Runebook — Ctrl+Alt+Space")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        // Closing the window hides it to the tray instead of quitting, so the
        // global hotkey keeps working.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Runebook");
}

#[cfg(test)]
mod tests {
    use super::{cap_output, is_export_filename, slugify};

    #[test]
    fn export_filename_matches_only_our_exports() {
        // Ours: zero-padded id (or any digits) + '-' + slug + .md
        assert!(is_export_filename("0001-deploy.md"));
        assert!(is_export_filename("42-rotate-creds.md"));
        // A user's own files in the sync folder must be left alone.
        assert!(!is_export_filename("README.md"));
        assert!(!is_export_filename("my-notes.md")); // prefix isn't all digits
        assert!(!is_export_filename("notes.txt"));
        assert!(!is_export_filename("-leading.md")); // empty numeric prefix
        assert!(!is_export_filename("0001-deploy.txt"));
    }

    #[test]
    fn slugify_is_filesystem_safe() {
        assert_eq!(slugify("Deploy via SSH"), "deploy-via-ssh");
        assert_eq!(slugify("Rotate Postgres creds!"), "rotate-postgres-creds");
        assert_eq!(slugify("   "), "runbook"); // empty → fallback, never ""
        // No path separators survive, so an export filename can't escape the dir.
        assert_eq!(slugify("../etc/passwd"), "etc-passwd");
        assert!(!slugify("a/b\\c").contains('/'));
    }

    #[test]
    fn cap_output_truncates_large_streams() {
        let small = b"hello";
        assert_eq!(cap_output(small), "hello");
        let big = vec![b'x'; 300 * 1024];
        let out = cap_output(&big);
        assert!(out.len() < big.len());
        assert!(out.ends_with("…output truncated…"));
    }
}
