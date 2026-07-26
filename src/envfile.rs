use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EnvEntry {
    pub key: String,
    pub value: String,
    pub raw_line: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EnvFile {
    pub path: std::path::PathBuf,
    pub entries: Vec<EnvEntry>,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiffKind {
    Added,
    Removed,
    Changed,
}

#[derive(Debug, Clone)]
pub struct EnvDiff {
    pub key: String,
    pub kind: DiffKind,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
}

impl EnvFile {
    pub fn parse(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read .env file: {}", path.display()))?;

        let mut entries = Vec::new();
        let lines: Vec<String> = content.lines().map(String::from).collect();

        for line in &lines {
            let trimmed = line.trim();

            // Skip empty lines, comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Handle export prefix
            let key_value = if let Some(stripped) = trimmed.strip_prefix("export ") {
                stripped
            } else {
                trimmed
            };

            // Split on first =
            if let Some(eq_pos) = key_value.find('=') {
                let key = key_value[..eq_pos].trim().to_string();
                let raw_value = key_value[eq_pos + 1..].trim().to_string();

                // Remove surrounding quotes
                let value = remove_quotes(&raw_value);

                entries.push(EnvEntry {
                    key,
                    value,
                    raw_line: line.clone(),
                });
            }
        }

        Ok(EnvFile {
            path: path.to_path_buf(),
            entries,
            lines,
        })
    }

    pub fn to_map(&self) -> HashMap<String, String> {
        self.entries
            .iter()
            .map(|e| (e.key.clone(), e.value.clone()))
            .collect()
    }
}

fn remove_quotes(s: &str) -> String {
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

pub fn diff_env(main: &EnvFile, linked: &EnvFile) -> Vec<EnvDiff> {
    let main_map = main.to_map();
    let linked_map = linked.to_map();
    let mut diffs = Vec::new();

    // Keys in main but not in linked (Added)
    for (key, main_val) in &main_map {
        match linked_map.get(key) {
            Some(linked_val) => {
                if main_val != linked_val {
                    diffs.push(EnvDiff {
                        key: key.clone(),
                        kind: DiffKind::Changed,
                        old_value: Some(linked_val.clone()),
                        new_value: Some(main_val.clone()),
                    });
                }
            }
            None => {
                diffs.push(EnvDiff {
                    key: key.clone(),
                    kind: DiffKind::Added,
                    old_value: None,
                    new_value: Some(main_val.clone()),
                });
            }
        }
    }

    // Keys in linked but not in main (Removed)
    for (key, linked_val) in &linked_map {
        if !main_map.contains_key(key) {
            diffs.push(EnvDiff {
                key: key.clone(),
                kind: DiffKind::Removed,
                old_value: Some(linked_val.clone()),
                new_value: None,
            });
        }
    }

    diffs.sort_by(|a, b| a.key.cmp(&b.key));
    diffs
}

pub fn merge_env(main: &EnvFile, linked: &EnvFile) -> Result<String> {
    let mut result_lines: Vec<String> = Vec::new();
    let linked_map = linked.to_map();
    let mut seen_keys = std::collections::HashSet::new();

    // Walk through main's lines, replacing values from linked where they differ
    for line in &main.lines {
        let trimmed = line.trim();

        // Pass through comments and blanks
        if trimmed.is_empty() || trimmed.starts_with('#') {
            result_lines.push(line.clone());
            continue;
        }

        let key_value = if let Some(stripped) = trimmed.strip_prefix("export ") {
            stripped
        } else {
            trimmed
        };

        if let Some(eq_pos) = key_value.find('=') {
            let key = key_value[..eq_pos].trim().to_string();
            let prefix = if trimmed.starts_with("export ") {
                "export "
            } else {
                ""
            };
            seen_keys.insert(key.clone());

            if let Some(linked_val) = linked_map.get(&key) {
                let value_str = format_value(linked_val);
                result_lines.push(format!("{}{}={}", prefix, key, value_str));
            } else {
                result_lines.push(line.clone());
            }
        } else {
            result_lines.push(line.clone());
        }
    }

    // Append keys from linked that aren't in main
    for entry in &linked.entries {
        if !seen_keys.contains(&entry.key) {
            let value_str = format_value(&entry.value);
            result_lines.push(format!("{}={}", entry.key, value_str));
        }
    }

    let mut output = result_lines.join("\n");
    if !output.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

fn format_value(val: &str) -> String {
    if val.contains(' ') || val.contains('#') || val.contains('"') {
        format!("\"{}\"", val.replace('"', "\\\""))
    } else {
        val.to_string()
    }
}
