# Valkyrie × Codex Integration Plan

Add Codex support to Valkyrie via Codex Hooks, mirroring the existing opencode plugin.

## Background

### Current opencode plugin architecture
- **In-process** Node.js module (`plugin/index.js`, 788 lines) loaded by opencode
- Uses opencode's event hooks (`session.status`, `session.idle`, `permission.asked`, `file.edited`, `command.executed`, …) and tool hooks (`tool.execute.before/after`)
- Maintains **in-memory state** across the session: tracked sagas, subagents, current tool, etc.
- Writes signal files to `~/.valkyrie/agents/<TMUX_PANE>.json`
- Pane-ID recovery via heartbeat-driven `syncPaneId()`
- Installed by `valkyrie install` → copies to `~/.config/opencode/plugins/valkyrie/` + registers in `opencode.json`

### Codex Hooks architecture (key differences)
- **External scripts** — each hook is a separate process that receives JSON on stdin, outputs JSON on stdout
- **Stateless** — no in-process state persists between hook invocations; must persist to disk
- Events: `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PermissionRequest`, `PostToolUse`, `SubagentStart`, `SubagentStop`, `Stop`, `PreCompact`, `PostCompact`
- Config via `~/.codex/hooks.json` or inline `[hooks]` in `config.toml`
- Plugin system: `.codex-plugin/plugin.json` manifest + `hooks/hooks.json`; Codex sets `$PLUGIN_ROOT` and `$PLUGIN_DATA` env vars
- **Trust flow**: hooks must be reviewed/trusted via `/hooks` before they run (unless managed or `--dangerously-bypass-hook-trust`)
- No direct `session.busy`/`session.idle` — status must be **inferred** from lifecycle events
- No `file.edited` event — file tracking via `PreToolUse`/`PostToolUse` with `apply_patch` matcher

---

## Architecture Decision: Single Python Script

**One script handles all events** — reads `hook_event_name` from stdin JSON and dispatches internally.

**Why Python (not Node/shell):**
- No runtime dependency on Node.js (opencode plugin requires it; Codex users may not have it)
- Universal availability, clean JSON handling, readable
- Single self-contained file

**Why single script (not per-event):**
- Mirrors opencode plugin's single-module architecture
- Shares state management, saga detection, signal writing logic
- One file to maintain, one file to trust

---

## File Structure

```
valkyrie/
├── codex/                              # NEW: Codex hook integration
│   ├── hooks/
│   │   └── valkyrie_hook.py            # Single hook script (all events)
│   └── hooks.json                      # Hook definitions (for direct install)
├── plugin/                             # EXISTING: opencode plugin (unchanged)
│   ├── index.js
│   ├── package.json
│   └── README.md
└── src/
    ├── plugin.rs                       # MODIFIED: add codex install/uninstall/status
    ├── agent/
    │   ├── model.rs                    # MODIFIED: add Codex variant to AgentType
    │   └── registry.rs                 # (no change — Codex relies on signal files, like opencode)
    ├── app.rs                          # MODIFIED: add "codex" to parse_signal_agent_type
    └── main.rs                         # MODIFIED: add --codex flag to Install/Uninstall/Status
```

---

## 1. Hook Script: `codex/hooks/valkyrie_hook.py`

### State Management

Since each hook invocation is a separate process, state must be persisted to disk:

**State file**: `~/.valkyrie/codex-state/<pane_id>.json`
```json
{
  "pane_id": "%5",
  "tracked_sagas": { "abc123": { "id": "abc123", "title": "...", "status": "active", ... } },
  "tracked_subagents": { "agent-1": { "id": "agent-1", "name": "...", "status": "running", ... } },
  "last_bash_command": "sg claim abc123",
  "permission_pending": false,
  "current_tool": "Bash",
  "current_activity": "running",
  "current_file": "src/main.rs",
  "current_task": "Fix the auth bug",
  "current_label": null,
  "current_worktree": null,
  "working_dir": "/home/user/project",
  "last_log": "Fixed JWT validation",
  "last_status": "running"
}
```

**Signal file** (what the TUI reads): `~/.valkyrie/agents/<pane_id>.json` — same schema as opencode plugin, with `agent_type: "codex"`.

Flow per hook invocation:
1. Read JSON from stdin
2. Get pane ID from `$TMUX_PANE`
3. Load state from state file (or initialize fresh)
4. Update state based on event
5. Save state file
6. Write signal file (the TUI reads this via inotify)

### Event → Action Mapping

