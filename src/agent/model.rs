use crate::git::DiffStats;
use crate::tmux::PaneInfo;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentType {
    Opencode,
    ClaudeCode,
    Aider,
    Generic,
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentType::Opencode => write!(f, "opencode"),
            AgentType::ClaudeCode => write!(f, "claude-code"),
            AgentType::Aider => write!(f, "aider"),
            AgentType::Generic => write!(f, "generic"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Running,
    Idle,
    WaitingInput,
    Error,
    Offline,
    Unknown,
}

impl AgentStatus {
    pub fn indicator(&self) -> &'static str {
        match self {
            AgentStatus::Running => "●",
            AgentStatus::Idle => "○",
            AgentStatus::WaitingInput => "◐",
            AgentStatus::Error => "⚠",
            AgentStatus::Offline => "✕",
            AgentStatus::Unknown => "?",
        }
    }
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Running => write!(f, "running"),
            AgentStatus::Idle => write!(f, "idle"),
            AgentStatus::WaitingInput => write!(f, "waiting_input"),
            AgentStatus::Error => write!(f, "error"),
            AgentStatus::Offline => write!(f, "offline"),
            AgentStatus::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub pane_id: String,
    pub window_id: String,
    pub session_name: String,
    pub agent_type: AgentType,
    pub status: AgentStatus,
    pub task_description: Option<String>,
    pub working_dir: String,
    pub last_activity: DateTime<Utc>,
    pub diff_stats: Option<DiffStats>,
    pub worktree: Option<String>,
}

impl Agent {
    pub fn from_pane(pane: &PaneInfo, agent_type: AgentType) -> Self {
        let name = if pane.pane_title.is_empty() || pane.pane_title == pane.current_command {
            pane.current_command.clone()
        } else {
            pane.pane_title.clone()
        };

        Self {
            id: pane.pane_id.clone(),
            name,
            pane_id: pane.pane_id.clone(),
            window_id: pane.window_id.clone(),
            session_name: pane.session_name.clone(),
            agent_type,
            status: AgentStatus::Unknown,
            task_description: None,
            working_dir: pane.current_path.clone(),
            last_activity: Utc::now(),
            diff_stats: None,
            worktree: None,
        }
    }
}
