# 06 — MCP server

`runebook-mcp` exposes the local Runebook database to AI tools that speak the
**Model Context Protocol** — Claude Code, Cursor, Claude Desktop, Windsurf, etc.
Once connected, an agent can search your saved runbooks, read their steps, and
capture new procedures while you work — the same data the overlay shows, reachable
from your editor or terminal.

Lives in [`mcp-server/`](../mcp-server/). See [05-decisions.md](05-decisions.md) **D10**
for why it's a standalone process rather than embedded in the app.

## How it fits

```
┌──────────────────┐     stdio (JSON-RPC)     ┌───────────────────┐
│ MCP client       │ ───────────────────────▶ │ runebook-mcp      │
│ (Claude Code,    │ ◀─────────────────────── │ (this crate)      │
│  Cursor, …)      │   spawns on demand       └─────────┬─────────┘
└──────────────────┘                                    │ rusqlite
                                                         ▼
                              ┌──────────────────────────────────────┐
   ┌───────────────────┐      │  <app-data>/runebook.db  (one file)  │
   │ Runebook overlay  │─────▶│  WAL mode → both can read/write       │
   │ app (Tauri)       │      └──────────────────────────────────────┘
   └───────────────────┘
```

- **Standalone, no running app required.** The client launches `runebook-mcp` as
  a stdio subprocess when it needs it; the server opens the SQLite file directly
  and exits when the client disconnects. The overlay app does **not** have to be
  open.
- **One source of truth for the data layer.** The server `#[path]`-includes the
  app's own [`src-tauri/src/db.rs`](../src-tauri/src/db.rs), so schema, migrations,
  and the FTS5 ranked search are shared verbatim — no second copy of the SQL to
  drift. The binary has **no Tauri/webkit dependency** (just `rusqlite` + `serde`).
- **Concurrency.** `db::open` puts the database in **WAL** mode with a 5s
  `busy_timeout`, so the app and the MCP server can read and write the same file
  at once without `SQLITE_BUSY` (D10).

## Build

```bash
cd mcp-server
cargo build --release
# binary → mcp-server/target/release/runebook-mcp
```

No webkit/GTK needed for this crate — it builds headless. (On a fresh box you only
need a Rust toolchain; `bundled` compiles SQLite from source.)

## Database path

Resolved in this order (first match wins):

1. `--db <PATH>` command-line flag
2. `RUNEBOOK_DB=<PATH>` environment variable
3. **default:** `$XDG_DATA_HOME/com.runebook.app/runebook.db`, falling back to
   `~/.local/share/com.runebook.app/runebook.db` — i.e. exactly what the Tauri
   app's `app_data_dir()` resolves to, so by default the server and the app share
   one file.

`runebook-mcp --help` prints this. If the database or its directory doesn't exist
yet, the server creates it (running the full migration chain), so the first step
you capture from an agent works even before you've opened the app.

## Connecting it

The server is a stdio MCP server: point any client at the built binary's absolute
path. Replace the path below with your checkout's
`…/mcp-server/target/release/runebook-mcp`.

### Claude Code

```bash
claude mcp add runebook -- /ABSOLUTE/PATH/runebook/mcp-server/target/release/runebook-mcp
```

…or commit a project-scoped [`.mcp.json`](https://docs.claude.com/en/docs/claude-code/mcp):

```json
{
  "mcpServers": {
    "runebook": {
      "command": "/ABSOLUTE/PATH/runebook/mcp-server/target/release/runebook-mcp"
    }
  }
}
```

### Cursor

Add to `~/.cursor/mcp.json` (global) or `.cursor/mcp.json` (per-project):

```json
{
  "mcpServers": {
    "runebook": {
      "command": "/ABSOLUTE/PATH/runebook/mcp-server/target/release/runebook-mcp"
    }
  }
}
```

### Any other MCP client (Claude Desktop, Windsurf, …)

Same shape — a stdio server with a `command` (the binary) and optional `args` /
`env`. To point at a non-default database:

```json
{
  "mcpServers": {
    "runebook": {
      "command": "/ABSOLUTE/PATH/.../runebook-mcp",
      "args": ["--db", "/custom/path/runebook.db"]
    }
  }
}
```

## Tools

| Tool | Args | Returns |
|------|------|---------|
| `list_runbooks` | `query?` | JSON array of runbook metadata (no step bodies). With `query`, FTS5-ranked across title/description/tags/step bodies. |
| `get_runbook` | `id` | JSON runbook with all steps (`id`, `position`, `title`, `body`). |
| `export_runbook_markdown` | `id` | One portable Markdown document for the runbook. |
| `create_runbook` | `title`, `description?`, `tags?` | `{ ok, id }` |
| `update_runbook` | `id`, `title?`, `description?`, `tags?` | `{ ok, id }` (only passed fields change; `tags` replaces the set) |
| `delete_runbook` | `id` | `{ ok, id }` — **destructive** (cascades to steps) |
| `add_step` | `runbook_id`, `title?`, `body?` (at least one of title/body) | `{ ok, id }` — appends to the end |
| `update_step` | `id`, `title?`, `body?` | `{ ok, id }` |
| `delete_step` | `id` | `{ ok, id }` — **destructive** |

A **step** is an optional short `title` plus a free-form markdown `body` (at least
one must be non-empty); put shell commands in fenced ` ``` ` code blocks so the app
keeps them one-click copyable (D8). Tool execution errors come back as a normal
result with `isError: true` so the model can react, rather than crashing the call —
and updating/deleting a **non-existent id errors** ("No runbook/step with id N")
rather than silently reporting success.

The server advertises **only** the `tools` capability — no resources or prompts.
It echoes the client's `protocolVersion` only if it's one it knows
(`2025-06-18`, `2025-03-26`, `2024-11-05`), otherwise it answers with its latest.

## Read-only mode

Set **`RUNEBOOK_MCP_READONLY=1`** (any value except empty / `0` / `false`) to run
the server without the mutating tools: only `list_runbooks`, `get_runbook`, and
`export_runbook_markdown` are advertised and accepted — `create`/`update`/`delete`
are hidden from `tools/list` and refused if called anyway. Use it to let an agent
search and read your runbooks without any chance of changing them:

```json
{
  "mcpServers": {
    "runebook": {
      "command": "/ABSOLUTE/PATH/.../runebook-mcp",
      "env": { "RUNEBOOK_MCP_READONLY": "1" }
    }
  }
}
```

## Security notes

- By default the server exposes **full read/write CRUD**, including delete. MCP
  clients gate every tool call behind user approval, so the agent can't silently
  mutate or wipe your runbooks — but treat granting access like giving write access
  to the file. For read-only access, set `RUNEBOOK_MCP_READONLY=1` (above).
- Steps can contain hosts, keys, and other secrets (see open question Q4). The
  database is plaintext; an agent with this server connected can read all of it.
  Connect it only in tools/projects you trust.
- The server only ever touches the one SQLite file — no shell, no network.

## Verifying

`cargo build --release` validates the build. End-to-end, pipe newline-delimited
JSON-RPC into the binary against a throwaway DB (stdout is the protocol channel,
stderr is logs):

```bash
RUNEBOOK_DB=/tmp/smoke.db ./target/release/runebook-mcp <<'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{}}}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"create_runbook","arguments":{"title":"Deploy"}}}
EOF
```