| Codex Event | Status | Activity | Key Actions |
|---|---|---|---|
| `SessionStart` | `running` | `thinking` | Init state, set `working_dir` from `cwd`, resolve session label from `~/.codex/session_index.jsonl` |
| `UserPromptSubmit` | `running` | `thinking` | Set `current_task` from `prompt` (truncated 80 chars) |
| `PreToolUse` | `running` | from tool map | Set `current_tool`, extract Bash commands for saga detection, extract file paths from `apply_patch` |
| `PermissionRequest` | `waiting_input` | `waiting` | Set `permission_pending = true` |
| `PostToolUse` | `running` | `thinking` | Clear `current_tool`, parse Bash output for saga JSON, clear `permission_pending` |
| `SubagentStart` | (no change) | (no change) | Add subagent to `tracked_subagents` |
| `SubagentStop` | (no change) | (no change) | Remove subagent from `tracked_subagents` |
| `Stop` | `idle` | null | Clear `current_tool`, `current_activity` |
| `PreCompact`/`PostCompact` | (no change) | `thinking` | Update timestamp only |

### Tool → Activity Mapping

| Codex `tool_name` | Valkyrie `activity` |
|---|---|
| `Bash` | `running` |
| `apply_patch` | `coding` |
| `mcp__*` | `exploring` |
| Other | `thinking` |

### Saga Tracking (shared logic with opencode plugin)

Same regex-based detection as opencode plugin:
- `PreToolUse` (Bash): extract `sg <subcmd> <id>` from `tool_input.command`, track interaction
- `PostToolUse` (Bash): parse `tool_response` for `sg context --format json` output, extract `sg new` output for new saga IDs
- Multi-ID commands (claim, done, unclaim, wontdo): extract trailing IDs
- `sg log`: capture log message as `last_log`
- Saga metadata refresh via `sg context <id> --format json` (spawned from hook script)

### Subagent Tracking

Codex provides cleaner subagent events than opencode:
- `SubagentStart`: `agent_id`, `agent_type` (use as name), `turn_id`
- `SubagentStop`: `agent_id`, `agent_type`, `last_assistant_message`

Track in `state["tracked_subagents"][agent_id]`:
```json
{
  "id": "agent-1",
  "name": "general",
  "prompt": null,
  "status": "running",
  "activity": "thinking",
  "tool_executing": null,
  "last_update": "2026-07-06T..."
}
```

Subagent tool events: `PreToolUse`/`PostToolUse` inputs include `session_id` (parent session). Codex doesn't directly tag tool events with subagent IDs, so subagent tool tracking may be limited (unlike opencode where `sessionID` in tool hooks identifies the subagent). This is a known limitation.

### Pane ID Handling

- Primary: `$TMUX_PANE` from environment (inherited from Codex process)
- No heartbeat-based recovery (stateless hooks can't do periodic checks)
- Optional fast check on `SessionStart`: verify pane exists in tmux, migrate signal file if desynced
- Orphaned signal files auto-cleaned by TUI during pane discovery (every 5s)
- State files in `~/.valkyrie/codex-state/` cleaned opportunistically on `SessionStart` (remove files for panes not in tmux)

### Cleanup

- No explicit signal file deletion (unlike opencode's `process.on("exit")`)
- TUI staleness detection (>60s → volatile fields filtered) and orphan cleanup (pane gone → file removed) handle this
- `Stop` hook sets status to `idle`, which is the correct end state

### Hook Output

All hooks exit 0 with no stdout output (we don't want to interfere with Codex's behavior):
- We never block tool calls (`PreToolUse` doesn't return `permissionDecision: "deny"`)
- We never block prompts (`UserPromptSubmit` doesn't return `decision: "block"`)
- We never continue the turn (`Stop` doesn't return `decision: "block"`)
- Pure observation — Valkyrie is a passive observer

---

## 2. Hook Definitions: `codex/hooks.json`

For the **direct install** approach (primary):

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "python3 __VALKYRIE_HOOK_PATH__",
            "timeout": 10
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "python3 __VALKYRIE_HOOK_PATH__",
            "timeout": 10
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "python3 __VALKYRIE_HOOK_PATH__",
            "timeout": 10
          }
        ]
      }
    ],
    "PermissionRequest": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "python3 __VALKYRIE_HOOK_PATH__",
            "timeout": 10
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "python3 __VALKYRIE_HOOK_PATH__",
            "timeout": 10
          }
        ]
      }
    ],
    "SubagentStart": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "python3 __VALKYRIE_HOOK_PATH__",
            "timeout": 10
          }
        ]
      }
    ],
    "SubagentStop": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "python3 __VALKYRIE_HOOK_PATH__",
            "timeout": 10
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "python3 __VALKYRIE_HOOK_PATH__",
            "timeout": 10
          }
        ]
      }
    ]
  }
}
```

`__VALKYRIE_HOOK_PATH__` is replaced with the absolute path at install time (e.g., `~/.codex/hooks/valkyrie_hook.py` or the home-expanded path).

`timeout: 10` ensures hooks don't block Codex if something goes wrong. The hook script should complete in <100ms (file I/O only, no network).

`PreCompact` and `PostCompact` are optional — include only if we want timestamp updates during compaction. Low priority; skip for v1.

---

## 3. Rust-Side Changes

### 3a. `src/agent/model.rs` — Add Codex variant

```rust
pub enum AgentType {
    Opencode,
    ClaudeCode,
    Codex,
}

