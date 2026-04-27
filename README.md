# valkyrie

A tmux sidebar TUI for tracking coding agents in real-time.

## Features

- **Agent Detection**: Automatically detects opencode, Claude Code, Aider, and generic agents
- **Status Tracking**: Shows running, idle, waiting for input, error, and offline states
- **Signal Protocol**: Real-time updates via signal files
- **Worktree Grouping**: Groups agents by git worktree for better organization
- **Git Diff Stats**: Shows +X/-Y diff statistics for each agent
- **Jump to Pane**: Navigate directly to agent panes from sidebar
- **Rename Agents**: Custom names persist across sessions
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

# Configure worktree root for grouping
valkyrie config set-worktree-root /path/to/project

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
| `r` | Rename agent |
| `d` | View git diff |
| `?` | Show help |
| `q/Esc` | Quit |

## Architecture

See [docs/architecture.md](docs/architecture.md) for full design documentation.

## License

MIT
