# valkyrie opencode Plugin

This plugin integrates opencode with the valkyrie TUI, providing real-time status updates.

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

The plugin listens for opencode events and writes status updates to signal files:

| Event | Signal Status |
|-------|---------------|
| `session.status { type: "busy" }` | `running` |
| `session.status { type: "idle" }` | `idle` |
| `session.idle` | `idle` |
| `session.error` | `error` |
| `permission.updated` | `waiting_input` |
| Process exit | Signal file deleted |

Signal files are written to `~/.valkyrie/agents/<pane-id>.json` and are automatically picked up by the valkyrie TUI.

## Requirements

- opencode running in tmux
- The `TMUX_PANE` environment variable must be set (automatic when in tmux)
