# Runebook

A system-wide **runbook scratchpad**. Summon a frameless, always-on-top overlay
with a global hotkey from anywhere on your desktop, capture the steps of a task
*as you do it* as free-form **markdown** notes, then replay those steps later —
one-click copy each command — instead of re-reading docs and code snippets every
time.

> Born from a real pain: deploying an app over SSH the second time and having to
> re-derive every step from scratch.

## Install

**Linux (Debian / Ubuntu / Pop!_OS, x86-64).** Grab the `.deb` from the
[latest release](https://github.com/AjasMohammed/RuneBook/releases/latest) and
install it straight from GitHub:

```bash
# Download the package
curl -L -o runebook.deb \
  https://github.com/AjasMohammed/RuneBook/releases/download/v0.1.0/Runebook_0.1.0_amd64.deb

# Install it — apt pulls in the webkit2gtk / appindicator / gtk runtime deps
sudo apt install ./runebook.deb
```

> Prefer `dpkg`? `sudo dpkg -i runebook.deb && sudo apt-get install -f` does the
> same thing, resolving the dependencies in the second step.

This installs the `runebook` binary and a desktop entry. Launch **Runebook** from
your application menu (or run `runebook`), then press **Ctrl+Alt+Space** to toggle
the overlay — **Esc** hides it, and the tray icon has Open / Quit. The window
starts hidden and lives in the tray.

Runtime dependencies (resolved automatically by `apt`): `libwebkit2gtk-4.1-0`,
`libayatana-appindicator3-1`, `libgtk-3-0`. To uninstall: `sudo apt remove runebook`.

### Updating

Update an installed copy to the latest release with one command:

```bash
npm run update     # from a checked-out repo

# …or without the repo — download, read it, then run (don't blind-pipe to bash):
curl -fsSL https://raw.githubusercontent.com/AjasMohammed/RuneBook/main/scripts/update.sh -o update.sh
less update.sh && bash update.sh
```

It checks the latest GitHub Release, compares it to your installed version, verifies
the download against the release's `SHA256SUMS`, and — only if it's newer, after one
`sudo` prompt — installs the new `.deb`. `--check` reports without installing (exit
`10` if an update is available); `--yes` skips the prompt.

### Build from source

Requires Rust, Node 22, and the Tauri Linux dev libraries
(`libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
build-essential curl wget file`):

```bash
source ~/.nvm/nvm.sh && nvm use 22
npm install
npm run tauri build      # bundles a .deb under src-tauri/target/release/bundle/deb/
# …or for development:
npm run tauri dev        # Vite (port 1420) + the overlay
```

### Releasing (maintainers)

Releases are built and published by GitHub Actions. To cut one:

```bash
npm run release 0.2.0    # bumps all four version files, commits, tags v0.2.0, pushes
```

Pushing the `v*` tag triggers [`.github/workflows/release.yml`](.github/workflows/release.yml),
which builds the Linux `.deb` (plus the standalone `runebook-mcp` binary and a
`SHA256SUMS`) and attaches them to a GitHub Release for the tag. `npm run release`
refuses a dirty tree or an existing tag and keeps `package.json`,
`src-tauri/tauri.conf.json`, and both `Cargo.toml`s in lockstep (`--dry-run` to
preview, `--no-push` to tag locally). Every push/PR to `main` is gated by
[`.github/workflows/ci.yml`](.github/workflows/ci.yml) — version-consistency, `fmt`,
`clippy`, tests, and the frontend + release build.

## Core idea

- **Runbook** — a named procedure ("Deploy via SSH", "Rotate Postgres creds").
- **Step** — one ordered chunk of a runbook: an optional title + a free-form
  markdown body (commands go in ```` ``` ```` fenced blocks).
- **Replay** — search → open runbook → copy each command (one click per code
  block) and go.

## Stack (decided)

- **Tauri** (Rust core + web UI) — ~5MB bundle, native global hotkey, tray, and
  transparent always-on-top overlay.
- **SQLite** via `rusqlite` (bundled) behind Rust IPC commands — local,
  queryable (FTS5), search across runbooks. *Not* `tauri-plugin-sql`: all SQL
  stays in the Rust core so the UI never touches the DB (see
  [docs/05-decisions.md](docs/05-decisions.md) D7).
- **Web UI** — **Svelte 4 + Vite**, styled with the existing typography system.
- **Target OS** — Linux / Pop!_OS on **X11** (confirmed) — overlay + global
  shortcuts work without Wayland workarounds.

## Docs

| File | What it covers |
|------|----------------|
| [docs/01-architecture.md](docs/01-architecture.md) | Process model, crates, plugins, IPC |
| [docs/02-data-model.md](docs/02-data-model.md) | SQLite schema, entities, queries |
| [docs/03-overlay-and-ux.md](docs/03-overlay-and-ux.md) | Window behavior, hotkeys, capture & replay flows |
| [docs/04-roadmap.md](docs/04-roadmap.md) | Phased build plan, milestones, acceptance |
| [docs/05-decisions.md](docs/05-decisions.md) | Decision log (ADRs) and open questions |
| [docs/06-mcp-server.md](docs/06-mcp-server.md) | MCP server — connect runbooks to Claude Code / Cursor |

## Status

All five v1 phases are implemented: overlay shell + global hotkey + tray (0–1);
the SQLite data layer with full runbook/step CRUD behind IPC commands (2); **FTS5
ranked search** + per-code-block copy + tag filtering (3); **Quick-add** capture
(4) — the hotkey lands on a keyboard-only composer (`⌘↵` save & next) appending to
a persisted "current runbook"; and **polish & export** (5) — Markdown export
(copy / save `.md`), launch-on-login, and a Settings panel (custom hotkey + accent
theme). Steps are free-form **markdown**; the **Browse** view renders them with a
copy button on every fenced code block.

Beyond v1, **Phase 6 (Advanced)** adds: **executable steps** (an opt-in ▶ run
button per code block that runs the command and shows its output inline),
**replay sessions** (work a runbook as a persistent checklist with a progress bar
that resumes where you left off), and **variable profiles** (named value sets like
prod/staging for the `{{var}}` placeholders, with secret vars that are masked and
never persisted). See [docs/04-roadmap.md](docs/04-roadmap.md) Phase 6.

**Phase 8 (Advanced, Tier 2)** adds a **command palette** (`⌘K` to jump to any
runbook), **project pinning** (pin a runbook to a folder; its Run commands execute
there), and **git-backed sync** (Settings → export every runbook to a git repo and
commit/push). An AI assistant is intentionally deferred for now.

An optional **MCP server** ([`mcp-server/`](mcp-server/)) exposes the same SQLite
database over the Model Context Protocol, so AI tools (Claude Code, Cursor, …) can
search and capture runbooks from your editor — see
[docs/06-mcp-server.md](docs/06-mcp-server.md).

Verified via `cargo test` + `npm run build`; a live `npm run tauri dev` needs a
graphical X11 session. Remaining work is iterative polish + the "Later" backlog in
[docs/04-roadmap.md](docs/04-roadmap.md).
