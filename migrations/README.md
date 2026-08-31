# `migrations/`

SQL schema migrations, applied by `sqlx migrate run` (or by `admet-api` at
start-up when `database.migrate_on_start` is true).

**Empty until Increment 2.** `just db-migrate` exits 0 with "no migrations yet"
rather than failing, and `admet-api` logs `no migrations to apply yet` — both are
deliberate, so an empty directory does not look like a broken one.

## Why this directory is at the repo root

`implementation.md` §8 sketched it under `crates/admet-db/migrations/`. It lives
here instead because three separate tools resolve `./migrations` from the
directory they are invoked in, and all three are invoked from the repo root:

| Tool | Default path |
|---|---|
| `sqlx migrate run` | `./migrations` |
| `sqlx migrate add` | `./migrations` |
| CI job / docker compose | `./migrations` |

Overriding all three with `--source` on every invocation is three chances to
forget. `admet_db::MIGRATIONS_DIR` names this path in one place so nothing has to
guess.

## Planned migrations

| File | Increment | Contents |
|---|---|---|
| `0001_initial.sql` | 2 | `molecules`, `endpoints`, `model_versions`, `predictions`, `prediction_values`, `batches` |
| `0002_auth.sql` | 3 | `users`, `sessions`, `projects`; `batches.project_id` FK |
| `0003_indexes.sql` | 4 | the indexes the measured query plans actually ask for |

`0003` is separate on purpose. Adding an index before there is a slow query is
guessing, and an unused index still costs every write. Increment 4 runs `EXPLAIN
ANALYZE` first and adds only what the plan justifies — that is the evidence the
performance chapter needs.

## Conventions

- **Forward-only.** No `down` files. A down migration that has never been run in
  anger does not work, and believing otherwise during an incident is worse than
  knowing you have to roll forward.
- **One concern per file.** A migration that adds a table and backfills it cannot
  be reviewed, because the interesting part is buried.
- **`CHAR(27)` for `inchikey`.** An InChIKey is exactly 27 characters
  (`14-10-1`), so the fixed-width type is a free constraint. See ADR-04.
- **Never edit an applied migration.** `sqlx` stores a checksum; changing a file
  that has run makes every subsequent `migrate run` fail with a mismatch. Write
  `0004_fix_whatever.sql` instead.
- **`sqlx migrate add -r false <name>`** to create one, so the filename gets the
  monotonic timestamp prefix that ordering depends on.
