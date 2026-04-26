#!/bin/bash

SIDEBAR_STATE="$HOME/.agent-sidebar/sidebar-pane"
SIDEBAR_WIDTH="30"

sidebar_pane_exists() {
    local pane_id="$1"
    tmux list-panes -a -F '#{pane_id}' 2>/dev/null | grep -q "^${pane_id}$"
}

get_pane_window() {
    local pane_id="$1"
    tmux list-panes -a -F '#{pane_id}:#{window_id}' 2>/dev/null | grep "^${pane_id}:" | cut -d: -f2
}

get_window_name() {
    local window_id="$1"
    tmux list-windows -a -F '#{window_id}:#{window_name}' 2>/dev/null | grep "^${window_id}:" | cut -d: -f2
}

count_panes_in_window() {
    local window_id="$1"
    tmux list-panes -t "$window_id" -F '#{pane_id}' 2>/dev/null | wc -l
}

get_other_window() {
    local exclude_window_id="$1"
    tmux list-windows -a -F '#{window_id}' 2>/dev/null | grep -v "^${exclude_window_id}$" | head -1
}

main() {
    if [[ ! -f "$SIDEBAR_STATE" ]]; then
        exit 0
    fi

    local pane_id
    pane_id=$(cat "$SIDEBAR_STATE" 2>/dev/null)
    if [[ -z "$pane_id" ]]; then
        exit 0
    fi

    if ! sidebar_pane_exists "$pane_id"; then
        exit 0
    fi

    local window_id
    window_id=$(get_pane_window "$pane_id")
    if [[ -z "$window_id" ]]; then
        exit 0
    fi

    local window_name
    window_name=$(get_window_name "$window_id")
    if [[ "$window_name" == "__sidebar_hidden__" ]]; then
        exit 0
    fi

    local pane_count
    pane_count=$(count_panes_in_window "$window_id")
    if [[ "$pane_count" -eq 1 ]]; then
        local other_window
        other_window=$(get_other_window "$window_id")
        if [[ -n "$other_window" ]]; then
            tmux join-pane -hb -l "$SIDEBAR_WIDTH" -s "$pane_id" -t "$other_window" 2>/dev/null
            sleep 0.05
            tmux resize-pane -t "$pane_id" -x "$SIDEBAR_WIDTH"
            tmux kill-window -t "$window_id" 2>/dev/null
        else
            tmux kill-window -t "$window_id" 2>/dev/null
        fi
    fi
}

main