// Display impl:
AgentType::Codex => write!(f, "codex"),
```

### 3b. `src/app.rs` — Parse signal agent type

```rust
fn parse_signal_agent_type(agent_type: &str) -> Option<AgentType> {
    match agent_type.to_ascii_lowercase().as_str() {
        "opencode" => Some(AgentType::Opencode),
        "claude-code" => Some(AgentType::ClaudeCode),
        "codex" => Some(AgentType::Codex),
        _ => None,
    }
}
```

No `CodexDetector` needed in the registry — Codex detection relies 100% on signal files (same design as opencode).

### 3c. `src/plugin.rs` — Add Codex install/uninstall/status

New functions alongside the existing opencode ones:

```rust
const CODEX_HOOK_SCRIPT: &str = include_str!("../codex/hooks/valkyrie_hook.py");
const CODEX_HOOKS_JSON_TEMPLATE: &str = include_str!("../codex/hooks.json");

fn codex_config_dir() -> PathBuf {
    dirs::home_dir().unwrap().join(".codex")
}

fn codex_hooks_dir() -> PathBuf {
    codex_config_dir().join("hooks")
}

fn codex_hooks_file() -> PathBuf {
    codex_config_dir().join("hooks.json")
}

pub fn install_codex(force: bool) -> Result<()> {
    // 1. Create ~/.codex/hooks/ if needed
    // 2. Copy valkyrie_hook.py to ~/.codex/hooks/valkyrie_hook.py
    // 3. Merge valkyrie hooks into ~/.codex/hooks.json
    //    - Load existing hooks.json (or empty)
    //    - Remove any existing "valkyrie" hooks (by matching command path)
    //    - Add our hooks with absolute path to the script
    //    - Write back
    // 4. Print trust instructions
}

pub fn uninstall_codex() -> Result<()> {
    // 1. Remove valkyrie_hook.py from ~/.codex/hooks/
    // 2. Remove valkyrie hooks from ~/.codex/hooks.json
    //    - Load, filter out entries matching our script path, write back
    // 3. Clean up state files in ~/.valkyrie/codex-state/
}

pub fn status_codex() -> Result<()> {
    // 1. Check if ~/.codex/hooks/valkyrie_hook.py exists
    // 2. Check if hooks.json contains valkyrie hooks
    // 3. Print status
}
```

**hooks.json merge strategy**: Load existing `~/.codex/hooks.json`, iterate all events, remove hooks whose `command` contains `valkyrie_hook.py`, then add our hooks. This preserves any other hooks the user has configured.

### 3d. `src/main.rs` — CLI flags

```rust
enum Commands {
    Install {
        #[arg(long, help = "Force reinstall if already installed")]
        force: bool,
        #[arg(long, help = "Install Codex hooks instead of opencode plugin")]
        codex: bool,
    },
    Uninstall {
        #[arg(long, help = "Uninstall Codex hooks instead of opencode plugin")]
        codex: bool,
    },
    Status {
        #[arg(long, help = "Check Codex hook status instead of opencode plugin")]
        codex: bool,
    },
    // ... rest unchanged
}
```

- `valkyrie install` → opencode (backward compat)
- `valkyrie install --codex` → Codex hooks
- `valkyrie uninstall --codex` → remove Codex hooks
- `valkyrie status --codex` → check Codex hooks

---

## 4. Installation Flow

### `valkyrie install --codex`

```
1. Check Python 3 is available (python3 --version)
2. Create ~/.codex/hooks/ directory
3. Copy codex/hooks/valkyrie_hook.py → ~/.codex/hooks/valkyrie_hook.py
4. Merge hook definitions into ~/.codex/hooks.json:
   - Load existing (or create empty {"hooks": {}})
   - Remove previous valkyrie hooks (match by script path)
   - Add our hooks with absolute path
   - Write back
