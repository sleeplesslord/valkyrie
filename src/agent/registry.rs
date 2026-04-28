use crate::agent::model::AgentType;
use crate::tmux::PaneInfo;

pub trait AgentDetector: Send + Sync {
    fn detect(&self, pane: &PaneInfo) -> Option<AgentType>;
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

    pub fn detect(&self, pane: &PaneInfo) -> Option<AgentType> {
        for detector in &self.detectors {
            if let Some(agent_type) = detector.detect(pane) {
                return Some(agent_type);
            }
        }
        None
    }

    #[allow(dead_code)]
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
    registry.register(super::claude::ClaudeDetector);
    registry.register(super::claude::OpencodeDetector);
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane_with(command: &str, title: &str) -> PaneInfo {
        PaneInfo {
            session_name: "main".to_string(),
            window_id: "@1".to_string(),
            pane_id: "%1".to_string(),
            pane_title: title.to_string(),
            current_command: command.to_string(),
            current_path: "/tmp".to_string(),
            is_active: true,
        }
    }

    #[test]
    fn default_registry_detects_claude() {
        let registry = create_default_registry();
        let pane = pane_with("claude", "claude");

        assert_eq!(registry.detect(&pane), Some(AgentType::ClaudeCode));
    }

    #[test]
    fn default_registry_detects_opencode_by_title() {
        let registry = create_default_registry();
        let pane = pane_with("zsh", "opencode");

        assert_eq!(registry.detect(&pane), Some(AgentType::Opencode));
    }

    #[test]
    fn default_registry_does_not_detect_opencode_by_plain_zsh() {
        let registry = create_default_registry();
        let pane = pane_with("zsh", "zsh");

        assert_eq!(registry.detect(&pane), None);
    }
}
