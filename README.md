# envsync

Sync `.env` files across git worktrees. Never lose your environment configuration when switching branches.

## The Problem

Git worktrees give you multiple working directories, but `.env` files are gitignored. Every new worktree starts with no environment configuration. You end up manually copying `.env` files, forgetting variables, and debugging "why isn't my app connecting to the database?"

**envsync** fixes this by automatically detecting and syncing `.env` files from your main worktree to all linked worktrees.

## Installation

```bash
# One-liner (macOS / Linux)
curl -sSL https://raw.githubusercontent.com/DukeeTheProgrammer/envsync/main/install.sh | bash
```

This downloads the pre-built binary for your platform and installs it to `/usr/local/bin` (or `~/.local/bin` if no sudo). If no pre-built binary is available, it builds from source automatically (requires [Rust](https://rustup.rs)).

**Other methods:**

```bash
# cargo install
cargo install envsync

# Clone and run install script
git clone https://github.com/DukeeTheProgrammer/envsync.git
cd envsync && ./install.sh
```

## Quick Start

```bash
# From your main worktree:
envsync init          # Create .envsync.toml config

# From any linked worktree:
envsync sync          # Copy .env files from main worktree
envsync status        # See what's synced
envsync diff          # Compare with main worktree
```

## Commands

### `envsync status`

Show all worktrees and their sync status.

```bash
$ envsync status

envsync status
──────────────────────────────────────────────────
Found 2 .env file(s) in main worktree

Worktree: /path/to/main (main)

Worktree: /path/to/feature-branch
  .env in sync
  .env.local out of sync 2 difference(s)
```

Use `--verbose` to see per-file sync status for each worktree.

### `envsync sync`

Sync `.env` files from the main worktree to the current (or all) linked worktrees.

```bash
$ envsync sync

envsync sync
──────────────────────────────────────────────────

Syncing worktree: /path/to/feature-branch
  copied .env
  merged .env.local

success: Synced 2 file(s).
```

**Options:**

| Flag | Description |
|------|-------------|
| `--all` | Sync all linked worktrees, not just the current one |
| `--dry-run` | Preview changes without writing files |
| `--use-source` | On conflict, overwrite with main worktree version |
| `--merge` | On conflict, merge both versions (linked values take precedence for overrides) |

### `envsync diff`

Show differences between main and current worktree `.env` files.

```bash
$ envsync diff

envsync diff
──────────────────────────────────────────────────
Main: /path/to/main
Current: /path/to/feature-branch

.env 2 difference(s):
+ NEW_API_KEY=abc123
~ PORT=3000 -> 3001
```

**Legend:**
- `+` Added (exists in main, missing in current)
- `-` Removed (exists in current, missing in main)
- `~` Changed (different values)

### `envsync init`

Create a `.envsync.toml` config file in the current project.

```bash
$ envsync init

success: Created .envsync.toml
```

### `envsync install-hook`

Install a git `post-checkout` hook that automatically runs `envsync sync` when switching branches.

```bash
$ envsync install-hook

success: Installed post-checkout hook
  Hook: /path/to/.git/hooks/post-checkout

# To uninstall:
$ envsync install-hook --uninstall
```

## Configuration

Create `.envsync.toml` in your project root:

```toml
[envsync]
# Files to sync (glob patterns)
include = [".env", ".env.local", ".env.*"]

# Files to never sync
ignore = [".env.production"]

# Per-worktree variable overrides
[envsync.overrides]
PORT = { base = 3000, increment = 1 }
DB_NAME = { base = "myapp_dev", suffix = "_wt{N}" }
```

### Overrides

The `overrides` section lets you define variables that should differ per worktree:

- **`increment`**: Auto-increment a numeric value (3000, 3001, 3002, ...)
- **`suffix`**: Append a suffix with `{N}` as the worktree index

This prevents port and database collisions when running multiple worktrees simultaneously.

## Use Cases

### AI Agent Workflows

When running multiple AI coding agents (Claude Code, Cursor, Codex) in parallel worktrees, each agent needs its own environment to avoid port and database collisions:

```bash
# Create worktrees for each agent
git worktree add ../agent-feature -b agent-feature
git worktree add ../agent-bugfix -b agent-bugfix

# Sync env to each
cd ../agent-feature && envsync sync
cd ../agent-bugfix && envsync sync

# Each gets unique ports/DBs via overrides
```

### Team Development

When multiple developers share a repository with worktrees for different features, envsync ensures everyone has the same base `.env` while allowing local overrides.

### CI/CD

Use `--dry-run` in CI to verify `.env` files are in sync before deploying:

```bash
envsync sync --dry-run || (echo "Environment files out of sync!" && exit 1)
```

## How It Works

1. **Detects** the main worktree using `git rev-parse --git-common-dir`
2. **Finds** `.env` files in the main worktree (root + monorepo directories)
3. **Compares** with the target worktree's `.env` files
4. **Copies** missing files, **merges** additions, **flags** conflicts

## What envsync Does NOT Do

- **Does not** manage secrets or encrypt `.env` files (use SOPS or age)
- **Does not** install dependencies (use your package manager or workz)
- **Does not** manage Docker containers (use docker-compose)

## License

MIT
