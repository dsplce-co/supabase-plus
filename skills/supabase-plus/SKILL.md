---
name: supabase-plus
description: Diagnose and fix Supabase access and data problems with the `sbp` CLI. Use for `permission denied for table/sequence` errors, rows missing despite grants (RLS), debugging with production data locally, and verifying that a production backup restores.
---

# supabase-plus (`sbp`)

`sbp` extends the Supabase CLI. Run it from inside a Supabase project (a directory with `supabase/`). Check `sbp --version` first; if it's missing, point the user to https://github.com/dsplce-co/supabase-plus#-installation. Run `sbp <command> --help` for every flag.

## Pick the command

| Symptom | Command |
| --- | --- |
| `permission denied for table`, `permission denied for sequence` | `sbp grants`, then `sbp grants --fix` |
| Grants are fine, but rows don't show up or inserts get rejected | `sbp policy` |
| A bug that only reproduces on production data | `sbp clone` |
| "Does our backup actually restore?" | `sbp backup` |

## Production is the user's call

`--linked` and `--db-url` read the production database. Ask the user before any command that uses them, and say what it reads. Pass `--confirm-prod` (`clone`, `backup`) only after the user agreed to that exact production read in this conversation.

Connection secrets live in the environment: `SUPABASE_DB_PASSWORD` (with `--linked`), `SUPABASE_DB_URL`, `SUPABASE_SERVICE_ROLE_KEY`. Keep their values out of commands, output and files.

## `sbp grants`

Prints which of `anon`, `authenticated` and `service_role` can touch each table, view, sequence and function (`--schema` for another schema than `public`).

- The local database still auto-grants new objects by default, so a clean local matrix proves nothing about production. Use `--linked` to see the real gap (after asking, see above).
- Its warning suggests `supabase db reset`, which recreates the local database from migrations and erases local data. Leave that to the user.
- `--fix` writes a migration with the missing grants and never applies it to production; deploying is `supabase db push`, which the user runs.
- `--fix` on the local database asks "run immediately?", so ask the user to run that one in their terminal. `--fix --linked` only writes the file.
- Objects it lists as skipped (no RLS, `security definer`, views running as owner) need a human decision: open them only after the user confirms that exposing them is intended.

## `sbp policy`

Evaluates each RLS policy as `anon` and as a signed-in user; `--as <id or email>` evaluates it as a particular user, with `--table` and `--name` to narrow it.

- Cells show how many rows a policy lets through (`3/12`), `by row` for insert checks that depend on the inserted row, `no grant` when the role lacks the privilege (go to `sbp grants`).
- Each policy is evaluated on its own, so the result is that policy's contribution, not the table's final behaviour.
- `--linked` / `--db-url` needs the user's confirmation in a terminal and can't run from an agent shell: ask the user to run it and paste the output.

## `sbp clone`

Copies chosen tables, auth users and storage files from production to the local Supabase, with personal data masked.

- Non-interactive run: `sbp clone --tables <a,b> --buckets <x> --linked --confirm-prod --json`, add `--mask table.column,...` for personal columns. The JSON report and a non-zero exit code on failure make it checkable.
- Without a terminal it only warns about columns that look like personal data and copies them unmasked. Settle the `--mask` list with the user before running.
- `--out supabase/seed.sql` writes a seed instead of replacing local data.
- It replaces local data of the chosen tables and never writes to production. Tell the user which local tables will be overwritten before running it.
- Schema comes from migrations; edge functions, secrets and settings are out of scope.

## `sbp backup`

`sbp backup --out <dir> --linked --confirm-prod` dumps the database, roles and storage files, then restores the dump in a throwaway container and compares it with `manifest.json`.

- Needs Docker. `--no-verify` skips the restore.
- The backup is not masked. Choose a `--out` directory outside any git repository, or one that git ignores, and tell the user where the data ended up.
- Report the verification table to the user as it is: mismatching row counts or file sizes are the finding.
