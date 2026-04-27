use crate::agent::registry::create_default_registry;
use crate::agent::{Agent, AgentStatus, AgentType};
use crate::git;
use crate::signal::SignalWatcher;
use crate::state::{AppState, Config};
use crate::tmux::{PaneInfo, Tmux};
use crate::worktree::WorktreeCache;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub enum Mode {
    #[default]
    Normal,
    Rename {
        agent_id: String,
    },
    Help,
    DiffView {
        agent_id: String,
    },
}

pub struct App {
    pub agents: Vec<Agent>,
    pub selection: usize,
    pub mode: Mode,
    pub input_buffer: String,
    pub last_refresh: DateTime<Utc>,
    pub tick_count: u64,
    pub diff_scroll: usize,
    tmux: Tmux,
    sidebar_pane_id: Option<String>,
    signal_watcher: SignalWatcher,
    state: AppState,
    config: Config,
    worktree_cache: WorktreeCache,
}

const PANE_DISCOVERY_INTERVAL: u64 = 20;
const GIT_DIFF_INTERVAL: u64 = 40;
const WORKTREE_REFRESH_INTERVAL: u64 = 200;

fn parse_signal_agent_type(agent_type: &str) -> Option<AgentType> {
    match agent_type.to_ascii_lowercase().as_str() {
        "opencode" => Some(AgentType::Opencode),
        "claude-code" => Some(AgentType::ClaudeCode),
        _ => None,
    }
}

fn pane_default_name(pane: &PaneInfo) -> String {
    if pane.pane_title.is_empty() || pane.pane_title == pane.current_command {
        pane.current_command.clone()
    } else {
        pane.pane_title.clone()
    }
}

fn resolve_agent_name(
    pane: &PaneInfo,
    saved_name: Option<&str>,
    signal_label: Option<String>,
) -> String {
    if let Some(name) = saved_name.filter(|n| !n.is_empty()) {
        return name.to_string();
    }

    if let Some(label) = signal_label {
        return label;
    }

    pane_default_name(pane)
}

fn is_sidebar_pane(sidebar_pane_id: Option<&str>, pane: &PaneInfo) -> bool {
    if sidebar_pane_id != Some(pane.pane_id.as_str()) {
        return false;
    }

    let cmd = pane.current_command.to_ascii_lowercase();
    let title = pane.pane_title.to_ascii_lowercase();
    cmd.contains("valkyrie") || title.contains("valkyrie")
}

impl App {
    pub fn new() -> Self {
        let tmux = Tmux::new();
        let sidebar_pane_id = Tmux::current_pane_id();
        let signal_watcher = SignalWatcher::new().unwrap_or_else(|e| {
            eprintln!("Warning: Failed to create signal watcher: {}", e);
            SignalWatcher::new().expect("Signal watcher required")
        });
        let state = AppState::load().unwrap_or_default();
        let config = Config::load().unwrap_or_default();

        let mut worktree_cache = WorktreeCache::new();
        if let Some(root) = config.worktree_root() {
            worktree_cache.set_root(root);
        }

        let mut app = Self {
            agents: Vec::new(),
            selection: 0,
            mode: Mode::default(),
            input_buffer: String::new(),
            last_refresh: Utc::now(),
            tick_count: 0,
            diff_scroll: 0,
            tmux,
            sidebar_pane_id,
            signal_watcher,
            state,
            config,
            worktree_cache,
        };
        app.discover_panes();
        app.update_signals();
        app.apply_saved_names();
        app.update_worktrees();
        app
    }

    pub fn tick(&mut self) {
        self.tick_count += 1;
        self.last_refresh = Utc::now();

        self.signal_watcher.poll();
        self.update_signals();

        if self.tick_count % PANE_DISCOVERY_INTERVAL == 0 {
            self.discover_panes();
            self.apply_saved_names();
        }

        if self.tick_count % GIT_DIFF_INTERVAL == 0 {
            self.update_git_diffs();
        }

        if self.tick_count % WORKTREE_REFRESH_INTERVAL == 0 {
            self.worktree_cache.refresh();
            self.update_worktrees();
        }
    }

    fn update_worktrees(&mut self) {
        for agent in &mut self.agents {
            if let Some(wt) = self.worktree_cache.find_worktree(&agent.working_dir) {
                agent.worktree = Some(wt.relative.clone());
            }
        }
    }

    pub fn agents_by_worktree(&self) -> Vec<(Option<String>, Vec<&Agent>)> {
        let mut groups: HashMap<Option<String>, Vec<&Agent>> = HashMap::new();

        for agent in &self.agents {
            groups
                .entry(agent.worktree.clone())
                .or_default()
                .push(agent);
        }

        let mut result: Vec<(Option<String>, Vec<&Agent>)> = groups.into_iter().collect();

        result.sort_by(|a, b| match (&a.0, &b.0) {
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(a_name), Some(b_name)) => a_name.cmp(b_name),
            (None, None) => std::cmp::Ordering::Equal,
        });

