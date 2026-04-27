#!/bin/bash

SIDEBAR_STATE="$HOME/.valkyrie/sidebar-pane"
SIDEBAR_HIDDEN_SESSION="__valkyrie_hidden__"
SIDEBAR_HIDDEN_WINDOW="__sidebar_hidden__"
SIDEBAR_HIDDEN_BOOTSTRAP="__valkyrie_bootstrap__"

sidebar_pane_exists() {
    local pane_id="$1"
    tmux list-panes -a -F '#{pane_id}' 2>/dev/null | grep -q "^${pane_id}$"
}

get_pane_window() {
    local pane_id="$1"
    tmux display-message -p -t "$pane_id" '#{window_id}' 2>/dev/null
}

get_window_name() {
    local window_id="$1"
    tmux display-message -p -t "$window_id" '#{window_name}' 2>/dev/null
}

count_panes_in_window() {
    local window_id="$1"
    tmux list-panes -t "$window_id" -F '#{pane_id}' 2>/dev/null | wc -l
}

ensure_hidden_session() {
    if tmux has-session -t "$SIDEBAR_HIDDEN_SESSION" 2>/dev/null; then
        return 0
    fi

    tmux new-session -d -s "$SIDEBAR_HIDDEN_SESSION" -n "$SIDEBAR_HIDDEN_BOOTSTRAP" >/dev/null 2>&1
}

cleanup_hidden_bootstrap() {
    local window_count
    window_count=$(tmux list-windows -t "$SIDEBAR_HIDDEN_SESSION" -F '#{window_id}' 2>/dev/null | wc -l | tr -d ' ')
    if [[ "$window_count" -gt 1 ]]; then
        tmux kill-window -t "$SIDEBAR_HIDDEN_SESSION:$SIDEBAR_HIDDEN_BOOTSTRAP" 2>/dev/null || true
    fi
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
    if [[ "$window_name" == "$SIDEBAR_HIDDEN_WINDOW" ]]; then
        exit 0
    fi

    local pane_count
    pane_count=$(count_panes_in_window "$window_id")
    if [[ "$pane_count" -eq 1 ]]; then
        ensure_hidden_session || exit 0
        tmux break-pane -d -s "$pane_id" -t "$SIDEBAR_HIDDEN_SESSION:" -n "$SIDEBAR_HIDDEN_WINDOW" 2>/dev/null || exit 0
        cleanup_hidden_bootstrap
    fi
}

main
