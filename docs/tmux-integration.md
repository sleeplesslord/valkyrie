# tmux Integration

## Sidebar Spawning

### Launch Command

```bash
tmux split-window -hb -l 50 -c "#{pane_current_path}" "valkyrie"
```

| Flag | Purpose |
|------|---------|
| `-h` | Horizontal split (creates vertical pane) |
| `-b` | Split to left (before current pane) |
| `-l 50` | Fixed width of 50 columns |
| `-c` | Start in current working directory |

### Session Attachment

When launched, sidebar should:
1. Check if already in tmux session
2. If not, show error and exit
3. Spawn as new pane in current window
4. Set pane title to "valkyrie"

## Pane Discovery

### List All Panes

```bash
tmux list-panes -a -F '#{session_name}:#{window_id}:#{pane_id}|#{pane_title}|#{pane_current_command}|#{pane_current_path}|#{pane_active}'
```

Output format:
```
main:@0:%0|zsh|zsh|/home/user/project|1
main:@1:%1|valkyrie|valkyrie|/home/user/project|0
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

Note: User-renamed names stored in `~/.valkyrie/state.json` for persistence.

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

Run `valkyrie setup-tmux` to print the recommended configuration. Add the output to `~/.tmux.conf` and reload with `tmux source ~/.tmux.conf`.
If you previously configured auto-follow, remove any `session-window-changed` hook that runs `move-sidebar-to-current.sh`.

Sidebar width defaults to 50 columns and can be changed with:

```bash
valkyrie config set-sidebar-width 65
```

After updating width, run `valkyrie setup-tmux` again if your installed scripts are outdated.

### Toggle Sidebar

The `toggle-sidebar.sh` script provides toggle functionality:

- If no sidebar exists: spawns new sidebar pane
- If sidebar exists in current window: hides it (moves to a detached hidden session)
- If sidebar exists in different window or hidden session: moves it to current window

State is tracked in `~/.valkyrie/sidebar-pane`.

### Manual Config

Users can add to `~/.tmux.conf`:

```bash
# Toggle sidebar with prefix+s
bind s run-shell "/path/to/toggle-sidebar.sh"

# Close window when sidebar is the only pane left
set-hook -g pane-exited 'run-shell "/path/to/check-sidebar-window-close.sh"'
```

### Auto-start

For automatic sidebar on session creation:

```bash
# In ~/.tmux.conf
set-hook -g session-created 'split-window -hb -l 50 "valkyrie"'

# Or use a configured width value
set-hook -g session-created 'split-window -hb -l "$(valkyrie config get-sidebar-width)" "valkyrie"'
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
