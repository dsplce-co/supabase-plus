# supabase-plus

🚀 Extra tools for managing Supabase projects — going beyond the regular Supabase CLI.

`supabase-plus` (`spb`) is a command-line utility that extends the official Supabase CLI with additional project management capabilities

At the moment the main feature is the ability to stop any running Supabase project with a single command without a need to:

- Figuring out what other project is running
- Navigating to its directory (or finding its slug) to stop it

⸻

## 🖤 Features

✅ Stop any running Supabase project with a single command<br>
✅ Self-upgrade capability through cargo<br>
✅ Shell completion support<br>
✅ Works alongside existing Supabase CLI<br>

⸻

## 📦 Installation

Install from crates.io using cargo:

```bash
cargo install supabase-plus
```

After installation, the `spb` command will be available in your terminal.

⸻

## 🧪 Usage

### Stop Any Running Project

Quickly stop all running Supabase projects:

```bash
spb stop-any
```

This command will:

- Scan for running Supabase Docker containers
- Identify project IDs
- Stop each detected project using the official Supabase CLI (in theory there might be only one
  supabase project running but sometimes single containers from other projects haunt the docker
  runtime if the project hasn't been stopped properly)

### Self-Update

Keep your installation up to date:

```bash
spb upgrade
```

### Shell Completions

Generate shell completions for your preferred shell:

```bash
spb completions bash

# For zsh it tries to write a completion script to ~/.zsh/completion/_spb path by default in future
# there might be an option to automatically write the script for other shells too
spb completions zsh

# If you want to get the completion script just printed you can pass `-n` flag
spb completions zsh -n

spb completions fish

spb completions powershell
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
