use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

const STATE_FILE: &str = ".valkyrie/state.json";
const CONFIG_FILE: &str = ".valkyrie/config.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentState {
    pub name: String,
    pub last_seen: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppState {
    pub agents: HashMap<String, AgentState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub worktree_root: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = Self::config_path();

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)?;
        let config: Config = serde_json::from_str(&content).unwrap_or_default();
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

    pub fn worktree_root(&self) -> Option<PathBuf> {
        self.worktree_root.as_ref().map(PathBuf::from)
    }

    fn config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(CONFIG_FILE)
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
