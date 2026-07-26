use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
pub struct Config {
    #[serde(default)]
    pub envsync: EnvsyncConfig,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
pub struct EnvsyncConfig {
    #[serde(default = "default_include")]
    pub include: Vec<String>,

    #[serde(default)]
    pub ignore: Vec<String>,

    #[serde(default)]
    pub overrides: HashMap<String, OverrideConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
#[allow(dead_code)]
pub enum OverrideConfig {
    Simple(String),
    Detailed {
        base: String,
        #[serde(default)]
        increment: Option<u32>,
        #[serde(default)]
        suffix: Option<String>,
    },
}

#[allow(dead_code)]
fn default_include() -> Vec<String> {
    vec![
        ".env".to_string(),
        ".env.local".to_string(),
        ".env.*".to_string(),
    ]
}

#[allow(dead_code)]
impl Config {
    pub fn load(project_root: &Path) -> Result<Self> {
        let config_path = project_root.join(".envsync.toml");

        if !config_path.exists() {
            return Ok(Config::default());
        }

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config: {}", config_path.display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config: {}", config_path.display()))?;

        Ok(config)
    }

    pub fn should_ignore(&self, file_path: &Path) -> bool {
        let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        for pattern in &self.envsync.ignore {
            if *pattern == file_name || *pattern == file_path.to_string_lossy() {
                return true;
            }
            // Simple glob matching
            if pattern.contains('*') {
                if let Ok(glob) = glob::Pattern::new(pattern) {
                    if glob.matches(file_name) {
                        return true;
                    }
                }
            }
        }

        false
    }
}
