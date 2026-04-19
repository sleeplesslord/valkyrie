# agent-sidebar

A tmux sidebar TUI for tracking coding agents in real-time.

## Features

- **Agent Detection**: Automatically detects opencode, Claude Code, Aider, and generic agents
- **Status Tracking**: Shows running, idle, waiting for input, error, and offline states
- **Signal Protocol**: Real-time updates via signal files
- **Worktree Grouping**: Groups agents by git worktree for better organization
- **Git Diff Stats**: Shows +X/-Y diff statistics for each agent
- **Jump to Pane**: Navigate directly to agent panes from sidebar
- **Rename Agents**: Custom names persist across sessions
- **tmux Integration**: Sidebar follows when switching windows, toggle visibility

## Installation

```bash
cargo install --path .
agent-sidebar install
```

Restart opencode after installation.

## tmux Integration

To enable the sidebar to follow you when switching windows and toggle visibility:

```bash
# Install scripts and print tmux config
agent-sidebar setup-tmux

# Add the printed config to ~/.tmux.conf, then reload
tmux source ~/.tmux.conf
```

This installs helper scripts to `~/.local/bin/` and prints the tmux configuration.

After setup:
- `prefix + s` toggles the sidebar (show/hide)
- Sidebar automatically moves to the current window when switching

## Usage

```bash
# Run the sidebar (must be in tmux)
agent-sidebar

# Configure worktree root for grouping
agent-sidebar config set-worktree-root /path/to/project

# Show current config
agent-sidebar config show
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
