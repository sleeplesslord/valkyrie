# valkyrie opencode Plugin

This plugin integrates opencode with the valkyrie TUI, providing real-time status updates and saga tracking.

## Installation

```bash
valkyrie install
```

Restart opencode after installation.

## Uninstallation

```bash
valkyrie uninstall
```

## Check Status

```bash
valkyrie status
```

## How It Works

The plugin listens for opencode events and tool hooks, writing status updates to JSON signal files.

### Event Hooks

| Event | Signal Update |
|-------|---------------|
| `session.status { type: "busy" }` | Sets status to `running`, captures session ID and label |
| `session.idle` | Sets status to `idle` |
| `session.error` | Sets status to `error` |
| `permission.asked` | Flags permission as pending |
| `permission.updated` | Clears pending flag, sets `waiting_input` if still pending |
| `file.edited` | Updates `current_file` in signal |
| `command.executed` | Catches `sg` commands routed outside the bash tool (slash commands, first-class tool routing) |

### Tool Hooks

| Hook | Purpose |
|------|---------|
| `tool.execute.before` | Captures bash commands before execution; detects saga IDs via regex |
| `tool.execute.after` | Parses bash output for saga JSON (avoids redundant `sg context` round-trips); clears pending permission flag |

### Saga Tracking

The plugin monitors bash commands and `command.executed` events for `sg` subcommands, tracking saga interactions in real time:

- **Detection**: Three capture paths — bash before-hook, `command.executed` event, bash after-hook output parsing
- **Tracked subcommands**: `claim`, `context`, `log`, `new`, `done`, `edit`, `relate`, `depend`, `unclaim`, `continue`, `reopen`, `wontdo`
- **Interaction field**: Each saga entry stores the last `sg` subcommand used, rendered as an icon in the TUI
- **Refresh**: Saga metadata (title, status, claimed_by) is refreshed via `sg context <id> --format json`
- **Log messages**: `sg log` messages are captured and written as the `last_log` field

### Pane ID Recovery

The plugin auto-detects when `$TMUX_PANE` becomes stale (pane destroyed and recreated, or tmux server restart) via `syncPaneId()`, called every heartbeat:

1. **Fast path**: verifies stored pane ID still exists in tmux
2. **Slow path**: walks the process tree to find a real TTY, then matches against `tmux list-panes`
3. On desync: deletes old signal file, updates pane ID, writes to new path

### Signal File Location

Signal files are written to `~/.valkyrie/agents/<pane-id>.json` and are automatically picked up by the valkyrie TUI via inotify.

## Debugging

Debug logs are written to `~/.valkyrie/plugin.log`. The `debug()` helper logs fire-and-forget (non-blocking):

```bash
# Check if plugin loaded
node --check ~/.config/opencode/plugins/valkyrie/index.js

# View recent debug output
tail -20 ~/.valkyrie/plugin.log

# Verify signal files
ls ~/.valkyrie/agents/
cat ~/.valkyrie/agents/*.json | jq .
```

## Requirements

- opencode running in tmux
- The `TMUX_PANE` environment variable must be set (automatic when in tmux)
