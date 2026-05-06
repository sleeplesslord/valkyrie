# Valkyrie Architecture

A tmux sidebar TUI that tracks coding agent status in real-time.

## Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         tmux session                            │
│  ┌──────────────┐  ┌─────────────────────────────────────────┐ │
│  │   Sidebar    │  │          Main Work Area                 │ │
│  │   (TUI)      │  │                                         │ │
│  │              │  │   [Agent panes run here]                │ │
│  │  ● agent1    │  │                                         │ │
│  │  ○ agent2    │  │                                         │ │
│  │              │  │                                         │ │
│  └──────────────┘  └─────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Technology Stack

| Component | Technology | Rationale |
|-----------|------------|-----------|
| TUI Framework | ratatui + crossterm | Standard Rust TUI stack, fast, single binary |
| Async Runtime | tokio | Event-driven polling, filesystem watching |
| File Watching | notify | Inotify-based signal file monitoring |
| Serialization | serde + serde_json | Signal file parsing, state persistence |
| CLI | clap | Argument parsing, help generation |
| Date/Time | chrono | Timestamp handling |

## Project Structure

```
valkyrie/
├── Cargo.toml
├── src/
│   ├── main.rs            # CLI entry point, tmux setup, event loop, key handling
│   ├── app.rs             # Application state machine, tick loop, signal updates
│   ├── ui.rs              # ratatui rendering components
│   ├── event.rs           # Input/tick/resize event stream
│   ├── tmux.rs            # tmux CLI wrapper
│   ├── signal.rs          # Signal file watcher (notify/inotify)
│   ├── git.rs             # git diff parsing
│   ├── state.rs           # Config persistence (JSON), trim path migration
│   ├── worktree.rs        # WorktreeCache: multi-root git worktree discovery
│   ├── plugin.rs          # Plugin install/uninstall logic
│   └── agent/
│       ├── mod.rs
│       ├── model.rs       # Core data structures (Agent, AgentType, AgentStatus)
│       ├── registry.rs    # Extensible agent type system (trait + default registry)
│       └── claude.rs      # Claude Code detector (pane command/title matching)
├── plugin/
│   └── index.js           # opencode plugin: event/tool hooks → signal file writes
└── docs/
    ├── architecture.md
    ├── data-model.md
    ├── tmux-integration.md
    └── signal-protocol.md
```

## Core Components

### 1. App State (`app.rs`)

Central state machine managing:
- List of discovered agents
- Current selection (pane ID, not Vec index — decoupled from display ordering)
- UI mode (Normal, Rename, Help, DiffView)
- Tick counter for refresh scheduling
- Trim paths for worktree display labels
- Worktree cache for grouping

### 2. Event Loop (`main.rs`)

Async event handling via `event.rs`:
- Keyboard input (crossterm) — dispatched to mode-specific handlers in main.rs
- Ticks (250ms) — drives pane discovery, signal refresh, git diff updates
- Resize events — triggers terminal autoresize

Filesystem events (signal file changes) flow through `SignalWatcher` and are picked up during the tick cycle via `update_signals()`, not as separate event variants.

### 3. tmux Integration (`tmux.rs`)

All tmux CLI operations:
- Spawn sidebar pane
- Discover agent panes
- Navigate to agent panes (by pane ID directly)
- Jump to worktree directories

### 4. Signal System (`signal.rs`)

Status detection via signal files:
- Primary: Watch `~/.valkyrie/agents/` for JSON status files via inotify (notify crate)
- Stale signals (>60s) are filtered for volatile fields (status, activity, tool, task, sagas, current_file, worktree)
- Identity fields (agent_type, label, last_update) intentionally skip the stale filter
- Orphaned signal files are auto-cleaned during pane discovery

### 5. Agent Registry (`agent/registry.rs`)

Extensible detector pattern:
```rust
trait AgentDetector: Send + Sync {
    fn detect(&self, pane_info: &PaneInfo) -> Option<AgentType>;
}
```

Default registry contains only `ClaudeDetector`. Opencode detection relies 100% on signal files — there is no `OpencodeDetector` in the registry (by design).

### 6. Config & State (`state.rs`)

- `Config` struct persisted to `~/.valkyrie/state.json`
- `trim_paths: Vec<String>` — path prefixes stripped from worktree display labels (replaces deprecated `worktree_root`)
- `sidebar_width: Option<u16>` — custom sidebar width override
- Backward compat: `worktree_root` deserializes via serde alias and auto-migrates to `trim_paths`

### 7. Worktree Cache (`worktree.rs`)

- `WorktreeCache` supports multiple project roots
- Refreshes via `git worktree list` for each root
- `find_worktree()` uses longest-prefix match (avoids HashMap ordering non-determinism)
- Display labels computed by `trim_display_path()`, not stored in cache

## Data Flow

```
┌─────────────────┐
│  Agent (plugin) │──write──▶ ~/.valkyrie/agents/<pane>.json
└─────────────────┘                        │
                                           │ inotify + 2s poll fallback
                                           ▼
┌─────────────────┐                  ┌─────────────────┐
│  Pane Discovery │─────agents──────▶│  SignalWatcher  │
│  (tmux list-panes, 5s)             │  (notify)       │
└─────────────────┘                  └────────┬────────┘
                                              │
                                              ▼
                                     ┌─────────────────┐
                                     │  App State      │
                                     │  (update_signals)│
                                     └────────┬────────┘
                                              │
                                              ▼
                                     ┌─────────────────┐
                                     │  UI Render      │
                                     │  (ratatui)      │
                                     └─────────────────┘
```

## Polling Cadence

| Task | Interval | Trigger |
|------|----------|---------|
| UI Render | 250ms | Tick event |
| Pane Discovery | 5s (20 ticks) | Tick event |
| Signal Files | Immediate | inotify event + 2s poll fallback |
| Git Diff Stats | 10s (40 ticks) | Tick event |
| Worktree Refresh | 50s (200 ticks) | Tick event |

## Extensibility

### Adding New Agent Types

1. Implement `AgentDetector` trait
2. Register in `AgentRegistry` via `create_default_registry()`
3. Agent panes detected via pane command/title matching

### Future Extensions

- Multiple sidebar positions (right, bottom)
- Auto-start via tmux config
- Agent-specific signal protocols
- Notification hooks (sound, visual)
- Remote agent tracking (SSH)
