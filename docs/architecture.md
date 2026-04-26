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
│   ├── main.rs            # CLI entry point, tmux setup, event loop
│   ├── app.rs             # Application state machine
│   ├── ui.rs              # ratatui rendering components
│   ├── event.rs           # Input/tick event handling
│   ├── tmux.rs            # tmux CLI wrapper
│   ├── signal.rs          # Signal file watcher
│   ├── git.rs             # git diff parsing
│   └── agent/
│       ├── mod.rs
│       ├── model.rs       # Core data structures
│       ├── registry.rs    # Extensible agent type system
│       └── opencode.rs    # opencode-specific detection
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
- Current selection index
- UI mode (Normal, Rename, Help)
- Tick counter for refresh scheduling

### 2. Event Loop (`event.rs`)

Async event handling:
- Keyboard input (crossterm)
- Ticks (250ms render, variable polling)
- Filesystem events (notify)

### 3. tmux Integration (`tmux.rs`)

All tmux CLI operations:
- Spawn sidebar pane
- Discover agent panes
- Navigate to agent panes
- Rename windows/panes

### 4. Signal System (`signal.rs`)

Hybrid status detection:
- Primary: Watch `~/.valkyrie/agents/` for status files
- Fallback: Poll pane content via `tmux capture-pane`

### 5. Agent Registry (`agent/registry.rs`)

Extensible detector pattern:
```rust
trait AgentDetector {
    fn detect(&self, pane_info: &PaneInfo) -> Option<AgentType>;
    fn parse_status(&self, pane_content: &str) -> AgentStatus;
}
```

## Data Flow

```
┌─────────────────┐
│  tmux panes     │
│  (agents)       │
└────────┬────────┘
         │ tmux list-panes (5s)
         ▼
┌─────────────────┐     ┌─────────────────┐
│  Pane Discovery │────▶│  Agent Registry │
└────────┬────────┘     └─────────────────┘
         │
         ▼
┌─────────────────┐
│  Agent State    │◀───────┐
└────────┬────────┘        │
         │                 │
         ▼                 │
┌─────────────────┐        │
│  Signal Watcher │────────┘
│  (notify)       │   status updates
└─────────────────┘
         │
         ▼
┌─────────────────┐
│  App State      │
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
| Pane Discovery | 5s | Tick event |
| Signal Files | Immediate | inotify event + 2s fallback |
| Pane Content (fallback) | 3s | Per-agent tick |
| Git Diff Stats | 10s | Per-agent tick |

## Extensibility

### Adding New Agent Types

1. Implement `AgentDetector` trait
2. Register in `AgentRegistry`
3. Agent panes auto-detected via:
   - Process name matching
   - Pane title patterns
   - Working directory heuristics

### Future Extensions

- Multiple sidebar positions (right, bottom)
- Auto-start via tmux config
- Agent-specific signal protocols
- Notification hooks (sound, visual)
- Remote agent tracking (SSH)
