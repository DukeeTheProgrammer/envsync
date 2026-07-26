use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

fn git_cmd(args: &[&str], cwd: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .context("Failed to execute git. Is git installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git {} failed: {}", args.join(" "), stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[allow(dead_code)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub name: String,
    pub is_main: bool,
}

pub fn find_main_worktree(start_path: &Path) -> Result<WorktreeInfo> {
    let toplevel = git_cmd(&["rev-parse", "--show-toplevel"], start_path)
        .context("Not inside a git repository. Run this command from within a git repo.")?;
    let toplevel = PathBuf::from(toplevel.trim());

    // Find the shared git directory
    let common_dir = git_cmd(&["rev-parse", "--git-common-dir"], start_path)
        .context("Failed to find git common directory")?;
    let common_dir = PathBuf::from(common_dir.trim());

    // The main worktree root is the parent of the common .git directory
    let main_root = if common_dir.is_absolute() {
        common_dir
            .parent()
            .context("Failed to determine main worktree root")?
            .to_path_buf()
    } else {
        toplevel
            .join("..")
            .join(&common_dir)
            .parent()
            .context("Failed to determine main worktree root")?
            .to_path_buf()
    };

    // Canonicalize if possible
    let main_root = main_root.canonicalize().unwrap_or(main_root);

    Ok(WorktreeInfo {
        path: main_root,
        name: "main".to_string(),
        is_main: true,
    })
}

pub fn list_worktrees(start_path: &Path) -> Result<Vec<WorktreeInfo>> {
    let output = git_cmd(&["worktree", "list", "--porcelain"], start_path)
        .context("Failed to list git worktrees")?;

    let mut worktrees: Vec<WorktreeInfo> = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_name: Option<String> = None;

    for line in output.lines() {
        if line.starts_with("worktree ") {
            if let Some(path) = current_path.take() {
                let name = current_name.take().unwrap_or_else(|| {
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                });
                let is_main = worktrees.is_empty();
                worktrees.push(WorktreeInfo {
                    path,
                    name,
                    is_main,
                });
            }
            let path_str = line.strip_prefix("worktree ").unwrap_or("");
            current_path = Some(PathBuf::from(path_str));
            current_name = None;
        } else if line.starts_with("head ")
            || line.starts_with("branch ")
            || line.starts_with("bare ")
            || line.starts_with("prunable ")
            || line.starts_with("locked ")
        {
            // skip metadata lines
        } else if !line.is_empty() {
            // This is the directory line (first line in porcelain output)
            if current_path.is_none() {
                current_path = Some(PathBuf::from(line.trim()));
            }
        }
    }

    // Push the last entry
    if let Some(path) = current_path.take() {
        let name = current_name.take().unwrap_or_else(|| {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });
        let is_main = worktrees.is_empty();
        worktrees.push(WorktreeInfo {
            path,
            name,
            is_main,
        });
    }

    Ok(worktrees)
}

pub fn get_current_worktree_root(path: &Path) -> Result<PathBuf> {
    let toplevel =
        git_cmd(&["rev-parse", "--show-toplevel"], path).context("Not inside a git repository.")?;
    Ok(PathBuf::from(toplevel.trim()))
}

#[allow(dead_code)]
pub fn is_main_worktree(path: &Path) -> Result<bool> {
    let common_dir = git_cmd(&["rev-parse", "--git-common-dir"], path)?;
    let common_dir = common_dir.trim();

    // In the main worktree, --git-common-dir returns "."
    // In linked worktrees, it returns a relative path like "../.git"
    Ok(common_dir == ".")
}
