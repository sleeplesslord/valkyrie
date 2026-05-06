# valkyrie

A tmux sidebar TUI for tracking AI coding agents in real-time via signal files.

## How It Works

Valkyrie discovers agents through **signal files** — small JSON files that agents write to `~/.valkyrie/agents/<pane-id>.json`. The sidebar watches this directory via inotify and updates instantly when signal files change. Any agent that can write a JSON file to the right path will appear in the sidebar — no plugin or special integration required beyond that.

An opencode plugin is included that writes signal files automatically (status, tool activity, saga tracking, current file). Claude Code is also detected via pane command/title matching as a fallback. But the core protocol is agent-agnostic: if your agent can create a signal file, valkyrie can track it.

See [docs/signal-protocol.md](docs/signal-protocol.md) for the full file format specification.

## Features

- **Signal Protocol**: Real-time agent tracking via JSON signal files with inotify monitoring
- **Agent-Agnostic**: Any agent that writes a signal file appears in the sidebar
- **Status Tracking**: Shows running, idle, waiting for input, error, and offline states
- **Activity Indicators**: Context-aware icons for coding, exploring, running, researching, thinking
- **Saga Tracking**: Tracks agent saga interactions (claim, context, log, etc.) with status icons
- **Worktree Grouping**: Groups agents by git worktree with configurable path trimming
- **Git Diff Stats**: Shows +X/-Y diff statistics for each agent
- **Jump to Pane**: Navigate directly to agent panes from sidebar
- **Jump to Worktree**: Open a new pane in the agent's worktree directory
- **Rename Agents**: Custom names persist across sessions
- **Agent Cleanup**: Remove offline agents and their stale signal files
- **tmux Integration**: Single-key sidebar state handling (hide/bring/spawn)

## Installation

```bash
cargo install --path .
valkyrie install
```

Restart opencode after installation.

## tmux Integration

To enable state-aware single-key sidebar behavior:

```bash
# Install scripts and print tmux config
valkyrie setup-tmux

# Add the printed config to ~/.tmux.conf, then reload
tmux source ~/.tmux.conf
```

This installs helper scripts to `~/.local/bin/` and prints the tmux configuration.
If you previously configured auto-follow, remove any old `session-window-changed` hook that runs `move-sidebar-to-current.sh`.

After setup:
- `prefix + s` hides the sidebar when it is in the current window
- `prefix + s` brings the sidebar here when it exists elsewhere (including hidden session)
- `prefix + s` spawns the sidebar when it is not running

## Usage

```bash
# Run the sidebar (must be in tmux)
valkyrie

# Configure path trimming for worktree display labels
valkyrie config add-trim-path /path/to/project

# Remove a trim path
valkyrie config remove-trim-path /path/to/project

# List configured trim paths
valkyrie config list-trim-paths

# Configure sidebar width (default: 50 columns)
valkyrie config set-sidebar-width 65

# Clear custom sidebar width and use default
valkyrie config clear-sidebar-width

# Show current config
valkyrie config show
```

## Keybindings

| Key | Action |
|-----|--------|
| `j/k` | Navigate agents |
| `Enter` | Jump to agent pane |
| `w` | Open worktree in new pane |
| `r` | Rename agent |
| `d` | View git diff (inline) |
| `D` | View git diff (new window) |
| `x` | Cleanup selected offline agent |
| `X` | Cleanup all offline agents |
| `?` | Show help |
| `q/Esc` | Quit |

## Architecture

See [docs/architecture.md](docs/architecture.md) for full design documentation.

## License

MIT
