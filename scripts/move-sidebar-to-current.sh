#!/bin/bash

SIDEBAR_STATE="$HOME/.agent-sidebar/sidebar-pane"
SIDEBAR_WIDTH="30"

get_current_window() {
    tmux display-message -p '#{window_id}'
}

sidebar_pane_exists() {
    local pane_id="$1"
    tmux list-panes -a -F '#{pane_id}' 2>/dev/null | grep -q "^${pane_id}$"
}

get_sidebar_info() {
    if [[ -f "$SIDEBAR_STATE" ]]; then
        local pane_id
        pane_id=$(cat "$SIDEBAR_STATE" 2>/dev/null)
        if [[ -n "$pane_id" ]] && sidebar_pane_exists "$pane_id"; then
            echo "$pane_id"
            return 0
        fi
    fi
    return 1
}

get_pane_window() {
    local pane_id="$1"
    tmux list-panes -a -F '#{pane_id}:#{window_id}' 2>/dev/null | grep "^${pane_id}:" | cut -d: -f2
}

main() {
    local sidebar_pane
    sidebar_pane=$(get_sidebar_info)
    
    if [[ -z "$sidebar_pane" ]]; then
        exit 0
    fi
    
    local sidebar_window current_window
    sidebar_window=$(get_pane_window "$sidebar_pane")
    current_window=$(get_current_window)
    
    if [[ "$sidebar_window" != "$current_window" ]]; then
        local active_pane
        active_pane=$(tmux display-message -p '#{pane_id}')

        tmux join-pane -hb -l "$SIDEBAR_WIDTH" -s "$sidebar_pane" -t "$current_window" 2>/dev/null

        local leftmost_pane
        leftmost_pane=$(tmux list-panes -t "$current_window" -F '#{pane_left} #{pane_id}' | sort -n | head -1 | awk '{print $2}')
        if [[ -n "$leftmost_pane" && "$leftmost_pane" != "$sidebar_pane" ]]; then
            tmux swap-pane -s "$sidebar_pane" -t "$leftmost_pane"
        fi

        tmux resize-pane -t "$sidebar_pane" -x "$SIDEBAR_WIDTH"

        tmux select-pane -t "$active_pane"
    fi
}

main
