# Data Model

## Core Types

### Agent

Primary entity representing a discovered coding agent.

```rust
pub struct Agent {
    pub id: String,
    pub name: String,
    pub pane_id: String,
    pub window_id: String,
    pub session_name: String,
    pub agent_type: AgentType,
    pub status: AgentStatus,
    pub task_description: Option<String>,
    pub working_dir: PathBuf,
    pub last_activity: DateTime<Utc>,
    pub diff_stats: Option<DiffStats>,
    pub source: StatusSource,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | String | Unique identifier (derived from pane_id) |
| `name` | String | User-renamable display name |
| `pane_id` | String | tmux pane ID (e.g., `%0`) |
| `window_id` | String | tmux window ID (e.g., `@0`) |
| `session_name` | String | tmux session name |
| `agent_type` | AgentType | Type of coding agent |
| `status` | AgentStatus | Current activity status |
| `task_description` | Option<String> | What the agent is working on |
| `working_dir` | PathBuf | Current working directory |
| `last_activity` | DateTime | Timestamp of last status change |
| `diff_stats` | Option<DiffStats> | Git diff statistics |
| `source` | StatusSource | Where status came from |

### AgentType

```rust
pub enum AgentType {
    Opencode,
    ClaudeCode,
    Aider,
    Generic,
}
```

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
| Running | `●` | Agent is actively processing |
| Idle | `○` | Agent is waiting for task |
| WaitingInput | `◐` | Agent needs user input |
| Error | `⚠` | Agent encountered an error |
| Offline | `✕` | Pane no longer exists |
| Unknown | `?` | Cannot determine status |

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
    pub current_path: PathBuf,
    pub is_active: bool,
}
```

### StatusSource

```rust
pub enum StatusSource {
    SignalFile,
    PaneContent,
    ProcessState,
    Unknown,
}
```

## App State

```rust
pub struct App {
    pub agents: Vec<Agent>,
    pub selection: usize,
    pub mode: Mode,
    pub input_buffer: String,
    pub last_refresh: DateTime<Utc>,
    pub signal_dir: PathBuf,
    pub tick_count: u64,
}

pub enum Mode {
    Normal,
    Rename { agent_id: String },
    Help,
    DiffView { agent_id: String },
}
```

## Persistence

### Local State File

Location: `~/.valkyrie/state.json`

```json
{
  "agents": {
    "%0": {
      "name": "auth-service",
      "last_seen": "2026-04-15T18:30:00Z"
    },
    "%1": {
      "name": "api-gateway",
      "last_seen": "2026-04-15T17:45:00Z"
    }
  }
}
```

Used for:
- Persisting user-renamed agent names
- Remembering agents across sidebar restarts
- Showing recently offline agents

### Signal Files

Location: `~/.valkyrie/agents/<pane-id>.json`

See [signal-protocol.md](./signal-protocol.md) for full specification.
