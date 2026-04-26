use crate::agent::model::{AgentType};
use crate::tmux::PaneInfo;
use crate::agent::registry::AgentDetector;

pub struct OpencodeDetector;

impl AgentDetector for OpencodeDetector {
    fn detect(&self, pane: &PaneInfo) -> Option<AgentType> {
        let cmd = pane.current_command.to_lowercase();
        let title = pane.pane_title.to_lowercase();
        
        if cmd.contains("opencode") || title.contains("opencode") {
            Some(AgentType::Opencode)
        } else {
            None
        }
    }
}
