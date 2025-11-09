> We're dsplce.co, check out our work on [github.com/dsplce-co](https://github.com/dsplce-co) 🖤

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

- `sbp stop-any` Ever been working on multiple projects? No clue which to stop to start the current? Here's the picklock
- `sbp create bucket` Had buckets locally once, never found them in prod at the end? Here's the command you "forgot" to run
- `sbp watch ./rpc -I` Stop fighting the teeny-tiny studio editor and store your rpcs in the repo like a human

And others like:

- `sbp manage realtime`
- `sbp manage migrations`

---

## Table of Contents

- [🖤 Features](#-features)
- [📦 Installation](#-installation)
  - [cargo](#cargo)
- [🧪 Usage](#-usage)
  - [Stop any running project](#stop-any-running-project)
  - [Create storage buckets interactively](#create-storage-buckets-interactively)
  - [Manage realtime switches interactively](#manage-realtime-switches-interactively)
  - [Store RPC-s in repo](#store-rpc-s-in-repo)
  - [Shell completions](#shell-completions)
  - [Self-update](#self-update)
- [🛠️ Requirements](#%EF%B8%8F-requirements)
- [📁 Repo & Contributions](#-repo--contributions)
- [📄 License](#-license)

⸻

## 📦 Installation

### cargo

Install from crates.io using cargo:

```bash
cargo install supabase-plus
```

Alternatively you can find pre-built binaries for your platform on [GitHub](https://github.com/dsplce-co/supabase-plus/releases).

After installation, the `sbp` command will be available in your terminal.

### Homebrew

Install from our tap:

```
brew install dsplce-co/tap/supabase-plus
```

This makes the `sbp` command available in your terminal.

### apt

Coming soon

### AUR repository

Coming soon

⸻

## 🧪 Usage

### Stop any running project

If you ever worked on multiple "Supa-based" projects you probably encountered this scenario where:

1. You wanted to `supabase start`
2. Then got an error saying some Supabase's already running

And then you had no clue which one; we've all been there, I'm not gonna even describe my ways of figuring this out, just run this, they're encapsulated in a single command:

```bash
sbp stop-any
```

But if you're really curious here you go, it:

- Scans for running Supabase Docker containers
- Identifies project IDs
- Stops each detected project using the official Supabase CLI (in theory there might be only one
  supabase project running but sometimes single containers from other projects haunt the docker
  runtime if the project hasn't been stopped properly)

![](./assets/stop-any-demo.gif)

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
