#!/bin/bash

SIDEBAR_STATE="$HOME/.valkyrie/sidebar-pane"
DEFAULT_SIDEBAR_WIDTH="50"
SIDEBAR_HIDDEN_SESSION="__valkyrie_hidden__"
SIDEBAR_HIDDEN_WINDOW="__sidebar_hidden__"
SIDEBAR_HIDDEN_BOOTSTRAP="__valkyrie_bootstrap__"

get_sidebar_width() {
    local configured_width
    configured_width=$(valkyrie config get-sidebar-width 2>/dev/null)
    if [[ "$configured_width" =~ ^[0-9]+$ ]] && [[ "$configured_width" -gt 0 ]]; then
        echo "$configured_width"
    else
        echo "$DEFAULT_SIDEBAR_WIDTH"
    fi
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
    tmux display-message -p -t "$pane_id" '#{window_id}' 2>/dev/null
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

hide_sidebar() {
    local pane_id="$1"

    ensure_hidden_session || return 1

    tmux break-pane -d -s "$pane_id" -t "$SIDEBAR_HIDDEN_SESSION:" -n "$SIDEBAR_HIDDEN_WINDOW" 2>/dev/null || return 1
    cleanup_hidden_bootstrap
}

show_sidebar_in_current_window() {
    local pane_id="$1"
    local sidebar_width="$2"
    local current_window
    current_window=$(get_current_window)

    local leftmost_pane
    leftmost_pane=$(tmux list-panes -t "$current_window" -F '#{pane_left} #{pane_id}' | sort -n | head -1 | awk '{print $2}')

    tmux join-pane -bdfh -l "$sidebar_width" -s "$pane_id" -t "$leftmost_pane" 2>/dev/null || return 1
    tmux resize-pane -t "$pane_id" -x "$sidebar_width" 2>/dev/null || true
}

spawn_sidebar() {
    local sidebar_width="$1"
    local current_path
    current_path=$(tmux display-message -p '#{pane_current_path}')
    local current_window
    current_window=$(get_current_window)

    local leftmost_pane
    leftmost_pane=$(tmux list-panes -t "$current_window" -F '#{pane_left} #{pane_id}' | sort -n | head -1 | awk '{print $2}')

    tmux split-window -bdfh -l "$sidebar_width" -c "$current_path" -t "$leftmost_pane" "valkyrie" >/dev/null 2>&1 || return 1
}

main() {
    local sidebar_width
    sidebar_width=$(get_sidebar_width)

    local sidebar_pane
    sidebar_pane=$(get_sidebar_info)

    if [[ -n "$sidebar_pane" ]]; then
        local sidebar_window current_window
        sidebar_window=$(get_pane_window "$sidebar_pane")
        current_window=$(get_current_window)

        if [[ "$sidebar_window" == "$current_window" ]]; then
            hide_sidebar "$sidebar_pane"
        else
            show_sidebar_in_current_window "$sidebar_pane" "$sidebar_width"
        fi
    else
        spawn_sidebar "$sidebar_width"
    fi
}

main
