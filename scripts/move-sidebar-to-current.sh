#!/bin/bash

SIDEBAR_STATE="$HOME/.valkyrie/sidebar-pane"
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

redistribute_panes() {
    local window_id="$1"
    local sidebar_pane="$2"
    local window_width
    window_width=$(tmux display-message -t "$window_id" -p '#{window_width}')
    local remaining=$((window_width - SIDEBAR_WIDTH))

    local panes=()
    while IFS= read -r pane; do
        [[ "$pane" != "$sidebar_pane" ]] && panes+=("$pane")
    done < <(tmux list-panes -t "$window_id" -F '#{pane_id}')

    local count=${#panes[@]}
    if [[ "$count" -eq 0 ]]; then
        return
    fi

    local pane_width=$((remaining / count))
    local cmds="resize-pane -t ${sidebar_pane} -x ${SIDEBAR_WIDTH}"$'\n'
    for pane in "${panes[@]}"; do
        cmds+="resize-pane -t ${pane} -x ${pane_width}"$'\n'
    done
    echo "$cmds" | tmux source-file -
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

        local leftmost_pane
        leftmost_pane=$(tmux list-panes -t "$current_window" -F '#{pane_left} #{pane_id}' | sort -n | head -1 | awk '{print $2}')

        tmux join-pane -hb -l "$SIDEBAR_WIDTH" -s "$sidebar_pane" -t "$leftmost_pane" 2>/dev/null

        sleep 0.05

        redistribute_panes "$current_window" "$sidebar_pane"
        tmux select-pane -t "$active_pane"
    fi
}

main