        result
    }

    fn update_git_diffs(&mut self) {
        for agent in &mut self.agents {
            if agent.status != AgentStatus::Offline {
                agent.diff_stats = git::get_diff_stats(&agent.working_dir);
            }
        }
    }

    fn apply_saved_names(&mut self) {
        for agent in &mut self.agents {
            if let Some(saved_name) = self.state.get_name(&agent.pane_id) {
                if !saved_name.is_empty() {
                    agent.name = saved_name.to_string();
                }
            }
        }
    }

    fn update_signals(&mut self) {
        for agent in &mut self.agents {
            let status = self.signal_watcher.get_status(&agent.pane_id);
            if status != AgentStatus::Unknown {
                agent.status = status;
            }

            if let Some(task) = self.signal_watcher.get_task(&agent.pane_id) {
                agent.task_description = Some(task);
            }

            agent.activity = self.signal_watcher.get_activity(&agent.pane_id);
            agent.tool_executing = self.signal_watcher.get_tool_executing(&agent.pane_id);
            agent.sagas = self.signal_watcher.get_sagas(&agent.pane_id);

            if let Some(label) = self.signal_watcher.get_label(&agent.pane_id) {
                let has_custom_name = self
                    .state
                    .get_name(&agent.pane_id)
                    .map(|n| !n.is_empty())
                    .unwrap_or(false);
                if !has_custom_name {
                    agent.name = label;
                }
            }

            if let Some(worktree_path) = self.signal_watcher.get_worktree(&agent.pane_id) {
                agent.working_dir = worktree_path.clone();
                if let Some(root) = self.worktree_cache.root() {
                    let wt_path = std::path::PathBuf::from(&worktree_path);
                    if let Ok(relative) = wt_path.strip_prefix(root) {
                        let rel = relative.to_string_lossy().to_string();
                        agent.worktree = if rel.is_empty() { None } else { Some(rel) };
                    } else {
                        agent.worktree = Some(worktree_path);
                    }
                }
            }

            agent.last_activity = Utc::now();
        }
    }

    fn discover_panes(&mut self) {
        if let Ok(panes) = self.tmux.list_panes() {
            let registry = create_default_registry();
            let current_ids: std::collections::HashSet<String> =
                self.agents.iter().map(|a| a.pane_id.clone()).collect();

            let mut new_agents: Vec<Agent> = Vec::new();
            let mut all_pane_ids: std::collections::HashSet<String> =
                std::collections::HashSet::new();

            for pane in panes {
                if is_sidebar_pane(self.sidebar_pane_id.as_deref(), &pane) {
                    continue;
                }

                all_pane_ids.insert(pane.pane_id.clone());

                let signal_agent_type = self
                    .signal_watcher
                    .get_agent_type(&pane.pane_id)
                    .as_deref()
                    .and_then(parse_signal_agent_type);

                let detected_agent_type = signal_agent_type.or_else(|| registry.detect(&pane));

                if let Some(agent_type) = detected_agent_type {
                    let resolved_name = resolve_agent_name(
                        &pane,
                        self.state.get_name(&pane.pane_id),
                        self.signal_watcher.get_label(&pane.pane_id),
                    );

                    if !current_ids.contains(&pane.pane_id) {
                        let mut new_agent = Agent::from_pane(&pane, agent_type);
                        new_agent.name = resolved_name;
                        new_agents.push(new_agent);
                    } else if let Some(existing) =
                        self.agents.iter_mut().find(|a| a.pane_id == pane.pane_id)
                    {
                        existing.name = resolved_name;
                    }
                }
            }

            for agent in &mut self.agents {
                if !all_pane_ids.contains(&agent.pane_id) {
                    let signal_status = self.signal_watcher.get_status(&agent.pane_id);
                    if signal_status == AgentStatus::Unknown {
                        agent.status = AgentStatus::Offline;
                    }
                }
            }

            self.agents.extend(new_agents);
            self.agents
                .retain(|a| a.status != AgentStatus::Offline || current_ids.contains(&a.pane_id));

            if self.selection >= self.agents.len() && !self.agents.is_empty() {
                self.selection = self.agents.len() - 1;
            }
        }
    }

    pub fn select_next(&mut self) {
        if !self.agents.is_empty() {
            self.selection = (self.selection + 1) % self.agents.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.agents.is_empty() {
            self.selection = if self.selection == 0 {
                self.agents.len() - 1
            } else {
                self.selection - 1
            };
        }
    }

    pub fn selected_agent(&self) -> Option<&Agent> {
        self.agents.get(self.selection)
    }

    pub fn selected_agent_mut(&mut self) -> Option<&mut Agent> {
        self.agents.get_mut(self.selection)
    }

    pub fn jump_to_selected(&self) -> Result<()> {
        if let Some(agent) = self.selected_agent() {
            // Switch window first, then focus the pane — select-pane alone
            // doesn't change the active window in tmux.
            let window_target = format!("{}:{}", agent.session_name, agent.window_id);
            self.tmux.select_window(&window_target)?;
            self.tmux
                .select_pane(&agent.session_name, &agent.window_id, &agent.pane_id)?;
        }
        Ok(())
    }

    pub fn jump_to_worktree(&self) -> Result<()> {
        if let Some(agent) = self.selected_agent() {
            let cwd = agent.worktree.as_deref().unwrap_or(&agent.working_dir);
            self.tmux.new_window_cwd("worktree", cwd)?;
        }
        Ok(())
    }

    pub fn start_rename(&mut self) {
        if let Some(agent) = self.selected_agent() {
            let agent_id = agent.pane_id.clone();
            self.input_buffer = agent.name.clone();
            self.mode = Mode::Rename { agent_id };
        }
    }

    pub fn confirm_rename(&mut self) -> Result<()> {
        if let Mode::Rename { agent_id } = &self.mode {
            let new_name = self.input_buffer.trim();
            if !new_name.is_empty() {
                self.state.set_name(agent_id, new_name);
                self.state.save()?;

                if let Some(agent) = self.agents.iter_mut().find(|a| &a.pane_id == agent_id) {
                    agent.name = new_name.to_string();
                }
            }
        }
        self.mode = Mode::Normal;
        self.input_buffer.clear();
        Ok(())
    }

    pub fn cancel_rename(&mut self) {
        self.mode = Mode::Normal;
        self.input_buffer.clear();
    }

    pub fn handle_rename_input(&mut self, c: char) {
        self.input_buffer.push(c);
    }

    pub fn handle_rename_backspace(&mut self) {
        self.input_buffer.pop();
    }

    pub fn open_diff_in_window(&self) {
        if let Some(agent) = self.selected_agent() {
            if !git::is_git_repo(&agent.working_dir) {
                return;
            }

            let dir = &agent.working_dir;
            let cmd = format!(
                "LESS=RSX sh -c 'git -C \"{}\" diff HEAD 2>/dev/null || git -C \"{}\" diff'",
                dir, dir
            );

            if let Err(e) = self.tmux.run_in_window("git-diff", &cmd) {
                eprintln!("Failed to open diff window: {}", e);
            }
        }
    }

    pub fn start_diff_view(&mut self) {
        if let Some(agent) = self.selected_agent() {
            let agent_id = agent.pane_id.clone();
            let diff = git::get_diff(&agent.working_dir)
                .unwrap_or_else(|| "Not a git repository".to_string());
            self.input_buffer = diff;
            self.diff_scroll = 0;
            self.mode = Mode::DiffView { agent_id };
        }
    }

    pub fn diff_scroll_up(&mut self) {
        self.diff_scroll = self.diff_scroll.saturating_sub(1);
    }

    pub fn diff_scroll_down(&mut self) {
        self.diff_scroll += 1;
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane_with(id: &str, command: &str, title: &str) -> PaneInfo {
        PaneInfo {
            session_name: "main".to_string(),
            window_id: "@0".to_string(),
            pane_id: id.to_string(),
            pane_title: title.to_string(),
            current_command: command.to_string(),
            current_path: "/tmp".to_string(),
            is_active: false,
        }
    }

    #[test]
    fn parse_signal_agent_type_maps_supported_values() {
        assert_eq!(
            parse_signal_agent_type("opencode"),
            Some(AgentType::Opencode)
        );
        assert_eq!(
            parse_signal_agent_type("OPENCODE"),
            Some(AgentType::Opencode)
        );
        assert_eq!(
            parse_signal_agent_type("claude-code"),
            Some(AgentType::ClaudeCode)
        );
        assert_eq!(parse_signal_agent_type("unknown"), None);
    }

    #[test]
    fn sidebar_pane_requires_matching_id_and_identity() {
        let sidebar = pane_with("%9", "valkyrie", "valkyrie");
        let reused = pane_with("%9", "zsh", "opencode");

        assert!(is_sidebar_pane(Some("%9"), &sidebar));
        assert!(!is_sidebar_pane(Some("%9"), &reused));
        assert!(!is_sidebar_pane(Some("%8"), &sidebar));
    }

    #[test]
    fn resolve_agent_name_prioritizes_saved_name() {
        let pane = pane_with("%1", "zsh", "OC");

        let resolved = resolve_agent_name(&pane, Some("My Agent"), Some("Label".to_string()));

        assert_eq!(resolved, "My Agent");
    }

    #[test]
    fn resolve_agent_name_uses_signal_label_before_tmux_name() {
        let pane = pane_with("%1", "zsh", "OC");

        let resolved = resolve_agent_name(&pane, None, Some("Feature Branch".to_string()));

        assert_eq!(resolved, "Feature Branch");
    }

    #[test]
    fn resolve_agent_name_falls_back_to_tmux_name() {
        let pane = pane_with("%1", "zsh", "OC");

        let resolved = resolve_agent_name(&pane, None, None);

        assert_eq!(resolved, "OC");
    }
}
