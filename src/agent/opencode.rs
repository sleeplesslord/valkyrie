use crate::agent::model::{AgentStatus, AgentType};
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

    fn parse_status(&self, pane_content: &str) -> AgentStatus {
        let content = pane_content.to_lowercase();
        
        if content.contains("waiting for input") 
            || content.contains("press enter")
            || content.contains("continue?")
        {
            return AgentStatus::WaitingInput;
        }
        
        if content.contains("error") || content.contains("failed") {
            return AgentStatus::Error;
        }
        
        if content.contains("thinking") 
            || content.contains("processing")
            || content.contains("working on")
        {
            return AgentStatus::Running;
        }
        
        if content.contains("idle") || content.contains("ready") {
            return AgentStatus::Idle;
        }
        
        AgentStatus::Unknown
    }
}
