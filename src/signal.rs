use crate::agent::AgentStatus;
use anyhow::Result;
use chrono::{DateTime, Utc};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

const SIGNAL_DIR: &str = ".valkyrie/agents";
const STALE_THRESHOLD_SECS: i64 = 60;

#[derive(Debug, Clone, Deserialize)]
pub struct SagaInfo {
    #[allow(dead_code)]
    pub id: String,
    pub title: String,
    pub status: String,
    pub claimed_by: Option<String>,
    /// Last sg subcommand used on this saga (context, claim, log, new, done, etc.)
    pub interaction: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SignalFile {
    pub version: Option<i32>,
    pub agent_type: Option<String>,
    pub status: Option<String>,
    pub task: Option<String>,
    pub activity: Option<String>,
    pub tool_executing: Option<String>,
    pub label: Option<String>,
    pub working_dir: Option<String>,
    pub worktree: Option<String>,
    pub current_file: Option<String>,
    pub last_update: Option<String>,
    pub sagas: Option<Vec<SagaInfo>>,
    pub metadata: Option<serde_json::Value>,
}

impl SignalFile {
    pub fn parse(content: &str) -> Result<Self> {
        let signal: SignalFile = serde_json::from_str(content)?;
        Ok(signal)
    }

    pub fn to_status(&self) -> AgentStatus {
        match self.status.as_deref() {
            Some("running") => AgentStatus::Running,
            Some("idle") => AgentStatus::Idle,
            Some("waiting_input") => AgentStatus::WaitingInput,
            Some("error") => AgentStatus::Error,
            Some("completed") => AgentStatus::Idle,
            _ => AgentStatus::Unknown,
        }
    }

    pub fn is_stale(&self) -> bool {
        if let Some(last_update) = &self.last_update {
            if let Ok(ts) = DateTime::parse_from_rfc3339(last_update) {
                let age = Utc::now().signed_duration_since(ts.with_timezone(&Utc));
                return age.num_seconds() > STALE_THRESHOLD_SECS;
            }
        }
        true
    }
}

pub struct SignalWatcher {
    signal_dir: PathBuf,
    _watcher: RecommendedWatcher,
    event_rx: Receiver<Result<Event, notify::Error>>,
    signals: HashMap<String, SignalFile>,
}

impl SignalWatcher {
    pub fn new() -> Result<Self> {
        let signal_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(SIGNAL_DIR);

        std::fs::create_dir_all(&signal_dir)?;

        let (tx, rx) = channel();
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            notify::Config::default().with_poll_interval(Duration::from_secs(2)),
        )?;

        watcher.watch(&signal_dir, RecursiveMode::NonRecursive)?;

        let mut watcher = Self {
            signal_dir,
            _watcher: watcher,
            event_rx: rx,
            signals: HashMap::new(),
        };

        watcher.load_existing_signals()?;
        Ok(watcher)
    }

    fn load_existing_signals(&mut self) -> Result<()> {
        if !self.signal_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&self.signal_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Some(pane_id) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(signal) = SignalFile::parse(&content) {
                            self.signals.insert(pane_id.to_string(), signal);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn poll(&mut self) -> Vec<String> {
        let mut changed = Vec::new();

        while let Ok(result) = self.event_rx.try_recv() {
            if let Ok(event) = result {
                if let Some(path) = event.paths.first() {
                    if let Some(pane_id) = path.file_stem().and_then(|s| s.to_str()) {
                        match event.kind {
                            EventKind::Create(_) | EventKind::Modify(_) => {
                                if let Ok(content) = std::fs::read_to_string(path) {
                                    if let Ok(signal) = SignalFile::parse(&content) {
                                        self.signals.insert(pane_id.to_string(), signal);
                                        changed.push(pane_id.to_string());
                                    }
                                }
                            }
                            EventKind::Remove(_) => {
                                self.signals.remove(pane_id);
                                changed.push(pane_id.to_string());
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        changed
    }

    pub fn get_status(&self, pane_id: &str) -> AgentStatus {
        self.signals
            .get(pane_id)
            .filter(|s| !s.is_stale())
            .map(|s| s.to_status())
            .unwrap_or(AgentStatus::Unknown)
    }

    pub fn get_agent_type(&self, pane_id: &str) -> Option<String> {
        self.signals
            .get(pane_id)
            .filter(|s| !s.is_stale())
            .and_then(|s| s.agent_type.clone())
    }

    pub fn get_task(&self, pane_id: &str) -> Option<String> {
        self.signals
            .get(pane_id)
            .filter(|s| !s.is_stale())
            .and_then(|s| s.task.clone())
    }

    pub fn get_worktree(&self, pane_id: &str) -> Option<String> {
        self.signals
            .get(pane_id)
            .filter(|s| !s.is_stale())
            .and_then(|s| s.worktree.clone())
    }

    #[allow(dead_code)]
    pub fn get_working_dir(&self, pane_id: &str) -> Option<String> {
        self.signals
            .get(pane_id)
            .filter(|s| !s.is_stale())
            .and_then(|s| s.working_dir.clone())
    }

    pub fn get_activity(&self, pane_id: &str) -> Option<String> {
        self.signals
            .get(pane_id)
            .filter(|s| !s.is_stale())
            .and_then(|s| s.activity.clone())
    }

    pub fn get_tool_executing(&self, pane_id: &str) -> Option<String> {
        self.signals
            .get(pane_id)
            .filter(|s| !s.is_stale())
            .and_then(|s| s.tool_executing.clone())
    }

    pub fn get_label(&self, pane_id: &str) -> Option<String> {
        self.signals
            .get(pane_id)
            .filter(|s| !s.is_stale())
            .and_then(|s| s.label.clone())
    }

    pub fn get_sagas(&self, pane_id: &str) -> Vec<SagaInfo> {
        self.signals
            .get(pane_id)
            .filter(|s| !s.is_stale())
            .and_then(|s| s.sagas.clone())
            .unwrap_or_default()
    }

    pub fn get_current_file(&self, pane_id: &str) -> Option<String> {
        self.signals
            .get(pane_id)
            .filter(|s| !s.is_stale())
            .and_then(|s| s.current_file.clone())
    }

    /// Returns the parsed `last_update` timestamp from the signal file.
    /// Intentionally does NOT filter stale signals — we need the timestamp
    /// even for stale agents so the UI can show correct "time since last activity".
    pub fn get_last_update(&self, pane_id: &str) -> Option<DateTime<Utc>> {
        self.signals
            .get(pane_id)
            .and_then(|s| s.last_update.as_deref())
            .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
            .map(|dt| dt.with_timezone(&Utc))
    }
}

impl Default for SignalWatcher {
    fn default() -> Self {
        Self::new().expect("Failed to create signal watcher")
    }
}
