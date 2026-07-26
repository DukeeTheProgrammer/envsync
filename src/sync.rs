use anyhow::{Context, Result};
use colored::*;
use glob::glob;

use std::fs;
use std::path::{Path, PathBuf};

use crate::envfile::{diff_env, merge_env, EnvDiff, EnvFile};

pub fn find_env_files(project_root: &Path) -> Result<Vec<PathBuf>> {
    let mut env_files: Vec<PathBuf> = Vec::new();

    // Root-level patterns
    let patterns = vec![
        ".env",
        ".env.local",
        ".env.development",
        ".env.staging",
        ".env.production",
    ];

    for pattern in &patterns {
        let path = project_root.join(pattern);
        if path.exists() {
            env_files.push(path);
        }
    }

    // Glob for .env.* files
    let glob_pattern = project_root.join(".env.*");
    if let Some(glob_str) = glob_pattern.to_str() {
        if let Ok(paths) = glob(glob_str) {
            for path in paths.flatten() {
                if path.is_file() && !env_files.contains(&path) {
                    env_files.push(path);
                }
            }
        }
    }

    // Monorepo: check apps/*/, packages/*/, libs/*/
    let monorepo_dirs = vec!["apps", "packages", "libs", "services"];
    for dir in &monorepo_dirs {
        let dir_path = project_root.join(dir);
        if dir_path.is_dir() {
            if let Ok(entries) = fs::read_dir(&dir_path) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        for pattern in &[".env", ".env.local"] {
                            let env_path = entry.path().join(pattern);
                            if env_path.exists() && !env_files.contains(&env_path) {
                                env_files.push(env_path);
                            }
                        }
                    }
                }
            }
        }
    }

    env_files.sort();
    env_files.dedup();
    Ok(env_files)
}

#[allow(dead_code)]
pub struct SyncResult {
    pub file: PathBuf,
    pub action: SyncAction,
    pub diff_output: Option<String>,
}

#[derive(Debug)]
pub enum SyncAction {
    Copied,
    Merged,
    Unchanged,
    Conflict(String),
}

pub fn sync_files(source: &Path, target: &Path, dry_run: bool) -> Result<SyncResult> {
    let env_file = EnvFile::parse(source)?;
    let target_exists = target.exists();

    if !target_exists {
        if dry_run {
            return Ok(SyncResult {
                file: target.to_path_buf(),
                action: SyncAction::Copied,
                diff_output: None,
            });
        }

        fs::copy(source, target).with_context(|| {
            format!(
                "Failed to copy {} to {}",
                source.display(),
                target.display()
            )
        })?;
        return Ok(SyncResult {
            file: target.to_path_buf(),
            action: SyncAction::Copied,
            diff_output: None,
        });
    }

    let target_env = EnvFile::parse(target)?;
    let diffs = diff_env(&env_file, &target_env);

    if diffs.is_empty() {
        return Ok(SyncResult {
            file: target.to_path_buf(),
            action: SyncAction::Unchanged,
            diff_output: None,
        });
    }

    // Check if there are real conflicts (target has values different from source)
    let has_conflicts = diffs
        .iter()
        .any(|d| d.kind == crate::envfile::DiffKind::Changed);

    if has_conflicts {
        let diff_output = format_diff(&diffs);
        if !dry_run {
            return Ok(SyncResult {
                file: target.to_path_buf(),
                action: SyncAction::Conflict(diff_output),
                diff_output: None,
            });
        }
        return Ok(SyncResult {
            file: target.to_path_buf(),
            action: SyncAction::Conflict(diff_output),
            diff_output: None,
        });
    }

    // Only additions/removals, safe to merge
    if dry_run {
        return Ok(SyncResult {
            file: target.to_path_buf(),
            action: SyncAction::Merged,
            diff_output: Some(format_diff(&diffs)),
        });
    }

    let merged = merge_env(&env_file, &target_env)?;
    fs::write(target, merged)
        .with_context(|| format!("Failed to write merged file: {}", target.display()))?;

    Ok(SyncResult {
        file: target.to_path_buf(),
        action: SyncAction::Merged,
        diff_output: Some(format_diff(&diffs)),
    })
}

pub fn format_diff(diffs: &[EnvDiff]) -> String {
    let mut output = String::new();

    for diff in diffs {
        match diff.kind {
            crate::envfile::DiffKind::Added => {
                output.push_str(&format!(
                    "{} {}={}\n",
                    "+".green().bold(),
                    diff.key,
                    diff.new_value.as_deref().unwrap_or("")
                ));
            }
            crate::envfile::DiffKind::Removed => {
                output.push_str(&format!(
                    "{} {}={}\n",
                    "-".red().bold(),
                    diff.key,
                    diff.old_value.as_deref().unwrap_or("")
                ));
            }
            crate::envfile::DiffKind::Changed => {
                output.push_str(&format!(
                    "{} {}={} -> {}\n",
                    "~".yellow().bold(),
                    diff.key,
                    diff.old_value.as_deref().unwrap_or(""),
                    diff.new_value.as_deref().unwrap_or("")
                ));
            }
        }
    }

    output
}

pub fn apply_conflict_resolution(source: &Path, target: &Path, strategy: &str) -> Result<()> {
    match strategy {
        "source" => {
            fs::copy(source, target).context("Failed to overwrite target with source")?;
        }
        "merge" => {
            let source_env = EnvFile::parse(source)?;
            let target_env = EnvFile::parse(target)?;
            let merged = merge_env(&source_env, &target_env)?;
            fs::write(target, merged)?;
        }
        _ => {
            anyhow::bail!("Unknown strategy: {}. Use 'source' or 'merge'", strategy);
        }
    }
    Ok(())
}
