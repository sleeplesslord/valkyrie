#!/usr/bin/env python3
"""Valkyrie Codex hook script.

Handles all Codex hook events (SessionStart, UserPromptSubmit, PreToolUse,
PermissionRequest, PostToolUse, SubagentStart, SubagentStop, Stop) and writes
status updates to JSON signal files that the Valkyrie TUI monitors via inotify.

Codex hooks are stateless — each invocation is a separate process. State is
persisted to ~/.valkyrie/codex-state/<pane_id>.json between hook calls.

Pure observation: never blocks, denies, or continues Codex. Always exits 0.
"""

import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

# ── Constants ─────────────────────────────────────────────────────────────

SIGNAL_DIR = Path.home() / ".valkyrie" / "agents"
STATE_DIR = Path.home() / ".valkyrie" / "codex-state"
LOG_FILE = Path.home() / ".valkyrie" / "plugin.log"
CODEX_SESSION_INDEX = Path.home() / ".codex" / "session_index.jsonl"

TOOL_ACTIVITY = {
    "Bash": "running",
    "apply_patch": "coding",
    "Edit": "coding",
    "Write": "coding",
}

SAGA_ID_PATTERN = re.compile(
    r"\bsg\s+(claim|done|log|context|edit|label|priority|depend|relate|unclaim|continue|reopen|wontdo)\s+([\w.-]+)"
)
SAGA_NEW_PATTERN = re.compile(r"\bsg\s+new\b")
MULTI_ID_COMMANDS = {"claim", "done", "unclaim", "wontdo"}
SAGA_REFRESH_INTERVAL = 10.0  # seconds

SAGA_OUTPUT_ID_PATTERN = re.compile(
    r"(?:created|saga|Saga)\s+(?:saga\s+)?([a-z0-9]{5,}(?:\.\d+)?)", re.IGNORECASE
)


# ── Helpers ───────────────────────────────────────────────────────────────

def debug(*args):
    """Fire-and-forget debug log to ~/.valkyrie/plugin.log."""
    try:
        ts = datetime.now(timezone.utc).isoformat()[11:19]
        line = f"[{ts}] " + " ".join(str(a) for a in args) + "\n"
        with open(LOG_FILE, "a") as f:
            f.write(line)
    except Exception:
        pass


def now_iso():
    return datetime.now(timezone.utc).isoformat()


def find_worktree(file_path):
    """Return the git worktree root for a file, or None."""
    if not file_path:
        return None
    try:
        directory = os.path.dirname(file_path) if os.path.dirname(file_path) else "."
        result = subprocess.run(
            ["git", "-C", directory, "rev-parse", "--show-toplevel"],
            capture_output=True, text=True, timeout=5,
        )
        if result.returncode == 0:
            return result.stdout.strip() or None
    except Exception:
        pass
    return None


def resolve_session_label(session_id):
    """Look up thread_name from ~/.codex/session_index.jsonl by session id."""
    if not session_id:
        return None
    try:
        with open(CODEX_SESSION_INDEX, "r") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    data = json.loads(line)
                    if data.get("id") == session_id:
                        return data.get("thread_name") or None
                except json.JSONDecodeError:
                    continue
    except (FileNotFoundError, IOError):
        pass
    return None


# ── State Management ──────────────────────────────────────────────────────

def default_state(pane_id):
    return {
        "pane_id": pane_id,
        "tracked_sagas": {},
        "tracked_subagents": {},
        "last_bash_command": None,
        "permission_pending": False,
        "current_tool": None,
        "current_activity": None,
        "current_file": None,
        "current_task": None,
        "current_label": None,
        "current_worktree": None,
        "working_dir": None,
        "current_session_id": None,
        "last_log": None,
        "last_status": "idle",
        "last_saga_refresh": 0.0,
        "saga_refresh_needed": False,
    }


def load_state(pane_id):
    path = STATE_DIR / f"{pane_id}.json"
    try:
        with open(path, "r") as f:
            state = json.load(f)
        # Ensure all keys exist (forward compat)
        defaults = default_state(pane_id)
        for k, v in defaults.items():
            if k not in state:
                state[k] = v
        return state
    except (FileNotFoundError, json.JSONDecodeError):
        return default_state(pane_id)


def save_state(state):
    try:
        STATE_DIR.mkdir(parents=True, exist_ok=True)
        path = STATE_DIR / f"{state['pane_id']}.json"
        with open(path, "w") as f:
            json.dump(state, f)
    except Exception as e:
        debug("save_state error:", e)


