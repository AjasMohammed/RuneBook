# 02 — Data model

Local SQLite. One file at `~/.local/share/runebook/runebook.db`.

## Entities

- **runbook** — a named procedure.
- **step** — one ordered chunk of a runbook: an optional title + a free-form
  **markdown** body (see [05-decisions.md](05-decisions.md) D8).
- **tag** + **runbook_tag** — many-to-many labels for filtering.

## Schema (current — after migration v2)

```sql
CREATE TABLE runbook (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  title       TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  created_at  TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE step (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  runbook_id  INTEGER NOT NULL REFERENCES runbook(id) ON DELETE CASCADE,
  position    INTEGER NOT NULL,           -- ordering within the runbook
  title       TEXT NOT NULL DEFAULT '',   -- optional short label, may be empty
  body        TEXT NOT NULL DEFAULT '',   -- free-form markdown (the content)
  created_at  TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE tag (
  id    INTEGER PRIMARY KEY AUTOINCREMENT,
  name  TEXT NOT NULL UNIQUE
);

CREATE TABLE runbook_tag (
  runbook_id INTEGER NOT NULL REFERENCES runbook(id) ON DELETE CASCADE,
  tag_id     INTEGER NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
  PRIMARY KEY (runbook_id, tag_id)
);

CREATE INDEX idx_step_runbook ON step(runbook_id, position);
```

## Migrations

Tracked by `PRAGMA user_version` in the Rust core (`src-tauri/src/db.rs`):

- **v1** — original schema with fixed step fields (command/why/where_ctx/example/note).
- **v2** — collapse those fields into a single markdown `body`: add `body`,
  backfill existing rows into markdown (command → fenced block, why → text,
  where/example → a list, note → blockquote), then `DROP` the old columns.
- **v3** — `setting` key/value store (current runbook, hotkey, accent, `allow_run`,
  per-runbook `secret_vars:<id>`).
- **v4** — FTS5 mirror of `step(title, body)` (skipped if FTS5 absent; see below).
- **v5** — `step_progress` for replay checklists ([05-decisions.md](05-decisions.md) D10).
- **v6** — `var_profile` for variable profiles (D12).
- **v7** — `runbook.project_dir` column for project pinning / run cwd (D15).

Code blocks live inside the markdown body itself (```` ``` ````), so commands no
longer need their own column — the UI renders them with a per-block copy button.

### Advanced tables (Phase 6)

```sql
-- Replay progress (D10): one mutable "done" flag per step, keyed by step id.
-- Cascades when a step (or its runbook) is deleted.
CREATE TABLE step_progress (
  step_id    INTEGER PRIMARY KEY REFERENCES step(id) ON DELETE CASCADE,
  done       INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Variable profiles (D12): named {{var}} value sets per runbook. `data` is a
-- JSON object of name->value; secret-marked vars are excluded before saving.
CREATE TABLE var_profile (
  runbook_id INTEGER NOT NULL REFERENCES runbook(id) ON DELETE CASCADE,
  name       TEXT NOT NULL,
  data       TEXT NOT NULL DEFAULT '{}',
  PRIMARY KEY (runbook_id, name)
);
```

`get_runbook` LEFT JOINs `step_progress` so each `step` carries a `done` flag;
`list_progress()` returns grouped `done/total` counts (only where `done > 0`) for
the in-progress badge on cards. Variable profiles are CRUD'd via
`list_var_profiles` / `get_var_profile` / `save_var_profile` / `delete_var_profile`.

## Full-text search (Phase 3 — implemented)

Search is the whole value of the tool, so step title/body is mirrored into an
FTS5 table (migration v4 in `db.rs`):

```sql
CREATE VIRTUAL TABLE step_fts USING fts5(
  title, body, content='step', content_rowid='id'
);
-- + AFTER INSERT/UPDATE/DELETE triggers on step keep step_fts in sync
--   (the 'delete' command form for external-content tables); backfilled with
--   INSERT INTO step_fts(step_fts) VALUES ('rebuild').
```

`list_runbooks(query)` ranks step matches by **bm25** and also matches runbook
title/description/tags; title matches sort first, then bm25, then recency. The
query string is tokenized into quoted prefix terms (`"ssh"*`) so partial typing
matches and punctuation can't break MATCH syntax.

**Gotchas baked into the query:** `bm25()` can't be used inside an aggregate or
across a join, so it's scored in an inner CTE marked `AS MATERIALIZED` (which
also stops SQLite from flattening that CTE back into the join). If FTS5 isn't
compiled into the SQLite build, migration v4 skips silently and search falls back
to a `LIKE '%term%'` across the same fields.

## Ordering & reorder

`position` is a plain integer per runbook. `reorder_steps(runbook_id, ordered_ids[])`
rewrites positions in a single transaction (`UPDATE step SET position = ? WHERE id = ?`).
Keep it simple now; switch to fractional/`LexoRank` keys only if drag-reorder of
huge runbooks ever feels slow.

## TypeScript shape (UI side)

```ts
type Step = {
  id: number;
  runbookId: number;
  position: number;
  title: string;      // optional label, may be ""
  body: string;       // free-form markdown
  done: boolean;      // replay progress (D10); false when no step_progress row
};

type Runbook = {
  id: number;
  title: string;
  description: string;
  tags: string[];
  projectDir: string;  // pinned dir / run cwd (D15); "" when unpinned, "" in list views
  steps: Step[];
  createdAt: string;
  updatedAt: string;
};
```

## Export format

`export_markdown(runbook_id)` renders a runbook to portable Markdown: the runbook
title as a heading, then each step as a numbered section (its title as a subhead,
its markdown body inline). Since step bodies are already markdown, export is
mostly concatenation. Lets a runbook live in a repo's `RUNBOOK.md` or be shared
with a teammate.