5. Print:
   "Codex hooks installed!"
   "Hook script: ~/.codex/hooks/valkyrie_hook.py"
   "Hooks config: ~/.codex/hooks.json"
   ""
   "Next steps:"
   "1. Restart Codex (or start a new session)"
   "2. Run /hooks in Codex to review and trust the valkyrie hooks"
   "3. Start valkyrie TUI in a tmux sidebar"
```

### `valkyrie uninstall --codex`

```
1. Remove ~/.codex/hooks/valkyrie_hook.py
2. Remove valkyrie hooks from ~/.codex/hooks.json
3. Optionally clean up ~/.valkyrie/codex-state/ (state files)
4. Print confirmation
```

### `valkyrie status --codex`

```
1. Check if hook script exists at ~/.codex/hooks/valkyrie_hook.py
2. Check if hooks.json contains valkyrie hooks
3. Check if signal files exist in ~/.valkyrie/agents/ with agent_type: "codex"
4. Print status summary
```

---

## 5. Hook Script Implementation Details

### `valkyrie_hook.py` structure (~300-400 lines)

```
┌─────────────────────────────────────────┐
│  main()                                 │
│  ├── read stdin JSON                    │
│  ├── get pane_id from $TMUX_PANE        │
│  ├── load_state(pane_id)                │
│  ├── dispatch by hook_event_name:       │
│  │   ├── SessionStart                   │
│  │   ├── UserPromptSubmit               │
│  │   ├── PreToolUse                     │
│  │   ├── PermissionRequest              │
│  │   ├── PostToolUse                    │
│  │   ├── SubagentStart                  │
│  │   ├── SubagentStop                   │
│  │   └── Stop                           │
│  ├── save_state(state)                  │
│  └── exit(0)                            │
├─────────────────────────────────────────┤
│  Helpers (shared logic):                │
│  ├── write_signal(state, status?)       │
│  ├── extract_saga_ids(text) → bool      │
│  ├── parse_saga_from_output(text)       │
│  ├── extract_saga_id_from_output(text)  │
│  ├── fetch_saga_info(saga_ids)          │
│  ├── find_worktree(file_path)           │
│  ├── debug(msg)                         │
│  └── cleanup_stale_state_files()        │
└─────────────────────────────────────────┘
```

### Key implementation notes

**`PreToolUse` — Bash command extraction:**
```python
tool_input = hook_input.get("tool_input", {})
command = tool_input.get("command", "")  # Codex puts Bash command here
state["last_bash_command"] = command
extract_saga_ids(state, command)
```

**`PreToolUse` — apply_patch file extraction:**
The `tool_input` for `apply_patch` likely contains patch content. Need to inspect actual format during implementation. If `tool_input.command` contains the patch, parse file paths from diff headers (`+++ b/path/to/file`). If it contains a `file_path` field, use that directly.

**`PostToolUse` — Bash output parsing:**
```python
tool_response = hook_input.get("tool_response", {})
# tool_response structure TBD — may be {"output": "..."} or {"stdout": "...", "stderr": "..."}
output_text = tool_response.get("output") or tool_response.get("stdout") or ""
# Try parsing as saga JSON (sg context --format json)
parse_saga_from_output(state, output_text)
# Check for sg new output
if SAGA_NEW_PATTERN.search(state.get("last_bash_command", "")):
    extract_saga_id_from_output(state, output_text)
```

**Saga metadata refresh:**
Same as opencode plugin — spawn `sg context <id> --format json` for tracked saga IDs. Rate-limit to avoid spawning on every hook (check `last_saga_refresh` timestamp, refresh at most every 10s).

**State file locking:**
Use `fcntl.flock` on the state file to prevent race conditions if hooks ever fire concurrently. Low priority — different event types don't overlap in practice.

---

## 6. Testing Strategy

### Unit tests (Python, in `codex/hooks/test_valkyrie_hook.py`)

- `test_extract_saga_ids` — regex matching for various `sg` commands
- `test_parse_saga_from_output` — JSON parsing from Bash output
- `test_extract_saga_id_from_output` — `sg new` output parsing
- `test_write_signal` — signal file format correctness
- `test_load_save_state` — state round-trip
- `test_event_dispatch` — mock stdin for each event type, verify state updates

### Integration tests (shell)

```bash
# 1. Install hooks
valkyrie install --codex

# 2. Simulate SessionStart hook
echo '{"hook_event_name":"SessionStart","cwd":"/tmp","session_id":"s1","source":"startup"}' | \
  python3 ~/.codex/hooks/valkyrie_hook.py

