# runebook-mcp

A [Model Context Protocol](https://modelcontextprotocol.io) (stdio) server that
exposes the local **Runebook** database to AI tools — Claude Code, Cursor, Claude
Desktop, etc. Connect it and an agent can search your runbooks, read their steps,
and capture new procedures, all from your editor or terminal.

It reuses Runebook's own data layer ([`../src-tauri/src/db.rs`](../src-tauri/src/db.rs),
included via `#[path]`) so the SQL schema, migrations, and FTS5 search are
single-sourced — and it has **no Tauri/webkit dependency** (just `rusqlite` +
`serde`), so it builds headless.

## Build

```bash
cargo build --release      # → target/release/runebook-mcp
runebook-mcp --help        # usage + DB path resolution
```

## Connect (Claude Code)

```bash
claude mcp add runebook -- "$(pwd)/target/release/runebook-mcp"
```

For Cursor and other clients, the full config + the tool reference live in
[`../docs/06-mcp-server.md`](../docs/06-mcp-server.md).

## Database

Defaults to the same file the app uses
(`~/.local/share/com.runebook.app/runebook.db`); override with `--db <PATH>` or
`RUNEBOOK_DB`. WAL mode lets the server and the overlay app share it concurrently.
