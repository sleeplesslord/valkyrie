# Data Model

## Core Types

### Agent

Primary entity representing a discovered coding agent.

```rust
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
    pub worktree_abs: Option<String>,
    pub current_file: Option<String>,
    pub last_log: Option<String>,
    pub sagas: Vec<SagaInfo>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `_id` | String | Unique identifier (same as pane_id) |
| `name` | String | User-renamable display name |
| `pane_id` | String | tmux pane ID (e.g., `%0`) — primary key |
| `window_id` | String | tmux window ID (for display grouping only, NOT navigation) |
| `session_name` | String | tmux session name (for display grouping only) |
| `agent_type` | AgentType | Type of coding agent |
| `status` | AgentStatus | Current activity status |
| `task_description` | Option<String> | What the agent is working on |
| `activity` | Option<String> | Activity category (coding, exploring, running, researching, thinking) |
| `tool_executing` | Option<String> | Name of tool currently executing |
| `working_dir` | String | Current working directory |
| `last_activity` | DateTime | Timestamp from signal file's `last_update` |
| `diff_stats` | Option<DiffStats> | Git diff statistics |
| `worktree` | Option<String> | Display label (trimmed via `trim_display_path`) |
| `worktree_abs` | Option<String> | Absolute worktree path for navigation — set from signal at same time as display label |
| `current_file` | Option<String> | File currently being edited |
| `last_log` | Option<String> | Last `sg log` message text |
| `sagas` | Vec<SagaInfo> | Tracked sagas with interaction state |

### AgentType

```rust
pub enum AgentType {
    Opencode,
    ClaudeCode,
}
```

Note: Opencode is detected via signal files only (no `OpencodeDetector` in the registry). ClaudeCode is detected via the `ClaudeDetector` matching pane command/title.

### AgentStatus

```rust
pub enum AgentStatus {
    Running,
    Idle,
    WaitingInput,
    Error,
    Offline,
    Unknown,
}
```

| Status | Indicator | Description |
|--------|-----------|-------------|
| Running | `░▒▓ ✎` (animated wave + activity icon) | Agent is actively processing |
| Idle | `○` | Agent is waiting for task |
| WaitingInput | `◐` | Agent needs user input |
| Error | `⚠` | Agent encountered an error |
| Offline | `✕` | Pane no longer exists |
| Unknown | `?` | Cannot determine status |

Running status indicators are context-aware based on `activity` and `tool_executing`:
- coding → `✎`
- exploring → `◉`
- running → `⟳`
- researching → `◈`
- thinking → `◎`

### SagaInfo

```rust
pub struct SagaInfo {
    pub id: String,
    pub title: String,
    pub status: String,
    pub claimed_by: Option<String>,
    pub interaction: Option<String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | String | Saga identifier |
| `title` | String | Human-readable saga title |
| `status` | String | Saga status (active, paused, done, wontdo) |
| `claimed_by` | Option<String> | Agent that claimed the saga |
| `interaction` | Option<String> | Last sg subcommand (context, claim, log, new, done, edit, relate, depend, unclaim, continue, reopen, wontdo) |

### DiffStats

```rust
pub struct DiffStats {
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
    pub uncommitted: bool,
}
```

### PaneInfo

Raw tmux pane data used for agent discovery.

```rust
pub struct PaneInfo {
    pub pane_id: String,
    pub window_id: String,
    pub session_name: String,
    pub pane_title: String,
    pub current_command: String,
    pub current_path: String,
    pub is_active: bool,
}
```

### SignalFile

Deserialized from `~/.valkyrie/agents/<pane-id>.json`:

```rust
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
    pub last_log: Option<String>,
    pub metadata: Option<serde_json::Value>,
}
```

See [signal-protocol.md](./signal-protocol.md) for the full specification.

## App State

```rust
pub struct App {
    pub agents: Vec<Agent>,
    pub selection: Option<String>,  // pane_id of selected agent
    pub mode: Mode,
    pub input_buffer: String,
    pub last_refresh: DateTime<Utc>,
    pub tick_count: u64,
    pub diff_scroll: usize,
    // ... private fields: tmux, signal_watcher, state, config, worktree_cache, trim_paths, sidebar_pane_id
}

pub enum Mode {
    Normal,
    Rename { agent_id: String },
    Help,
    DiffView { _agent_id: String },
}
```

Key design: `selection` is keyed by pane_id (`Option<String>`), not Vec index. This decouples selection from display ordering (which is worktree-grouped) and prevents "jumps to wrong agent" bugs.

## Persistence

### Local State File

Location: `~/.valkyrie/state.json`

```json
{
  "trim_paths": ["/home/user/project"],
  "sidebar_width": 50,
  "names": {
    "%0": "auth-service",
    "%1": "api-gateway"
  }
}
```

Used for:
- Persisting user-renamed agent names
- Trim paths for worktree display labels (migrated from deprecated `worktree_root`)
- Custom sidebar width

### Signal Files

Location: `~/.valkyrie/agents/<pane-id>.json`

See [signal-protocol.md](./signal-protocol.md) for full specification.
