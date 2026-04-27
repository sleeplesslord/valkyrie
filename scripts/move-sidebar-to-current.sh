#!/bin/bash

SIDEBAR_STATE="$HOME/.valkyrie/sidebar-pane"
SIDEBAR_WIDTH="30"
LOCK_NAME="valkyrie-sidebar-move"

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
    tmux display-message -p -t "$pane_id" '#{window_id}' 2>/dev/null
}

main() {
    tmux wait-for -L "$LOCK_NAME" 2>/dev/null || exit 0
    trap 'tmux wait-for -U "$LOCK_NAME" 2>/dev/null' EXIT

    local sidebar_pane
    sidebar_pane=$(get_sidebar_info)

    if [[ -z "$sidebar_pane" ]]; then
        exit 0
    fi

    local sidebar_window current_window
    sidebar_window=$(get_pane_window "$sidebar_pane")
    current_window=$(get_current_window)

    if [[ "$sidebar_window" == "$current_window" ]]; then
        exit 0
    fi

    local leftmost_pane
    leftmost_pane=$(tmux list-panes -t "$current_window" -F '#{pane_left} #{pane_id}' | sort -n | head -1 | awk '{print $2}')

    tmux join-pane -bdfh -l "$SIDEBAR_WIDTH" -s "$sidebar_pane" -t "$leftmost_pane" 2>/dev/null || exit 0
    tmux resize-pane -t "$sidebar_pane" -x "$SIDEBAR_WIDTH" 2>/dev/null || true
}

main
