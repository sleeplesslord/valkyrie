# Signal File Protocol

## Overview

Signal files provide a cooperative communication channel between agents and the sidebar. Agents that support this protocol write status updates to JSON files that the sidebar monitors in real-time.

## Directory Structure

```
~/.valkyrie/
├── state.json              # Sidebar state (user names, history)
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
  "task": "Implementing authentication module",
  "working_dir": "/home/user/projects/auth-service",
  "last_update": "2026-04-15T18:30:00Z",
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
| `agent_type` | string | No | Agent identifier (opencode, claude-code, etc.) |
| `status` | string | Yes | Current status (see Status Values) |
| `task` | string | No | Human-readable task description |
| `working_dir` | string | No | Current working directory |
| `last_update` | string | Yes | ISO 8601 timestamp |
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

opencode should write signal updates when:
1. Starting a task
2. Completing a task
3. Waiting for user input
4. Encountering an error
5. Entering idle state

Implementation approach:
- Add hooks in opencode's event loop
- Write to `~/.valkyrie/agents/<TMUX_PANE>.json`
- Clean up file on exit

### Hook Implementation (Pseudocode)

```python
def update_signal(status, task=None):
    pane_id = os.environ.get('TMUX_PANE', 'unknown')
        signal_path = Path.home() / '.valkyrie' / 'agents' / f'{pane_id}.json'
    
    data = {
        'status': status,
        'last_update': datetime.utcnow().isoformat() + 'Z',
    }
    
    if task:
        data['task'] = task
    
    signal_path.parent.mkdir(parents=True, exist_ok=True)
    signal_path.write_text(json.dumps(data))

# Called at appropriate points in agent lifecycle
update_signal('running', 'Refactoring auth module')
# ... work ...
update_signal('waiting_input', 'Need approval for file changes')
# ... user input ...
update_signal('completed', 'Refactoring complete')
update_signal('idle')
```

### Cleanup

Agents should remove their signal file on clean exit:

```python
def cleanup_signal():
    pane_id = os.environ.get('TMUX_PANE')
    if pane_id:
    signal_path = Path.home() / '.valkyrie' / 'agents' / f'{pane_id}.json'
        signal_path.unlink(missing_ok=True)
```

## Sidebar Implementation

### File Watching

Using `notify` crate:
1. Watch `~/.valkyrie/agents/` directory
2. On `Create` or `Modify` events: parse JSON, update agent status
3. On `Remove` event: mark agent as offline (or remove if configured)

### Fallback Behavior

When no signal file exists:
1. Use `tmux capture-pane` to get pane content
2. Parse content for status indicators
3. Use process state as last resort

### Staleness Detection

Consider signal stale if:
1. `last_update` is older than 60 seconds
2. Pane no longer exists (via `tmux has-session`)

Stale signals trigger fallback status detection.

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
