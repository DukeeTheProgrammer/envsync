mod config;
mod envfile;
mod error;
mod sync;
mod worktree;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use std::path::Path;
use std::process;

#[derive(Parser)]
#[command(
    name = "envsync",
    about = "Sync .env files across git worktrees",
    version,
    long_about = "envsync detects .env files in your git repository and keeps them synchronized across all worktrees. Never lose your environment configuration when switching branches."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show the current sync status
    Status {
        /// Show detailed info
        #[arg(short, long)]
        verbose: bool,
    },

    /// Sync .env files from main worktree to linked worktrees
    Sync {
        /// Sync all linked worktrees (not just current)
        #[arg(long)]
        all: bool,

        /// Show what would be synced without making changes
        #[arg(short, long)]
        dry_run: bool,

        /// On conflict, use the source (main) version
        #[arg(long)]
        use_source: bool,

        /// On conflict, merge both versions
        #[arg(long)]
        merge: bool,
    },

    /// Show differences between main and current worktree .env files
    Diff {
        /// Show the main version
        #[arg(long)]
        main: bool,
    },

    /// Initialize envsync config in current project
    Init {
        /// Force overwrite existing config
        #[arg(short, long)]
        force: bool,
    },

    /// Install git hook for automatic sync on checkout
    InstallHook {
        /// Uninstall the hook
        #[arg(long)]
        uninstall: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        error::print_error(&e);
        process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Status { verbose } => cmd_status(verbose),
        Commands::Sync {
            all,
            dry_run,
            use_source,
            merge,
        } => cmd_sync(all, dry_run, use_source, merge),
        Commands::Diff { main } => cmd_diff(main),
        Commands::Init { force } => cmd_init(force),
        Commands::InstallHook { uninstall } => cmd_install_hook(uninstall),
    }
}

fn cmd_status(verbose: bool) -> Result<()> {
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;
    let worktrees =
        worktree::list_worktrees(&current_dir).context("Failed to list git worktrees")?;

    println!();
    println!("{} {}", "envsync".cyan().bold(), "status".cyan());
    println!("{}", "─".repeat(50).dimmed());

    let main_info = worktree::find_main_worktree(&current_dir)?;
    let env_files = sync::find_env_files(&main_info.path)?;

    if env_files.is_empty() {
        println!(
            "{} No .env files found in main worktree: {}",
            "warning:".yellow().bold(),
            main_info.path.display()
        );
        return Ok(());
    }

    println!(
        "{} {} .env file(s) in main worktree\n",
        "Found".green().bold(),
        env_files.len()
    );

    for wt in &worktrees {
        let marker = if wt.is_main {
            " (main)".dimmed().to_string()
        } else {
            String::new()
        };
        println!(
            "{} {}{}",
            "Worktree:".cyan(),
            wt.path.display().to_string().white(),
            marker
        );

        if verbose && !wt.is_main {
            for env_file in &env_files {
                let relative = env_file.strip_prefix(&main_info.path).unwrap_or(env_file);
                let target = wt.path.join(relative);

                if target.exists() {
                    let source_env = envfile::EnvFile::parse(env_file)?;
                    let target_env = envfile::EnvFile::parse(&target)?;
                    let diffs = envfile::diff_env(&source_env, &target_env);

                    if diffs.is_empty() {
                        println!("  {} {}", relative.display(), "in sync".green());
                    } else {
                        println!(
                            "  {} {} {} difference(s)",
                            relative.display(),
                            "out of sync".red(),
                            diffs.len()
                        );
                    }
                } else {
                    println!("  {} {}", relative.display(), "missing".yellow());
                }
            }
        }
        println!();
    }

    Ok(())
}

