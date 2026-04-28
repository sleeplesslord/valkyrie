use crate::agent::model::AgentType;
use crate::agent::registry::AgentDetector;
use crate::tmux::PaneInfo;

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

pub struct OpencodeDetector;

impl AgentDetector for OpencodeDetector {
    fn detect(&self, pane: &PaneInfo) -> Option<AgentType> {
        let cmd = pane.current_command.to_lowercase();
        let title = pane.pane_title.to_lowercase();

        // opencode runs as a child of the shell (bun/node), so the pane
        // command is typically "zsh" or "bash". The pane title is set by
        // opencode's TUI to include "opencode".
        if cmd.contains("opencode") || title.contains("opencode") {
            Some(AgentType::Opencode)
        } else {
            None
        }
    }
}
