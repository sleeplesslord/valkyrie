use crate::agent::model::{AgentStatus, AgentType};
use crate::tmux::PaneInfo;

pub trait AgentDetector: Send + Sync {
    fn detect(&self, pane: &PaneInfo) -> Option<AgentType>;
    fn parse_status(&self, pane_content: &str) -> AgentStatus;
}

pub struct AgentRegistry {
    detectors: Vec<Box<dyn AgentDetector>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
        }
    }

    pub fn register<D: AgentDetector + 'static>(&mut self, detector: D) {
        self.detectors.push(Box::new(detector));
    }

    pub fn detect(&self, pane: &PaneInfo) -> Option<(AgentType, &dyn AgentDetector)> {
        for detector in &self.detectors {
            if let Some(agent_type) = detector.detect(pane) {
                return Some((agent_type, detector.as_ref()));
            }
        }
        None
    }

    pub fn is_agent_pane(&self, pane: &PaneInfo) -> bool {
        self.detectors.iter().any(|d| d.detect(pane).is_some())
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn create_default_registry() -> AgentRegistry {
    let mut registry = AgentRegistry::new();
    registry.register(super::opencode::OpencodeDetector);
    registry.register(GenericDetector);
    registry
}

struct GenericDetector;

impl AgentDetector for GenericDetector {
    fn detect(&self, pane: &PaneInfo) -> Option<AgentType> {
        let cmd = pane.current_command.to_lowercase();
        let title = pane.pane_title.to_lowercase();
        
        if cmd.contains("opencode") || title.contains("opencode") {
            return None;
        }
        if cmd.contains("claude") || title.contains("claude") {
            return Some(AgentType::ClaudeCode);
        }
        if cmd.contains("aider") || title.contains("aider") {
            return Some(AgentType::Aider);
        }
        if title.contains("agent") || cmd.contains("agent") {
            return Some(AgentType::Generic);
        }
        None
    }

    fn parse_status(&self, _pane_content: &str) -> AgentStatus {
        AgentStatus::Unknown
    }
}
