# Signal File Protocol

## Overview

Signal files provide a cooperative communication channel between agents and the sidebar. Agents that support this protocol write status updates to JSON files that the sidebar monitors in real-time via inotify.

## Directory Structure

```
~/.valkyrie/
├── state.json              # Sidebar state (user names, config, trim paths)
├── plugin.log               # Debug log from opencode plugin
├── sidebar-pane             # Tracks sidebar pane ID for toggle script
└── agents/
    ├── %0.json             # Status for pane %0
    ├── %1.json             # Status for pane %1
    └── %42.json            # Status for pane %42
```

## Signal File Format

### Full Schema

```json
{
  "version": 1,
  "agent_type": "opencode",
  "status": "running",
  "activity": "coding",
  "tool_executing": "edit",
  "label": "auth-service",
  "task": "Implementing authentication module",
  "working_dir": "/home/user/projects/auth-service",
  "worktree": ".worktrees/feature-auth",
  "current_file": "src/auth/mod.rs",
  "last_update": "2026-04-15T18:30:00Z",
  "last_log": "Implemented JWT validation",
  "sagas": [
    {"id": "abc123", "title": "Implement auth", "status": "active", "claimed_by": "agent-1", "interaction": "claim"},
    {"id": "def456", "title": "Add OAuth", "status": "active", "claimed_by": null, "interaction": "context"}
  ],
  "metadata": {
    "model": "claude-3-opus",
    "tokens_used": 12543
  }
}
```

### Minimal Schema

Only required fields:
```json
{
  "status": "running",
  "last_update": "2026-04-15T18:30:00Z"
}
```

### Field Specifications

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `version` | int | No | Protocol version (currently 1) |
| `agent_type` | string | No | Agent identifier (`opencode`, `claude-code`) |
| `status` | string | Yes | Current status (see Status Values) |
| `activity` | string | No | Activity category (`coding`, `exploring`, `running`, `researching`, `thinking`) |
| `tool_executing` | string | No | Name of tool currently executing |
| `label` | string | No | Session display name (used as agent name in sidebar) |
| `task` | string | No | Human-readable task description |
| `working_dir` | string | No | Current working directory |
| `worktree` | string | No | Worktree path (relative or absolute, used for grouping and display) |
| `current_file` | string | No | File currently being edited |
| `last_update` | string | Yes | ISO 8601 timestamp |
| `last_log` | string | No | Last `sg log` message from the agent |
| `sagas` | array | No | List of saga objects (see Saga Objects) |
| `metadata` | object | No | Agent-specific additional data |

### Status Values

| Value | Description |
|-------|-------------|
| `running` | Actively processing |
| `idle` | Waiting for task |
| `waiting_input` | Needs user input |
| `error` | Encountered an error |
| `completed` | Task finished |

### Saga Objects

Each saga in the `sagas` array has:

```json
{
  "id": "abc123",
  "title": "Implement auth",
  "status": "active",
  "claimed_by": "agent-1",
  "interaction": "context"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Saga identifier |
| `title` | string | No | Human-readable saga title |
| `status` | string | No | Saga status (active, paused, done, wontdo) |
| `claimed_by` | string \| null | No | Agent that claimed the saga |
| `interaction` | string \| null | No | Last sg subcommand used (see Interaction Values) |

### Interaction Values

The `interaction` field tracks what the agent last did with a saga, enabling the sidebar to display meaningful icons that distinguish "referenced as context" from "claimed for work" from "logged progress":

| Value | Icon | Color | Description |
|-------|------|-------|-------------|
| `context` | ◫ | cyan | Read full context (reference lookup) |
| `claim` | ◐ | yellow | Claimed for work |
| `log` | ✎ | gray | Added a log entry |
| `new` | ✦ | green | Created the saga |
| `done` | ✓ | dark gray | Marked complete |
| `edit` | ✎ | magenta | Edited title/description/priority |
| `relate` | ◈ | cyan | Added a relationship link |
| `depend` | ◈ | cyan | Added a dependency |
| `unclaim` | ○ | yellow | Released claim |
| `continue` | ▶ | green | Resumed a paused saga |
| `reopen` | ↻ | yellow | Reopened a completed saga |
| `wontdo` | ⊘ | dark gray | Marked won't-do |

## Agent Integration

### For opencode

The opencode plugin (`plugin/index.js`) writes signal updates via event and tool hooks:

**Event hooks** — `event({ event })` handles:
1. `session.status` — busy/idle transitions, session ID and label capture
2. `session.idle` / `session.error` — status changes
3. `permission.asked` / `permission.updated` — waiting_input state
4. `file.edited` — current file tracking
5. `command.executed` — catches `sg` commands routed outside bash (slash commands, first-class tools)

**Tool hooks**:
- `tool.execute.before(input, output)` — captures bash commands, detects saga IDs via regex
- `tool.execute.after(input, output)` — parses bash output for saga JSON, clears permission flags

**Signal file lifecycle**:
1. Plugin captures `$TMUX_PANE` at startup as the signal filename
2. Writes to `~/.valkyrie/agents/<TMUX_PANE>.json` on each status change
3. Auto-detects pane ID desync via `syncPaneId()` (called every heartbeat)
4. Deletes signal file on exit via `process.on("exit", ...)` using synchronous `unlinkSync()`

### Cleanup

Agents should remove their signal file on clean exit. The opencode plugin does this via:

```javascript
process.on("exit", () => {
  try { fs.unlinkSync(signalPath); } catch {}
});
```

Orphaned signal files (panes no longer in tmux) are also auto-cleaned by the TUI during pane discovery every 5 seconds.

## Sidebar Implementation

### File Watching

Using `notify` crate:
1. Watch `~/.valkyrie/agents/` directory via inotify
2. On `Create` or `Modify` events: parse JSON, update agent status
3. On `Remove` event: mark agent as offline
4. 2-second poll fallback in addition to inotify

### Staleness Detection

A signal is considered stale if `last_update` is older than 60 seconds. The staleness filter is applied **selectively**:

**Volatile fields (filtered by stale)**: `status`, `activity`, `tool_executing`, `task`, `sagas`, `current_file`, `worktree` — these revert to defaults for stale agents.

**Identity fields (NOT filtered by stale)**: `agent_type`, `label`, `last_update` — these persist even for stale agents. Filtering them would make opencode agents invisible or cause names to revert to "zsh" during idle periods.

## Backward Compatibility

- Unknown fields in JSON are ignored
- Missing `version` field defaults to version 1
- Missing optional fields use sensible defaults
- Malformed JSON is logged and ignored

## Future Extensions

### Version 2 (Planned)

- Support for progress indicators
- Structured task hierarchy
- Webhook notifications
- Remote agent support via network signals

### Example Future Signal

```json
{
  "version": 2,
  "status": "running",
  "task": {
    "title": "Implement auth",
    "progress": 0.65,
    "subtasks": [
      {"title": "OAuth", "complete": true},
      {"title": "Sessions", "complete": true},
      {"title": "Tokens", "complete": false}
    ]
  }
}
```
