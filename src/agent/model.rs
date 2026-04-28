use crate::git::DiffStats;
use crate::signal::SagaInfo;
use crate::tmux::PaneInfo;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentType {
    Opencode,
    ClaudeCode,
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentType::Opencode => write!(f, "opencode"),
            AgentType::ClaudeCode => write!(f, "claude-code"),
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

const WAVE_BLOCKS: &[&str] = &["░", "▒", "▓", "█"];

/// 3-char wave pulser: each column peaks at a different tick offset.
/// Produces patterns like ░░▓ ░▓▒ ▓▒░ █▓▓ ▓█▒ … creating a rolling wave.
fn wave_frame(tick: u64) -> String {
    let t = tick as usize;
    let n = WAVE_BLOCKS.len();
    let mut out = String::with_capacity(6);
    for col in 0..3 {
        // Each column is offset by 1 tick, stepped every 2 ticks for a smooth wave
        let phase = (t / 2 + col) % (n * 2);
        let idx = if phase < n { phase } else { n * 2 - 1 - phase };
        out.push_str(WAVE_BLOCKS[idx.min(n - 1)]);
    }
    out
}

impl AgentStatus {
    pub fn indicator(&self, activity: Option<&str>, tool: Option<&str>, tick_count: u64) -> String {
        let wave = wave_frame(tick_count);
        match self {
            AgentStatus::Running => match (activity, tool) {
                (Some("coding"), Some(t)) => format!("{} ✎ {}", wave, truncate_tool(t)),
                (Some("exploring"), Some(t)) => format!("{} ◉ {}", wave, truncate_tool(t)),
                (Some("running"), Some(t)) => format!("{} ⟳ {}", wave, truncate_tool(t)),
                (Some("researching"), Some(_)) => format!("{} ◈ web", wave),
                (_, Some(t)) => format!("{} ◉ {}", wave, truncate_tool(t)),
                (Some("coding"), None) => format!("{} ✎", wave),
                (Some("exploring"), None) => format!("{} ◉", wave),
                (Some("running"), None) => format!("{} ⟳", wave),
                (Some("researching"), None) => format!("{} ◈", wave),
                (Some("thinking"), None) => format!("{} ◎", wave),
                _ => wave,
            },
            AgentStatus::Idle => "○".to_string(),
            AgentStatus::WaitingInput => "◐".to_string(),
            AgentStatus::Error => "⚠".to_string(),
            AgentStatus::Offline => "✕".to_string(),
            AgentStatus::Unknown => "?".to_string(),
        }
    }
}

fn truncate_tool(tool: &str) -> &str {
    match tool.len() {
        0..=6 => tool,
        _ => &tool[..6],
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
    pub _id: String,
    pub name: String,
    pub pane_id: String,
    pub window_id: String,
    pub session_name: String,
    pub agent_type: AgentType,
    pub status: AgentStatus,
    pub task_description: Option<String>,
    pub activity: Option<String>,
    pub tool_executing: Option<String>,
    pub working_dir: String,
    pub last_activity: DateTime<Utc>,
    pub diff_stats: Option<DiffStats>,
    pub worktree: Option<String>,
    pub current_file: Option<String>,
    pub sagas: Vec<SagaInfo>,
}

impl Agent {
    pub fn from_pane(pane: &PaneInfo, agent_type: AgentType) -> Self {
        let name = if pane.pane_title.is_empty() || pane.pane_title == pane.current_command {
            pane.current_command.clone()
        } else {
            pane.pane_title.clone()
        };

        Self {
            _id: pane.pane_id.clone(),
            name,
            pane_id: pane.pane_id.clone(),
            window_id: pane.window_id.clone(),
            session_name: pane.session_name.clone(),
            agent_type,
            status: AgentStatus::Unknown,
            task_description: None,
            activity: None,
            tool_executing: None,
            working_dir: pane.current_path.clone(),
            last_activity: Utc::now(),
            diff_stats: None,
            worktree: None,
            current_file: None,
            sagas: Vec::new(),
        }
    }
}
