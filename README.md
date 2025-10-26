# supabase-plus

⚡ Extra tools for managing Supabase projects — going beyond the regular Supabase CLI.

`supabase-plus` (`sbp`) is a command-line utility that extends the official Supabase CLI with additional project management capabilities

⸻

## 🖤 Features

🛑 Stop any running Supabase project with a single command<br>
🪣 Create migration files for creating new buckets via an interactive CLI<br>

## 🍩 Other traits

👨‍💻 Shell completion support<br>
☯️ Works alongside existing Supabase CLI<br>

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