# ── Signal Writing ─────────────────────────────────────────────────────────

def write_signal(state, status=None):
    if status:
        state["last_status"] = status
    try:
        SIGNAL_DIR.mkdir(parents=True, exist_ok=True)
        # Refresh saga metadata if needed
        refresh_sagas(state)
        sagas = list(state["tracked_sagas"].values())
        sagas.reverse()
        sagas = sagas[:10]
        subagents = list(state["tracked_subagents"].values())
        signal = {
            "version": 1,
            "agent_type": "codex",
            "status": state["last_status"],
            "activity": state["current_activity"],
            "tool_executing": state["current_tool"],
            "task": state["current_task"],
            "label": state["current_label"],
            "working_dir": state["working_dir"],
            "worktree": state["current_worktree"],
            "current_file": state["current_file"],
            "last_update": now_iso(),
            "sagas": sagas if sagas else None,
            "last_log": state["last_log"],
            "subagents": subagents if subagents else None,
        }
        # Remove None values to keep signal compact
        signal = {k: v for k, v in signal.items() if v is not None}
        path = SIGNAL_DIR / f"{state['pane_id']}.json"
        with open(path, "w") as f:
            json.dump(signal, f, indent=2)
    except Exception as e:
        debug("write_signal error:", e)


# ── Saga Tracking ──────────────────────────────────────────────────────────

def split_shell_commands(text):
    return [s for s in re.split(r"\s*(?:&&|\|\||[;|]|\n)\s*", text) if s]


def extract_saga_ids(state, text):
    """Detect sg subcommands and track saga IDs. Returns True if any found."""
    found = False
    for segment in split_shell_commands(text):
        for m in SAGA_ID_PATTERN.finditer(segment):
            subcmd = m.group(1)
            saga_id = m.group(2)
            existing = state["tracked_sagas"].get(saga_id)
            state["tracked_sagas"][saga_id] = {
                "id": saga_id,
                "title": existing["title"] if existing else "",
                "status": existing["status"] if existing else "unknown",
                "claimed_by": existing["claimed_by"] if existing else None,
                "interaction": subcmd,
                "interaction_at": now_iso(),
            }
            found = True
            debug("extract_saga_ids:", subcmd, saga_id)

            # Capture log message from sg log commands
            if subcmd == "log":
                tail = segment[m.end():].strip()
                msg = None
                if tail.startswith('"'):
                    end = tail.find('"', 1)
                    msg = tail[1:end] if end > 0 else tail[1:]
                elif tail.startswith("'"):
                    end = tail.find("'", 1)
                    msg = tail[1:end] if end > 0 else tail[1:]
                elif tail and not tail.startswith("-"):
                    msg = tail
                if msg:
                    state["last_log"] = msg
                    debug("extract_log_message:", msg)

            # Multi-ID commands (e.g. "sg claim abc def"): pick up trailing IDs
            if subcmd in MULTI_ID_COMMANDS:
                tail = segment[m.end():]
                for tm in re.finditer(r"\s+([\w.-]+)", tail):
                    if tm.group(1).startswith("-"):
                        break
                    tid = tm.group(1)
                    tex = state["tracked_sagas"].get(tid)
                    state["tracked_sagas"][tid] = {
                        "id": tid,
                        "title": tex["title"] if tex else "",
                        "status": tex["status"] if tex else "unknown",
                        "claimed_by": tex["claimed_by"] if tex else None,
                        "interaction": subcmd,
                        "interaction_at": now_iso(),
                    }
                    debug("extract_saga_ids tail:", subcmd, tid)

    return found


def extract_saga_id_from_output(state, text):
    """Detect new saga IDs from `sg new` command output."""
    if not text:
        return False
    m = SAGA_OUTPUT_ID_PATTERN.search(text)
    if m:
        saga_id = m.group(1)
        existing = state["tracked_sagas"].get(saga_id)
        state["tracked_sagas"][saga_id] = {
            "id": saga_id,
            "title": existing["title"] if existing else "",
            "status": existing["status"] if existing else "unknown",
            "claimed_by": existing["claimed_by"] if existing else None,
            "interaction": "new",
            "interaction_at": now_iso(),
        }
        debug("extract_saga_id_from_output: new", saga_id)
        return True
    return False