fn cmd_sync(all: bool, dry_run: bool, use_source: bool, merge: bool) -> Result<()> {
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;
    let main_info =
        worktree::find_main_worktree(&current_dir).context("Failed to find main worktree")?;
    let env_files = sync::find_env_files(&main_info.path)?;

    if env_files.is_empty() {
        println!(
            "{} No .env files found in main worktree.",
            "warning:".yellow().bold()
        );
        return Ok(());
    }

    let worktrees = worktree::list_worktrees(&current_dir)?;

    println!();
    println!(
        "{} {}",
        "envsync".cyan().bold(),
        if dry_run {
            "sync (dry run)".cyan()
        } else {
            "sync".cyan()
        }
    );
    println!("{}", "─".repeat(50).dimmed());

    let mut sync_count = 0;
    let mut conflict_count = 0;

    for wt in &worktrees {
        if wt.is_main {
            continue;
        }

        if !all && wt.path != current_dir && !is_ancestor(&main_info.path, &wt.path) {
            continue;
        }

        println!(
            "\n{} {}",
            "Syncing worktree:".cyan(),
            wt.path.display().to_string().white()
        );

        for env_file in &env_files {
            let relative = env_file.strip_prefix(&main_info.path).unwrap_or(env_file);
            let target = wt.path.join(relative);

            match sync::sync_files(env_file, &target, dry_run) {
                Ok(result) => match result.action {
                    sync::SyncAction::Copied => {
                        println!("  {} {}", "copied".green(), relative.display());
                        sync_count += 1;
                    }
                    sync::SyncAction::Merged => {
                        println!("  {} {}", "merged".green(), relative.display());
                        if let Some(diff) = &result.diff_output {
                            println!("{}", diff);
                        }
                        sync_count += 1;
                    }
                    sync::SyncAction::Unchanged => {
                        println!("  {} {}", "unchanged".dimmed(), relative.display());
                    }
                    sync::SyncAction::Conflict(diff) => {
                        conflict_count += 1;
                        if use_source || merge {
                            let strategy = if use_source { "source" } else { "merge" };
                            sync::apply_conflict_resolution(env_file, &target, strategy)?;
                            println!(
                                "  {} {} ({})",
                                "resolved".green(),
                                relative.display(),
                                strategy
                            );
                        } else {
                            println!("  {} {}", "conflict".red().bold(), relative.display());
                            println!("{}", diff);
                        }
                    }
                },
                Err(e) => {
                    println!("  {} {}: {}", "error".red().bold(), relative.display(), e);
                }
            }
        }
    }

    println!();
    if conflict_count == 0 {
        println!(
            "{} Synced {} file(s).",
            "success:".green().bold(),
            sync_count
        );
    } else {
        println!(
            "{} Synced {} file(s), {} conflict(s). Use {} or {} to resolve.",
            "warning:".yellow().bold(),
            sync_count,
            conflict_count,
            "--use-source".cyan(),
            "--merge".cyan()
        );
    }

    Ok(())
}

fn is_ancestor(_parent: &Path, _child: &Path) -> bool {
    // Simple heuristic: check if the child path starts with the parent path's parent
    // This is a simplified check; a full implementation would use git2
    true // Default to syncing all worktrees for now
}

fn cmd_diff(_main_only: bool) -> Result<()> {
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;
    let main_info = worktree::find_main_worktree(&current_dir)?;
    let current_root = worktree::get_current_worktree_root(&current_dir)?;

    if main_info.path == current_root {
        println!(
            "{} You are in the main worktree. Nothing to diff.",
            "info:".blue().bold()
        );
        return Ok(());
    }

    let env_files = sync::find_env_files(&main_info.path)?;

    if env_files.is_empty() {
        println!(
            "{} No .env files found in main worktree.",
            "warning:".yellow().bold()
        );
        return Ok(());
    }

    println!();
    println!("{} {}", "envsync".cyan().bold(), "diff".cyan());
    println!("{}", "─".repeat(50).dimmed());
    println!(
        "{} {}",
        "Main:".dimmed(),
        main_info.path.display().to_string().white()
    );
    println!(
        "{} {}",
        "Current:".dimmed(),
        current_root.display().to_string().white()
    );
    println!();

    for env_file in &env_files {
        let relative = env_file.strip_prefix(&main_info.path).unwrap_or(env_file);
        let target = current_root.join(relative);

        if !target.exists() {
            println!(
                "{} {} (not present in current worktree)",
                relative.display().to_string().yellow(),
                "MISSING".red().bold()
            );
            continue;
        }

        let source_env = envfile::EnvFile::parse(env_file)?;
        let target_env = envfile::EnvFile::parse(&target)?;
        let diffs = envfile::diff_env(&source_env, &target_env);

        if diffs.is_empty() {
            println!("{} {} (in sync)", relative.display(), "OK".green());
        } else {
            println!(
                "{} {} difference(s):",
                relative.display(),
                diffs.len().to_string().yellow()
            );
            println!("{}", sync::format_diff(&diffs));
        }
    }

    Ok(())
}