# 3. Verify signal file created
cat ~/.valkyrie/agents/$TMUX_PANE.json | jq .agent_type  # → "codex"
cat ~/.valkyrie/agents/$TMUX_PANE.json | jq .status      # → "running"

# 4. Simulate PreToolUse (Bash)
echo '{"hook_event_name":"PreToolUse","cwd":"/tmp","session_id":"s1","tool_name":"Bash","tool_input":{"command":"sg claim abc123"}}' | \
  python3 ~/.codex/hooks/valkyrie_hook.py

# 5. Verify saga tracked
cat ~/.valkyrie/agents/$TMUX_PANE.json | jq '.sagas[0].id'       # → "abc123"
cat ~/.valkyrie/agents/$TMUX_PANE.json | jq '.sagas[0].interaction'  # → "claim"

# 6. Simulate Stop
echo '{"hook_event_name":"Stop","cwd":"/tmp","session_id":"s1"}' | \
  python3 ~/.codex/hooks/valkyrie_hook.py

# 7. Verify status changed to idle
cat ~/.valkyrie/agents/$TMUX_PANE.json | jq .status  # → "idle"
```

### Rust tests

- `parse_signal_agent_type` test: add case for `"codex"` → `AgentType::Codex`
- Verify existing tests still pass (no regressions)

### Manual end-to-end test

1. `valkyrie install --codex`
2. Start Codex in tmux pane
3. Run `/hooks` in Codex, trust valkyrie hooks
4. Start valkyrie TUI in sidebar
5. Verify Codex pane appears with `codex` agent type
6. Send a prompt, verify status changes to `running`
7. Run `sg claim <id>`, verify saga appears in sidebar
8. Wait for turn to end, verify status changes to `idle`

---

## 7. Known Limitations & Future Work

### v1 limitations
1. **No pane-ID recovery** — `$TMUX_PANE` is trusted as-is. If pane is recreated mid-session (rare), signal file may go stale until TUI cleans it up. Acceptable for v1.
2. **Subagent tool tracking limited** — Codex's `PreToolUse`/`PostToolUse` inputs include `session_id` (parent), not a subagent ID. Unlike opencode where tool hooks carry `sessionID` to identify subagents, Codex tool events can't distinguish parent vs subagent tool calls. Subagents will show as tracked but their tool_executing won't update.
3. **No `file.edited` equivalent** — file tracking relies on `PreToolUse`/`PostToolUse` with `apply_patch` matcher. The exact `tool_input` format for apply_patch needs verification during implementation.
4. **Session label resolution** — Codex doesn't include a title in hook inputs, but `~/.codex/session_index.jsonl` contains `{ "id": "...", "thread_name": "..." }` per line. The hook script reads this file on `SessionStart` (which provides `session_id`) to look up the `thread_name` and set it as the label. The file is JSONL — each line is a standalone JSON object. Reading is cheap (tail the file, parse lines, match `id`). The label is cached in state and only re-read on `SessionStart` to avoid parsing on every hook invocation.
5. **Hook trust required** — user must run `/hooks` and trust hooks after install. Can't be automated.
6. **State file accumulation** — `~/.valkyrie/codex-state/` files for dead panes accumulate. Cleaned opportunistically on `SessionStart`, but not perfectly.
7. **No PreCompact/PostCompact hooks** — skipped for v1 (no useful status change).

### Future enhancements
- **Codex plugin packaging** — package as a proper Codex plugin (`.codex-plugin/plugin.json` + marketplace entry) for cleaner installation via `/plugins`
- **Pane-ID recovery** — implement TTY-based pane matching on `SessionStart` (walk process tree, match against `tmux list-panes`)
- **`model` field** — Codex hooks provide `model` in input; store in signal `metadata` for display

---

## 8. Implementation Order

1. **`codex/hooks/valkyrie_hook.py`** — core hook script with all event handlers
2. **`codex/hooks.json`** — hook definitions template
3. **`src/agent/model.rs`** — add `Codex` variant
4. **`src/app.rs`** — add `"codex"` to `parse_signal_agent_type`
5. **`src/plugin.rs`** — add `install_codex`/`uninstall_codex`/`status_codex`
6. **`src/main.rs`** — add `--codex` flag to CLI
7. **Build & test** — `cargo build`, run unit tests
8. **Integration test** — simulate hooks via stdin piping
9. **Manual E2E test** — install, start Codex, verify in TUI
10. **Update docs** — `plugin/README.md` → add Codex section, `docs/signal-protocol.md` → add Codex integration section