def parse_saga_from_output(state, tool_output):
    """Parse saga JSON directly from sg context --format json output."""
    if not tool_output or not isinstance(tool_output, str):
        return False
    try:
        data = json.loads(tool_output)
        if data and isinstance(data, dict) and data.get("saga", {}).get("id"):
            saga = data["saga"]
            saga_id = saga["id"]
            existing = state["tracked_sagas"].get(saga_id)
            state["tracked_sagas"][saga_id] = {
                "id": saga_id,
                "title": saga.get("title", ""),
                "status": saga.get("status", "unknown"),
                "claimed_by": saga.get("claimed_by"),
                "interaction": existing["interaction"] if existing else "context",
                "interaction_at": existing["interaction_at"] if existing else now_iso(),
            }
            debug("parse_saga_from_output:", saga_id)
            return True
    except (json.JSONDecodeError, AttributeError):
        pass
    return False


def fetch_saga_info(state):
    """Fetch saga metadata via `sg context <id> --format json`."""
    ids = list(state["tracked_sagas"].keys())
    if not ids:
        return
    for saga_id in ids:
        try:
            result = subprocess.run(
                ["sg", "context", saga_id, "--format", "json"],
                capture_output=True, text=True, timeout=5,
            )
            if result.returncode == 0 and result.stdout.strip():
                data = json.loads(result.stdout)
                if data and data.get("saga"):
                    saga = data["saga"]
                    existing = state["tracked_sagas"].get(saga_id)
                    state["tracked_sagas"][saga_id] = {
                        "id": saga["id"],
                        "title": saga.get("title", ""),
                        "status": saga.get("status", "unknown"),
                        "claimed_by": saga.get("claimed_by"),
                        "interaction": existing["interaction"] if existing else None,
                        "interaction_at": existing["interaction_at"] if existing else None,
                    }
        except Exception as e:
            debug("fetch_saga_info error for", saga_id, ":", e)


def refresh_sagas(state):
    """Rate-limited saga metadata refresh."""
    now = datetime.now(timezone.utc).timestamp()
    if not state["saga_refresh_needed"] and (now - state["last_saga_refresh"]) < SAGA_REFRESH_INTERVAL:
        return
    state["saga_refresh_needed"] = False
    state["last_saga_refresh"] = now
    fetch_saga_info(state)


# ── File Extraction ────────────────────────────────────────────────────────

def extract_file_from_tool(tool_name, tool_input):
    """Extract file path from tool input for current_file tracking."""
    if not tool_input or not isinstance(tool_input, dict):
        return None
    if tool_name in ("Edit", "Write", "Read"):
        return tool_input.get("file_path") or tool_input.get("filePath")
    if tool_name == "apply_patch":
        # Try common field names
        for key in ("file_path", "filePath", "path"):
            if tool_input.get(key):
                return tool_input[key]
        # Parse diff headers from command/patch content
        command = tool_input.get("command", "")
        if isinstance(command, str):
            for line in command.split("\n"):
                if line.startswith("+++ b/"):
                    return line[6:]
                if line.startswith("+++ "):
                    return line[4:].strip()
    return None


# ── Event Handlers ─────────────────────────────────────────────────────────

def handle_session_start(state, hook):
    state["working_dir"] = hook.get("cwd")
    state["current_session_id"] = hook.get("session_id")
    # Resolve session label from session_index.jsonl
    label = resolve_session_label(hook.get("session_id"))
    if label:
        state["current_label"] = label
        debug("session label:", label)
    state["current_activity"] = "thinking"
    write_signal(state, "running")
    debug("SessionStart:", hook.get("source"), "session:", hook.get("session_id"))


def handle_user_prompt_submit(state, hook):
    prompt = hook.get("prompt", "")
    if prompt:
        state["current_task"] = prompt[:80]
    state["current_activity"] = "thinking"
    write_signal(state, "running")
    debug("UserPromptSubmit:", prompt[:80] if prompt else "(empty)")


def handle_pre_tool_use(state, hook):
    tool_name = hook.get("tool_name", "")
    tool_input = hook.get("tool_input", {})

    state["current_tool"] = tool_name
    state["current_activity"] = TOOL_ACTIVITY.get(tool_name, "thinking")

    # Bash command extraction + saga detection
    if tool_name == "Bash" and isinstance(tool_input, dict):
        command = tool_input.get("command", "")
        if command:
            state["last_bash_command"] = command
            found = extract_saga_ids(state, command)
            if found:
                state["saga_refresh_needed"] = True
            debug("bash before:", command[:120])

    # File tracking
    file_path = extract_file_from_tool(tool_name, tool_input)
    if file_path:
        state["current_file"] = file_path
        wt = find_worktree(file_path)
        if wt:
            state["current_worktree"] = wt

    write_signal(state, "running")
    debug("PreToolUse:", tool_name)


