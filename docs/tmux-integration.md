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
1. Signal file presence in `~/.valkyrie/agents/` (primary — for opencode)
2. `pane_current_command` matching known agent processes (for Claude Code via `ClaudeDetector`)
3. Not matching the sidebar pane itself

## Navigation

### Jump to Agent Pane

Pane IDs are globally unique and stable for the tmux server lifetime. Use them directly:

```bash
tmux select-pane -t %N
```

**Do not use cached `window_id` or `session_name` for navigation.** These become stale when panes move between windows (via `break-pane`, `join-pane`, `move-pane`). The two-step `select-window` → `select-pane` approach can jump to the wrong window.

```bash
# WRONG — uses stale cached location
tmux select-window -t main:@0
tmux select-pane -t main:@0.%1

# RIGHT — pane ID is ground truth, tmux auto-switches to its current window
tmux select-pane -t %1
```

Cached `window_id`/`session_name` should only be used for display/organizational purposes (e.g., grouping agents by window in the sidebar), never for navigation commands.

## Sidebar States

The `toggle-sidebar.sh` script provides three-state behavior on a single key (`prefix + s`):

1. **Sidebar in current window** → hide it (move to a detached hidden session named `_valkyrie`)
2. **Sidebar in different window or hidden** → bring it to the current window
3. **No sidebar running** → spawn a new sidebar pane

State is tracked in `~/.valkyrie/sidebar-pane`.

## Pane Renaming

### Rename Pane Title

```bash
tmux select-pane -t <pane_id> -T "<new_title>"
```

Note: User-renamed names stored in `~/.valkyrie/state.json` for persistence.

## Sidebar Cleanup

### On Exit

When sidebar TUI exits:
1. Pane automatically closes when process ends
2. State saved to `~/.valkyrie/state.json` before exit

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

### Manual Config

Users can add to `~/.tmux.conf`:

```bash
# Toggle sidebar with prefix+s
bind s run-shell "~/.local/bin/toggle-sidebar.sh"

# Close window when sidebar is the only pane left
set-hook -g pane-exited 'run-shell "~/.local/bin/check-sidebar-window-close.sh"'
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

## Terminal Handling

### Sidebar Terminal Setup

On startup:
1. Save original terminal settings
2. Enter alternate screen buffer
3. Enable raw mode
4. Hide cursor (show for input)

On exit:
1. Restore terminal settings
2. Exit alternate screen buffer
3. Show cursor

Handled automatically by ratatui/crossterm.
