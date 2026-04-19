# tmux Integration

## Sidebar Spawning

### Launch Command

```bash
tmux split-window -hb -l 30 -c "#{pane_current_path}" "agent-sidebar"
```

| Flag | Purpose |
|------|---------|
| `-h` | Horizontal split (creates vertical pane) |
| `-b` | Split to left (before current pane) |
| `-l 30` | Fixed width of 30 columns |
| `-c` | Start in current working directory |

### Session Attachment

When launched, sidebar should:
1. Check if already in tmux session
2. If not, show error and exit
3. Spawn as new pane in current window
4. Set pane title to "agent-sidebar"

## Pane Discovery

### List All Panes

```bash
tmux list-panes -a -F '#{session_name}:#{window_id}:#{pane_id}|#{pane_title}|#{pane_current_command}|#{pane_current_path}|#{pane_active}'
```

Output format:
```
main:@0:%0|zsh|zsh|/home/user/project|1
main:@1:%1|agent-sidebar|agent-sidebar|/home/user/project|0
```

### Filter for Agent Panes

Detect potential agent panes by:
1. `pane_current_command` matching known agent processes
2. `pane_title` containing agent identifiers
3. `pane_current_path` matching project patterns

## Navigation

### Jump to Agent Pane

```bash
tmux select-pane -t <session>:<window>.<pane>
```

Example:
```bash
tmux select-pane -t main:@0.%0
```

### Return to Sidebar

After jumping to agent, user can:
1. Use tmux navigation (`prefix + arrow`)
2. Press `Esc` or `q` in agent to return (requires agent support)

## Pane Renaming

### Rename Window

```bash
tmux rename-window -t <window_id> "<new_name>"
```

### Rename Pane Title

```bash
tmux select-pane -t <pane_id> -T "<new_title>"
```

Note: User-renamed names stored in `~/.agent-sidebar/state.json` for persistence.

## Sidebar Cleanup

### On Exit

When sidebar TUI exits:
1. Kill the pane: `tmux kill-pane -t <sidebar_pane_id>`
2. Or: Pane automatically closes when process ends

### Graceful Shutdown

Handle `SIGTERM` and `SIGINT` to:
1. Save state to `state.json`
2. Restore terminal state
3. Exit cleanly

## tmux Environment Variables

Useful for detecting sidebar context:

| Variable | Description |
|----------|-------------|
| `TMUX` | Contains session ID, window, pane |
| `TMUX_PANE` | Current pane ID |
| `PANE` | (set by tmux) Current pane ID |

### Parse TMUX Variable

```
TMUX=/tmp/tmux-1000/default,1234,0
         socket path          ,pid,session_id
```

## Integration Hooks

### Quick Setup

Run `agent-sidebar setup-tmux` to print the recommended configuration. Add the output to `~/.tmux.conf` and reload with `tmux source ~/.tmux.conf`.

### Toggle Sidebar

The `toggle-sidebar.sh` script provides toggle functionality:

- If no sidebar exists: spawns new sidebar pane
- If sidebar exists in current window: hides it (moves to hidden window)
- If sidebar exists in different window: moves it to current window

State is tracked in `~/.agent-sidebar/sidebar-pane`.

### Window Switch Hook

The `move-sidebar-to-current.sh` script is triggered on `session-window-changed` hook to move the sidebar to the current window automatically.

### Manual Config

Users can add to `~/.tmux.conf`:

```bash
# Toggle sidebar with prefix+s
bind s run-shell "/path/to/toggle-sidebar.sh"

# Move sidebar when switching windows
set-hook -g session-window-changed 'run-shell "/path/to/move-sidebar-to-current.sh"'
```

### Auto-start

For automatic sidebar on session creation:

```bash
# In ~/.tmux.conf
set-hook -g session-created 'split-window -hb -l 30 "agent-sidebar"'
```

Note: Currently not recommended; manual launch preferred.

## Pane Content Capture

### Capture Current Content

```bash
tmux capture-pane -t <pane_id> -p -S -50
```

| Flag | Purpose |
|------|---------|
| `-p` | Print to stdout (don't copy to buffer) |
| `-S -50` | Start 50 lines back from cursor |
| `-E -` | End at cursor position |

Used for fallback status detection when signal files unavailable.

## Terminal Handling

### Sidebar Terminal Setup

On startup:
1. Save original terminal settings
2. Enter alternate screen buffer
3. Enable raw mode
4. Hide cursor (show for input)

On exit:
1. Restore terminal settings
3. Exit alternate screen buffer
4. Show cursor

Handled automatically by ratatui/crossterm.
