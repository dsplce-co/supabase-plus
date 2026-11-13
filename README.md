> We're dsplce.co, check out our work on our website: [dsplce.co](https://dsplce.co) 🖤

# supabase-plus

[![Supabase](https://img.shields.io/badge/Supabase-3ECF8E?style=for-the-badge&logo=supabase&logoColor=white)](https://supabase.com/)
[![crates.io Size](https://img.shields.io/crates/d/supabase-plus?style=for-the-badge&color=%23FF0346)](https://crates.io/crates/supabase-plus)
[![crates.io Size](https://img.shields.io/crates/size/supabase-plus?style=for-the-badge)](https://crates.io/crates/supabase-plus)
[![License](https://img.shields.io/crates/l/supabase-plus.svg?style=for-the-badge)](https://crates.io/crates/supabase-plus)
[![crates.io](https://img.shields.io/crates/v/supabase-plus?style=for-the-badge&color=%230F80C1)](https://crates.io/crates/supabase-plus)

⚡ Extra tools for managing Supabase projects — going beyond the regular Supabase CLI.

`supabase-plus` (`sbp`) is a batteries-included command-line utility that extends the official Supabase CLI with additional project management capabilities

_Disclaimer: this project has no affiliation with the official Supabase project or trademark._

![Demo](./assets/overview-demo.gif)

## 🖤 Features

- `sbp clone` Bug only shows up on prod data? Bring the data, users and files over to your local Supabase, personal data masked on the way
- `sbp grants` New table, `permission denied` in your app? See who can touch what and get the missing grants as a migration
- `sbp policy` Grants are there but rows still don't show up? See what your RLS policies really let through for anon, any signed in user or a particular one
- `sbp backup` Is your backup really a backup? Save production, files included, and restore it in a throwaway database to find out
- `sbp running` Just want to peek at which project is up without stopping anything? This one tells you
- `sbp create bucket` Had buckets locally once, never found them in prod at the end? Here's the command you "forgot" to run
- `sbp watch ./rpc -I` Stop fighting the teeny-tiny studio editor and store your rpcs in the repo like a human

And others like:

- `sbp manage realtime`
- `sbp manage migrations`

---

## Table of Contents

- [🖤 Features](#-features)
- [📦 Installation](#-installation)
  - [nix](#nix)
  - [cargo](#cargo)
  - [Homebrew](#homebrew)
  - [.deb file](#deb-file)
  - [apt](#apt)
  - [AUR repository](#aur-repository)
- [🧪 Usage](#-usage)
  - [Show the running project](#show-the-running-project)
  - [Clone production state locally](#clone-production-state-locally)
  - [Check who has access to what](#check-who-has-access-to-what)
  - [Check what your RLS policies let through](#check-what-your-rls-policies-let-through)
  - [Back up production and check it restores](#back-up-production-and-check-it-restores)
  - [Create storage buckets interactively](#create-storage-buckets-interactively)
  - [Manage realtime switches interactively](#manage-realtime-switches-interactively)
  - [Store RPC-s in repo](#store-rpc-s-in-repo)
  - [Agent skill](#agent-skill)
  - [Shell completions](#shell-completions)
  - [Self-update](#self-update)
- [🛠️ Requirements](#%EF%B8%8F-requirements)
- [📁 Repo & Contributions](#-repo--contributions)
- [📄 License](#-license)

⸻

## 📦 Installation

### nix

Install to your profile with:

```bash
nix profile add github:dsplce-co/supabase-plus
```

or just run with
```bash
nix run github:dsplce-co/supabase-plus
```

### cargo

Install from crates.io using cargo:

```bash
cargo install supabase-plus
```

Alternatively you can find pre-built binaries for your platform on [GitHub](https://github.com/dsplce-co/supabase-plus/releases).

After installation, the `sbp` command will be available in your terminal.

### Homebrew

Install from our tap:

```bash
brew install dsplce-co/tap/supabase-plus
```

This makes the `sbp` command available in your terminal.

### `.deb` file

You can find the latest `.deb` file on [GitHub Releases](https://github.com/dsplce-co/supabase-plus/releases/latest).

### apt

Coming soon

### AUR repository

Coming soon

⸻

## 🧪 Usage

### Show the running project

Sometimes you don't want to stop anything, you just want to know which project currently holds the local Supabase runtime. Run:

```bash
sbp running
```

It:

- Scans for running Supabase Docker containers
- Identifies project IDs
- Prints each detected project (in a healthy state there's exactly one)

"But doesn't `supabase status` already do this?" — sort of, as long as you're standing in exactly the right folder. The official command is a bit near-sighted: it only ever reports the project whose directory you're currently in, and huffs an error if you're not in one (or if nothing's running at all). So the one question you actually had — _which of my projects is up right now?_ — is the one it can't answer the moment you've `cd`'d elsewhere and can't remember which repo it was.

`sbp running` doesn't care where you're standing. It goes straight to Docker, scans every Supabase container on the whole machine, and just tells you which project(s) are up — no `cd`-ing around required.

It's purely read-only, it never touches your lovely containers.

### Clone production state locally

Migrations give you the schema, but not the _state_ of production: the data, the users you can sign in as and the files your rows point at

```bash
sbp clone --tables users,orders --buckets avatars
```

This command will:

- Copy auth users (with password hashes), the data of chosen tables and the files of chosen buckets from production to your local Supabase, replacing what was there for those tables
- Mask personal data on the way (see below), real values never reach your disk
- Show an estimate of the transfer first, as it counts towards your egress limit
- Compare the files with what production holds at the end, and tell what changed in the meantime

Run it without flags to pick tables and buckets interactively. Useful flags:

- `--where "orders:created_at > now() - interval '30 days'"` and `--limit` take only some rows
- `--mask users.email,users.phone` masks text columns, `clone` also points out the ones which look like personal data
- `--out supabase/seed.sql` writes a seed for `supabase db reset` instead of loading the local database
- `--no-auth` leaves users out, `--json` prints a report for CI and agents (non-zero exit code on failure, `--confirm-prod` replaces the question)

Already ran `supabase link`? Add `--linked` and set `SUPABASE_DB_PASSWORD` (the same variable the Supabase CLI reads), the database is taken from the linked project, and so is the address of its API for files (they still need `--service-role-key` or `SUPABASE_SERVICE_ROLE_KEY`). It can't be combined with `--db-url` and `--project-url`, nor with the variables behind them.

Otherwise point it at production with `--db-url` (or `SUPABASE_DB_URL`), and for files also with `--project-url` and `--service-role-key` (or `SUPABASE_URL`, `SUPABASE_SERVICE_ROLE_KEY`). Use the direct connection or the session mode pooler, and a role which bypasses RLS (like `postgres`), otherwise rows hidden by policies are silently missing from the copy. Masks can also be listed in `sbp.toml`, so the whole team masks the same way:

```toml
[clone]
mask = ["users.email", "users.phone"]
```

Fakes are the same for the same real value, so relations and unique constraints keep working, e-mails in Auth are always masked. Production is only read, in a single snapshot, and local data is replaced in one transaction.

It doesn't copy the schema (that's what migrations are for), edge functions, secrets or settings (use `supabase pull`), and it never writes to production.

### Check who has access to what

Since 30 October new objects in `public` are no longer granted to the API roles automatically, so a fresh table can end with `permission denied` until someone writes a `grant`. Run:

```bash
sbp grants
```

It prints a matrix of `anon`, `authenticated` and `service_role` against your tables, views, sequences and functions (add `--schema` for another schema than `public`):

- `S` `I` `U` `D` stand for select, insert, update and delete, `U` also for usage on sequences, `X` for execute, lowercase means just some columns
- Columns with their own grants get a separate row
- The last column tells whether an object is guarded: RLS for tables, `security_invoker` for views, `security definer` for functions

To check production instead of the local database add `--linked` (the project from `supabase link`, with the password in `SUPABASE_DB_PASSWORD`) or `--db-url`. That's the place where the gaps actually show, `--fix` still only writes a migration file, which you deploy with `supabase db push`.

By default your local Supabase still grants new objects to these roles by itself, so it won't show what production is going to miss. Set `auto_expose_new_tables = false` under `[api]` in `supabase/config.toml` and run `supabase db reset` to make it behave the same, `sbp grants` warns you about it.

To get the missing ones as a migration run:

```bash
sbp grants --fix
```

It:

- Touches only objects nobody has access to yet, so what you granted on purpose stays as it is
- Gives `anon` read access and `authenticated` with `service_role` full access to tables, read access to views, `usage` on sequences (inserts into `serial` columns fail without it) and `execute` to functions
- Leaves out tables without RLS, views running as their owner and `security definer` functions, as opening them could expose data, they're listed instead
- Generates a timestamped migration file in `supabase/migrations/` and optionally applies it to your local database

### Check what your RLS policies let through

A grant only opens the door to a table, the policies decide which rows are behind it. And as a policy can be any piece of SQL, often checking who is asking, the only way to know what it does is to run it. Run:

```bash
sbp policy
```

It evaluates every policy for an anonymous visitor and for any signed in user (one that owns nothing and isn't a member of anything). To see it from the point of view of a particular user add `--as` with their id or email:

```bash
sbp policy --table orders --as jane@example.com
```

Narrow it down with `--schema` (`public` by default), `--table` (just the name, without schema) and `--name`. Each cell tells you:

- `3/12` how many of the table's rows the policy lets through (`using` expression), for select, update, delete and all
- `yes` or `no` for insert policies whose `with check` doesn't look at the inserted row, `by row` for the ones that do, as the answer depends on what's being inserted
- `-` when the policy isn't for that role, `no grant` when the role has no privilege for the command at all (see [`sbp grants`](#check-who-has-access-to-what))

Every policy is evaluated on its own, as the real role with JWT claims built from the user's row in `auth.users` (so nothing session specific like `aal`), so what you see is what the policy contributes, not what the table ends up allowing after all permissive policies are combined with the restrictive ones. Policies of tables with RLS turned off are listed too, they just have no effect. Nothing is written: every evaluation runs in a read-only transaction that is rolled back and limited to 30 seconds per statement.

Add `--linked` or `--db-url` to evaluate against production. It's the one thing that makes `sbp` open a temporary view in a remote session (gone with the transaction), so it asks for your confirmation first.

![](./assets/policy-demo.gif)

### Back up production and check it restores

A backup nobody ever restored is a hope, not a backup. Run:

```bash
sbp backup --out ~/backups/shop-2026-11-10
```

This command will:

- Dump the whole production database (schema, data, auth users) with `pg_dump` to `database.dump`, the roles (without passwords) with `pg_dumpall` to `roles.sql`, and download the files of every storage bucket to `storage/<bucket>/`
- Write `manifest.json` with the row count of each of your tables plus `auth.users` and `storage.objects`, and the size and SHA-256 of each file
- Restore the dump in a disposable `supabase/postgres` container and compare it with the manifest: the row counts of those tables and the sizes of the files on disk against the restored `storage.objects`, shown as a table
- Remove the container, nothing of your local project is touched

`pg_dump` runs in a container of the same Postgres version as production, taken from the project linked with `supabase link`, so all you need is Docker (the `docker` command). Add `--no-verify` to skip the restore. Connecting to production works as in `sbp clone` (`--linked`, or `--db-url` with `--project-url` and `--service-role-key` for files), and so does `--confirm-prod`.

The backup is **not masked**, so a directory inside a git repository has to be ignored by it, and it has to be empty. `roles.sql` lists all roles including the ones Supabase reserves, drop those before running it on a project. The trial restore leaves out owners and grants, as the roles of the Supabase services don't exist in the disposable database, they're kept in the dump. Files are checked against the data, not through the Storage API.

### Create storage buckets interactively

You might also have had this situation where you got your buckets created, you were happy, and then after merging to prod and having (db-diff-generated) migrations run, you've realised (or your client has), your buckets were either your imagination or db diff just simply didn't reflect them

Well, it didn't and it won't as they're stored as records in a table (`"storage"."buckets"`) and are not part of any schema

```bash
sbp create bucket
```

This command will:

- Guide you through bucket configuration with an interactive prompt
- Set bucket name/slug
- Configure visibility (public/private)
- Optionally set MIME type restrictions by file extension
- Generate a timestamped migration file in `supabase/migrations/`
- Optionally apply the migration immediately to your local database (recommended)

![](./assets/create-bucket-demo.gif)

### Manage realtime switches interactively

Another entity which is db-diff-immune are realtime switches on tables, they're neither schema nor data, but are bound to a publication feature of Postgres, long story short, run:

```bash
sbp manage realtime
```

This command will:

- Display all tables in the specified schema (defaults to `public`)
- Show current realtime subscription status for each table
- Allow you to interactively select/deselect tables for realtime
- Generate appropriate SQL to add/remove tables from the `supabase_realtime` publication
- Create a timestamped migration file in `supabase/migrations/`
- Optionally apply the migration immediately to your local database (recommended)

![](./assets/manage-rt-demo.gif)

### Store RPC-s in repo

Change my mind but chasing the latest version of an RPC in the depths of Postgres or the latest migration containing it (mild-panic) or copying it from the studio's RPC editor aren't things I like to do. Especially in order to edit it in an untitled file in my editor and then paste it back and execute in db to sync any change

C'mon, I'm lazy I don't want to leave my editor

Why not store them in repo to watch them and execute automatically on change?

```bash
sbp watch ./rpc
```

This command will:

- Watch for changes to `.sql` files in the specified directory
- Automatically execute modified SQL files as database queries
- Useful for storing RPC functions in a repository and keeping them synced with your local database
- Before pushing your changes you can just run the regular `supabase db diff -f <migration_name>`
  to generate a migration that will reflect on remote environments

You can also run all SQL files immediately when starting the watcher:

```bash
sbp watch ./rpc --immediate
```

The `--immediate` (or `-I`) flag will execute all existing SQL files in the directory initially on the command start.

**Example file:**

`rpc/hello_world.sql`:

```sql
drop function if exists public.hello_world;

create function public.hello_world(name text)
 returns boolean
 language plpgsql
 security definer
as $function$
  declare
    greeting text;
  begin
  end;
$function$;
```

_Please note that there is a `drop` statement at the beginning of the file. This is necessary to ensure that the function is dropped before it is recreated. In the future we plan to add `--autodrop` flag to automatically generate and run drop statements before applying the file's SQL behind the scenes._

![](./assets/watch-demo.gif)

### Agent skill

Want your AI coding agent to reach for `sbp` on its own when it hits `permission denied` or a backup you can't trust? Install the skill:

```bash
npx skills add dsplce-co/supabase-plus
```

It covers `sbp grants`, `sbp policy`, `sbp clone` and `sbp backup`: which one fits which symptom, and that anything touching production is left to your say-so.

### Shell completions

Generate shell completions for your preferred shell:

```bash
sbp completions bash

# For zsh it tries to write a completion script to ~/.zsh/completion/_sbp path by default in future
# there might be an option to automatically write the script for other shells too
sbp completions zsh

# If you want to get the completion script just printed you can pass `-n` flag
sbp completions zsh -n

sbp completions fish

sbp completions powershell
```

### Self-update

Keep your installation up to date:

```bash
sbp upgrade
```

⸻

## 🛠️ Requirements

- **docker socket**: Properly working `/var/run/docker.sock` on Unix-based systems and `\\.\pipe\docker_engine` on Windows, and the `docker` command for `sbp backup`
- **npx**: For running `supabase` CLI commands when needed (in the future there will be an option of customising the source of this command), it usually comes with Node.js installation
- **cargo**: For installation and self-updates

⸻

## 📁 Repo & Contributions

🛠️ **Repo**: [https://github.com/dsplce-co/supabase-plus](https://github.com/dsplce-co/supabase-plus)<br>
📦 **Crate**: [https://crates.io/crates/supabase-plus](https://crates.io/crates/supabase-plus)

PRs welcome, feel free to contribute

⸻

## 📄 License

MIT or Apache-2.0, at your option.