def handle_permission_request(state, hook):
    state["permission_pending"] = True
    state["current_activity"] = "waiting"
    write_signal(state, "waiting_input")
    debug("PermissionRequest:", hook.get("tool_name"))


def handle_post_tool_use(state, hook):
    tool_name = hook.get("tool_name", "")
    tool_input = hook.get("tool_input", {})
    tool_response = hook.get("tool_response", {})

    # Clear tool state
    if state["current_tool"] == tool_name:
        state["current_tool"] = None

    # Clear permission pending
    state["permission_pending"] = False

    # Bash output parsing
    if tool_name == "Bash":
        cmd = state.get("last_bash_command")
        state["last_bash_command"] = None

        # Re-extract saga IDs (catches any missed in before-hook)
        if cmd:
            found = extract_saga_ids(state, cmd)
            if found:
                state["saga_refresh_needed"] = True
            # Check for sg new output
            if SAGA_NEW_PATTERN.search(cmd):
                output_text = get_response_text(tool_response)
                if output_text:
                    extract_saga_id_from_output(state, output_text)
                state["saga_refresh_needed"] = True

        # Try parsing saga JSON from bash output
        output_text = get_response_text(tool_response)
        if output_text:
            parsed = parse_saga_from_output(state, output_text)
            if parsed:
                state["saga_refresh_needed"] = True

    # Reset activity
    state["current_activity"] = (
        TOOL_ACTIVITY.get(state["current_tool"], "thinking")
        if state["current_tool"]
        else "thinking"
    )

    write_signal(state, "running")
    debug("PostToolUse:", tool_name)


def get_response_text(tool_response):
    """Extract text from tool_response, handling various formats."""
    if isinstance(tool_response, str):
        return tool_response
    if isinstance(tool_response, dict):
        return (
            tool_response.get("output")
            or tool_response.get("stdout")
            or tool_response.get("content")
            or ""
        )
    return ""


def handle_subagent_start(state, hook):
    agent_id = hook.get("agent_id", "")
    agent_type = hook.get("agent_type", "subagent")
    if agent_id:
        state["tracked_subagents"][agent_id] = {
            "id": agent_id,
            "name": agent_type or "subagent",
            "prompt": None,
            "description": None,
            "status": "running",
            "activity": "thinking",
            "tool_executing": None,
            "last_update": now_iso(),
        }
        debug("SubagentStart:", agent_id, agent_type)
    write_signal(state)


def handle_subagent_stop(state, hook):
    agent_id = hook.get("agent_id", "")
    if agent_id and agent_id in state["tracked_subagents"]:
        del state["tracked_subagents"][agent_id]
        debug("SubagentStop:", agent_id)
    write_signal(state)


def handle_stop(state, hook):
    state["current_tool"] = None
    state["current_activity"] = None
    state["permission_pending"] = False
    write_signal(state, "idle")
    debug("Stop")


# ── Main ───────────────────────────────────────────────────────────────────

EVENT_HANDLERS = {
    "SessionStart": handle_session_start,
    "UserPromptSubmit": handle_user_prompt_submit,
    "PreToolUse": handle_pre_tool_use,
    "PermissionRequest": handle_permission_request,
    "PostToolUse": handle_post_tool_use,
    "SubagentStart": handle_subagent_start,
    "SubagentStop": handle_subagent_stop,
    "Stop": handle_stop,
}


def main():
    # Read hook input from stdin
    try:
        raw = sys.stdin.read()
        hook = json.loads(raw) if raw.strip() else {}
    except json.JSONDecodeError as e:
        debug("stdin parse error:", e)
        sys.exit(0)

    event_name = hook.get("hook_event_name", "")

    # Get pane ID from TMUX_PANE
    pane_id = os.environ.get("TMUX_PANE")
    if not pane_id:
        debug("No TMUX_PANE, skipping hook:", event_name)
        sys.exit(0)

    # Load state
    state = load_state(pane_id)
    state["pane_id"] = pane_id

    # Update working_dir if cwd is provided
    if hook.get("cwd"):
        state["working_dir"] = hook["cwd"]

    # Dispatch to handler
    handler = EVENT_HANDLERS.get(event_name)
    if handler:
        try:
            handler(state, hook)
        except Exception as e:
            debug(f"handler error ({event_name}):", e)
    else:
        debug("unhandled event:", event_name)

    # Save state
    save_state(state)

    sys.exit(0)


if __name__ == "__main__":
    main()
