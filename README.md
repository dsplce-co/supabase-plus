> We're dsplce.co, check out our work on [github.com/dsplce-co](https://github.com/dsplce-co) 🖤

# supabase-plus

[![Supabase](https://img.shields.io/badge/Supabase-3ECF8E?style=for-the-badge&logo=supabase&logoColor=white)](https://supabase.com/)
[![crates.io Size](https://img.shields.io/crates/d/supabase-plus?style=for-the-badge&color=%23FF0346)](https://crates.io/crates/supabase-plus)
[![crates.io Size](https://img.shields.io/crates/size/supabase-plus?style=for-the-badge)](https://crates.io/crates/supabase-plus)
[![License](https://img.shields.io/crates/l/supabase-plus.svg?style=for-the-badge)](https://crates.io/crates/supabase-plus)
[![crates.io](https://img.shields.io/crates/v/supabase-plus?style=for-the-badge&color=%230F80C1)](https://crates.io/crates/supabase-plus)

⚡ Extra tools for managing Supabase projects — going beyond the regular Supabase CLI.

`supabase-plus` (`sbp`) is a command-line utility that extends the official Supabase CLI with additional project management capabilities

_Disclamer: this project has no affiliation with the official Supabase project or trademark._

## Table of Contents

- [🖤 Features](#-features)
- [🍩 Other traits](#-other-traits)
- [📦 Installation](#-installation)
- [🧪 Usage](#-usage)
  - [Stop any running project](#stop-any-running-project)
  - [Create storage buckets](#create-storage-buckets)
  - [Manage realtime subscriptions](#manage-realtime-subscriptions)
  - [Store RPC-s in repo](#store-rpc-s-in-repo)
  - [Shell completions](#shell-completions)
  - [Self-update](#self-update)
- [🛠️ Requirements](#%EF%B8%8F-requirements)
- [📁 Repo & Contributions](#-repo--contributions)
- [📄 License](#-license)

⸻

## 🖤 Features

🛑 Stop any running Supabase project with a single command<br>
🪣 Creating new buckets via an interactive CLI and have a migration generated automatically<br>
🧩 Store RPC-s in repo as SQL files and use `watch` subcommand to write them to db on file change<br>

## 🍩 Other traits

- Shell completion support
- Works alongside existing Supabase CLI

⸻

## 📦 Installation

Install from crates.io using cargo:

```bash
cargo install supabase-plus
```

After installation, the `sbp` command will be available in your terminal.

⸻

## 🧪 Usage

### Stop any running project

Quickly stop all running Supabase projects:

```bash
sbp stop-any
```

This command will:

- Scan for running Supabase Docker containers
- Identify project IDs
- Stop each detected project using the official Supabase CLI (in theory there might be only one
  supabase project running but sometimes single containers from other projects haunt the docker
  runtime if the project hasn't been stopped properly)

This way you're gaining an ability to stop any running Supabase project with a single command without the need of:

- Figuring out what other project is running
- Navigating to its directory (or finding its slug) to stop it

### Create storage buckets

Interactively create new storage buckets with automatic migration generation:

```bash
sbp create bucket
```

This command will:

- Guide you through bucket configuration with an interactive prompt
- Set bucket name/slug
- Configure visibility (public/private)
- Optionally set MIME type restrictions by file extension
- Generate a timestamped migration file in `supabase/migrations/`
- Optionally apply the migration immediately to your local database (so it might be your main workflow for new buckets given that buckets are stored as records in `"storage"."buckets"` and `supabase db diff` only compares schemas ignorging data entirely)

### Manage realtime subscriptions

Interactively enable/disable realtime on your tables and store those changes in your project's migration files:

```bash
sbp manage realtime
```

This command will:

- Display all tables in the specified schema (defaults to `public`)
- Show current realtime subscription status for each table
- Allow you to interactively select/deselect tables for realtime
- Generate appropriate SQL to add/remove tables from the `supabase_realtime` publication
- Create a timestamped migration file in `supabase/migrations/`
- Optionally apply the migration immediately to your local database

You can specify a different schema:

```bash
sbp manage realtime --schema <schema_name>
```

Example output SQL:

```sql
alter publication supabase_realtime add table "public"."users", "public"."posts";
alter publication supabase_realtime drop table "public"."old_table";
```

This provides a much more dev-friendly way to manage realtime subscriptions compared to manually writing SQL or using the Supabase dashboard on each environment, especially when you need to enable/disable realtime for multiple tables at once.

### Store RPC-s in repo

Monitor SQL files in a directory and automatically write them to the database when they change:

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

The `--immediate` (or `-I`) flag will execute all existing SQL files in the directory intially on the command start.

**Example file:**

> rpc/hello_world.sql

```
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

- **docker socket**: Properly working `/var/run/docker.sock` on Unix-based systems and `\\.\pipe\docker_engine` on Windows
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
