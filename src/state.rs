use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

const STATE_FILE: &str = ".valkyrie/state.json";
const CONFIG_FILE: &str = ".valkyrie/config.json";
pub const DEFAULT_SIDEBAR_WIDTH: u16 = 50;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentState {
    pub name: String,
    pub last_seen: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppState {
    pub agents: HashMap<String, AgentState>,
}

/// Paths to strip from worktree display labels.
///
/// When an agent's worktree is `/home/user/project/.worktrees/feature-auth`
/// and `trim_paths` contains `/home/user/project`, the UI shows
/// `◆ .worktrees/feature-auth` instead of the full absolute path.
///
/// If no trim path matches, the full absolute path is shown.
/// If `trim_paths` is empty, no trimming is applied.
///
/// Backward compat: the old `worktree_root` (single string) field is
/// migrated to `trim_paths[0]` on load.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Deprecated: single worktree root. Migrated to `trim_paths` on load.
    #[serde(default)]
    worktree_root: Option<String>,

    /// Paths to strip from worktree display labels for readability.
    /// Multiple project roots can be configured.
    #[serde(default)]
    pub trim_paths: Vec<String>,

    #[serde(default)]
    pub sidebar_width: Option<u16>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = Self::config_path();

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)?;
        let mut config: Config = serde_json::from_str(&content).unwrap_or_default();

        // Migrate deprecated worktree_root → trim_paths
        if let Some(root) = config.worktree_root.take() {
            if !root.is_empty() && !config.trim_paths.contains(&root) {
                config.trim_paths.push(root);
            }
            // Persist the migration so we don't re-migrate every load
            let _ = config.save();
        }

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Return trim paths as absolute PathBufs for prefix stripping.
    pub fn trim_paths(&self) -> Vec<PathBuf> {
        self.trim_paths.iter().map(PathBuf::from).collect()
    }

    pub fn sidebar_width(&self) -> u16 {
        self.sidebar_width
            .filter(|width| *width > 0)
            .unwrap_or(DEFAULT_SIDEBAR_WIDTH)
    }

    fn config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(CONFIG_FILE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidebar_width_defaults_to_constant() {
        let config = Config::default();
        assert_eq!(config.sidebar_width(), DEFAULT_SIDEBAR_WIDTH);
    }

    #[test]
    fn sidebar_width_uses_configured_value() {
        let config = Config {
            worktree_root: None,
            trim_paths: Vec::new(),
            sidebar_width: Some(72),
        };
        assert_eq!(config.sidebar_width(), 72);
    }

    #[test]
    fn deserialize_legacy_config_without_sidebar_width() {
        let config: Config =
            serde_json::from_str(r#"{"worktree_root":"/tmp/project"}"#).unwrap();
        assert_eq!(config.sidebar_width(), DEFAULT_SIDEBAR_WIDTH);
        // worktree_root is None after deserialization (it's an alias, not preserved)
    }

    #[test]
    fn migrate_worktree_root_to_trim_paths() {
        let json = r#"{"worktree_root":"/tmp/project","sidebar_width":50}"#;
        let mut config: Config = serde_json::from_str(json).unwrap();
        // After deserialization, worktree_root is populated from the JSON
        // but we need to simulate migration. In load() it happens automatically.
        // Test the migration logic directly:
        if let Some(root) = config.worktree_root.take() {
            if !root.is_empty() && !config.trim_paths.contains(&root) {
                config.trim_paths.push(root);
            }
        }
        assert_eq!(config.trim_paths, vec!["/tmp/project"]);
        assert!(config.worktree_root.is_none());
    }

    #[test]
    fn trim_paths_returns_pathbufs() {
        let config = Config {
            worktree_root: None,
            trim_paths: vec!["/home/user/proj-a".to_string(), "/home/user/proj-b".to_string()],
            sidebar_width: None,
        };
        let paths = config.trim_paths();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], PathBuf::from("/home/user/proj-a"));
        assert_eq!(paths[1], PathBuf::from("/home/user/proj-b"));
    }
}

impl AppState {
    pub fn load() -> Result<Self> {
        let path = Self::state_path();

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)?;
        let state: AppState = serde_json::from_str(&content).unwrap_or_default();
        Ok(state)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::state_path();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn get_name(&self, pane_id: &str) -> Option<&str> {
        self.agents.get(pane_id).map(|s| s.name.as_str())
    }

    pub fn set_name(&mut self, pane_id: &str, name: &str) {
        let entry = self.agents.entry(pane_id.to_string()).or_default();
        entry.name = name.to_string();
        entry.last_seen = Some(chrono::Utc::now().to_rfc3339());
    }

    fn state_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(STATE_FILE)
    }
}
