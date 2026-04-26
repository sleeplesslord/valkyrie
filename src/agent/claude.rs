use crate::agent::model::{AgentType};
use crate::tmux::PaneInfo;
use crate::agent::registry::AgentDetector;

pub struct ClaudeDetector;

impl AgentDetector for ClaudeDetector {
    fn detect(&self, pane: &PaneInfo) -> Option<AgentType> {
        let cmd = pane.current_command.to_lowercase();
        let title = pane.pane_title.to_lowercase();

        if cmd.contains("claude") || title.contains("claude") {
            Some(AgentType::ClaudeCode)
        } else {
            None
        }
    }
}