fn cmd_init(force: bool) -> Result<()> {
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;
    let config_path = current_dir.join(".envsync.toml");

    if config_path.exists() && !force {
        println!(
            "{} Config already exists. Use --force to overwrite.",
            "warning:".yellow().bold()
        );
        return Ok(());
    }

    let default_config = r#"# envsync configuration
# Docs: https://github.com/dukeetheprogrammer/envsync

[envsync]
# Files to sync (glob patterns)
include = [".env", ".env.local", ".env.*"]

# Files to never sync (gitignore-style)
ignore = []

# Per-worktree variable overrides
# [envsync.overrides]
# PORT = { base = 3000, increment = 1 }
# DB_NAME = { base = "myapp_dev", suffix = "_wt{N}" }
"#;

    std::fs::write(&config_path, default_config).context("Failed to write config file")?;

    println!("{} Created .envsync.toml", "success:".green().bold());

    Ok(())
}

fn cmd_install_hook(uninstall: bool) -> Result<()> {
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;
    let hooks_dir = current_dir.join(".git").join("hooks");

    // Handle worktrees: .git is a file, not a directory
    let git_path = current_dir.join(".git");
    let hooks_dir = if git_path.is_file() {
        // This is a linked worktree, find the hooks dir from the git file
        let content = std::fs::read_to_string(&git_path)?;
        let git_dir = content
            .lines()
            .find(|l| l.starts_with("gitdir:"))
            .and_then(|l| l.strip_prefix("gitdir: "))
            .map(|s| s.trim())
            .context("Failed to parse .git file")?;

        // Go up from .git/worktrees/<name> to .git/hooks
        let git_dir_path = std::path::PathBuf::from(git_dir);
        git_dir_path
            .parent()
            .context("Failed to find git dir parent")?
            .parent()
            .context("Failed to find hooks dir")?
            .join("hooks")
    } else {
        hooks_dir
    };

    let hook_path = hooks_dir.join("post-checkout");

    if uninstall {
        if hook_path.exists() {
            let content = std::fs::read_to_string(&hook_path)?;
            if content.contains("envsync") {
                std::fs::remove_file(&hook_path)?;
                println!("{} Removed envsync hook", "success:".green().bold());
            } else {
                println!(
                    "{} Hook exists but doesn't contain envsync. Not removing.",
                    "warning:".yellow().bold()
                );
            }
        } else {
            println!("{} No post-checkout hook found.", "info:".blue().bold());
        }
        return Ok(());
    }

    // Install hook
    let hook_content = r#"#!/bin/sh
# envsync post-checkout hook - auto-sync .env files
envsync sync 2>/dev/null || true
"#;

    if hook_path.exists() {
        let existing = std::fs::read_to_string(&hook_path)?;
        if existing.contains("envsync") {
            println!("{} envsync hook already installed", "info:".blue().bold());
            return Ok(());
        }

        // Append to existing hook
        let mut new_content = existing;
        if !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        new_content.push_str("\n# envsync auto-sync\nenvsync sync 2>/dev/null || true\n");
        std::fs::write(&hook_path, new_content)?;
    } else {
        std::fs::write(&hook_path, hook_content)?;
    }

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&hook_path, perms)?;
    }

    println!("{} Installed post-checkout hook", "success:".green().bold());
    println!("  {} {}", "Hook:".dimmed(), hook_path.display());

    Ok(())
}
