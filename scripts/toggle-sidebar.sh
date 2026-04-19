#!/bin/bash

SIDEBAR_STATE="$HOME/.agent-sidebar/sidebar-pane"
SIDEBAR_WIDTH="30"

get_current_session() {
    tmux display-message -p '#{session_name}'
}

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

hide_sidebar() {
    local pane_id="$1"
    local hidden_window
    hidden_window=$(tmux new-window -P -F '#{window_id}' -n "__sidebar_hidden__")
    tmux break-pane -s "$pane_id" -t "$hidden_window"
    tmux select-window -t "$(get_current_window)"
}

show_sidebar_in_current_window() {
    local pane_id="$1"
    local current_window
    current_window=$(get_current_window)
    tmux join-pane -hb -l "$SIDEBAR_WIDTH" -s "$pane_id" -t "$current_window"
}

spawn_sidebar() {
    local current_path
    current_path=$(tmux display-message -p '#{pane_current_path}')
    tmux split-window -hb -l "$SIDEBAR_WIDTH" -c "$current_path" "agent-sidebar"
}

main() {
    local sidebar_pane
    sidebar_pane=$(get_sidebar_info)
    
    if [[ -n "$sidebar_pane" ]]; then
        local sidebar_window current_window
        sidebar_window=$(get_pane_window "$sidebar_pane")
        current_window=$(get_current_window)
        
        if [[ "$sidebar_window" == "$current_window" ]]; then
            hide_sidebar "$sidebar_pane"
        else
            show_sidebar_in_current_window "$sidebar_pane"
        fi
    else
        spawn_sidebar
    fi
}

main
