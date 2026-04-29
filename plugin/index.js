import { writeFile, mkdir, unlink } from "fs/promises";
import { homedir } from "os";
import { join, dirname } from "path";
import { exec } from "child_process";
import { promisify } from "util";

const execAsync = promisify(exec);
const SIGNAL_DIR = join(homedir(), ".valkyrie", "agents");
const LOG_FILE = join(homedir(), ".valkyrie", "plugin.log");

const TOOL_ACTIVITY = {
  read: "exploring",
  glob: "exploring",
  grep: "exploring",
  edit: "coding",
  write: "coding",
  bash: "running",
  webfetch: "researching",
};

// Matches sg subcommands that take a saga ID as the first arg.
// Capture group 1 = subcommand, group 2 = saga ID.
// For multi-ID commands like "sg claim abc123 def456", the global +while
// loop will match the first ID, then extractSgIdsFromTail picks up the rest.
const SAGA_ID_PATTERN = /\bsg\s+(claim|done|log|context|edit|label|priority|depend|relate|unclaim|continue|reopen|wontdo)\s+([\w.-]+)/g;
const SAGA_NEW_PATTERN = /\bsg\s+new\b/g;

/// After the first regex match consumes "sg <cmd> <id>", any remaining IDs
/// on the same line are bare tokens. This extracts them until a flag (--xx) or
/// end of string. Only valid for multi-ID commands (claim, done, unclaim, wontdo).
/// Other commands like "log" have a message arg after the ID, which must NOT be
/// treated as saga IDs.
const MULTI_ID_COMMANDS = new Set(["claim", "done", "unclaim", "wontdo"]);

function extractSgIdsFromTail(text, firstMatchEnd, subcmd) {
  if (!MULTI_ID_COMMANDS.has(subcmd)) return [];
  const tail = text.slice(firstMatchEnd);
  const ids = [];
  for (const m of tail.matchAll(/\s+([\w.-]+)/g)) {
    if (m[1].startsWith("-")) break; // flag, stop
    ids.push(m[1]);
  }
  return ids;
}

async function findWorktree(filePath) {
  try {
    const dir = dirname(filePath);
    const { stdout } = await execAsync(
      `git -C "${dir}" rev-parse --show-toplevel 2>/dev/null || echo ""`
    );
    return stdout.trim() || null;
  } catch {
    return null;
  }
}

function extractFilePath(tool, args) {
  if (!args) return null;
  switch (tool) {
    case "read":
    case "edit":
      return args.filePath || null;
    case "write":
      return args.filePath || null;
    case "glob":
    case "grep":
      return args.path || args.include || null;
    default:
      return null;
  }
}

/// Debug logger — appends timestamped messages to ~/.valkyrie/plugin.log.
/// Keep calls lightweight; this is a dev diagnostic tool.
function debug(...args) {
  const ts = new Date().toISOString().slice(11, 19);
  const line = `[${ts}] ${args.join(" ")}\n`;
  // Fire-and-forget write — never await or block the main path
  import("fs/promises").then(fs => fs.appendFile(LOG_FILE, line).catch(() => {}));
}

async function fetchSagaInfo(sagaIds, trackedSagas) {
  const sagas = [];
  for (const id of sagaIds) {
    try {
      const { stdout } = await execAsync(
        `sg context ${id} --format json 2>/dev/null`
      );
      const data = JSON.parse(stdout);
      if (data && data.saga) {
        const existing = trackedSagas.get(id);
        sagas.push({
          id: data.saga.id,
          title: data.saga.title || "",
          status: data.saga.status || "unknown",
          claimed_by: data.saga.claimed_by || null,
          interaction: existing?.interaction || null,
        });
      }
    } catch (e) {
      debug("fetchSagaInfo failed for", id, ":", e?.message || e);
    }
  }
  return sagas;
}

export default async function AgentSidebarPlugin(ctx) {
  const paneId = process.env.TMUX_PANE;
  
  if (!paneId) {
    return {};
  }

  const signalPath = join(SIGNAL_DIR, `${paneId}.json`);
  let currentFile = null;
  let currentWorktree = ctx.worktree || null;
  let lastStatus = "idle";
  let currentActivity = null;
  let currentTool = null;
  let currentTask = null;
  let currentLabel = null;
  let currentSessionId = null;
  let lastBashCommand = null; // carried from before → after hook
  let permissionPending = false; // true while waiting for user approval
  const trackedSagas = new Map();
  let sagaRefreshNeeded = false;
  let lastSagaRefresh = 0;
  let lastLogMessage = null;
  const SAGA_REFRESH_INTERVAL = 10000;

  function extractSession(event) {
    const properties = event?.properties;
    if (properties?.info && typeof properties.info === "object") {
      return properties.info;
    }
    if (properties?.id) {
      return properties;
    }
    return null;
  }

  function extractSessionId(event) {
    return (
      event?.properties?.sessionID ||
      event?.properties?.info?.id ||
      event?.properties?.id ||
      null
    );
  }

  function setLabelFromSession(session) {
    if (!session?.id) return false;
    if (currentSessionId && session.id !== currentSessionId) return false;
    currentSessionId = session.id;
    const nextLabel = typeof session.title === "string" && session.title.trim()
      ? session.title
      : null;
    if (nextLabel === currentLabel) return false;
    currentLabel = nextLabel;
    return true;
  }

  async function refreshSessionLabelById(sessionId) {
    if (!sessionId) return false;
    try {
      const result = await Promise.race([
        ctx.client.session.get({ path: { id: sessionId } }),
        new Promise((_, reject) => setTimeout(() => reject(new Error("timeout")), 5000)),
      ]);
      const session = result?.data;
      if (!session || session.id !== sessionId) return false;
      return setLabelFromSession(session);
    } catch {
      return false;
    }
  }

  async function refreshSagas() {
    const now = Date.now();
    if (!sagaRefreshNeeded && now - lastSagaRefresh < SAGA_REFRESH_INTERVAL) {
      return;
    }
    sagaRefreshNeeded = false;
    lastSagaRefresh = now;
    const ids = [...trackedSagas.keys()];
    if (ids.length === 0) return;
    const sagas = await fetchSagaInfo(ids, trackedSagas);
    for (const s of sagas) {
      trackedSagas.set(s.id, s);
    }
    for (const id of ids) {
      if (!trackedSagas.has(id)) {
        trackedSagas.delete(id);
      }
    }
  }

  /// Split a command string on shell operators (&&, ||, ;, |, newlines)
  /// so each subcommand is parsed independently. This eliminates all
  /// tail-bleed edge cases from chained commands like
  /// "sg claim abc && sg context xyz".
  function splitShellCommands(text) {
    return text.split(/\s*(?:&&|\|\||[;|]|\n)\s*/).filter(Boolean);
  }

  function extractSagaIds(text) {
    let found = false;
    for (const segment of splitShellCommands(text)) {
      let m;
      SAGA_ID_PATTERN.lastIndex = 0;
      while ((m = SAGA_ID_PATTERN.exec(segment)) !== null) {
        const subcmd = m[1];
        const id = m[2];
        const existing = trackedSagas.get(id);
        trackedSagas.set(id, {
          id,
          title: existing?.title || "",
          status: existing?.status || "unknown",
          claimed_by: existing?.claimed_by || null,
          interaction: subcmd,
        });
        found = true;
        debug("extractSagaIds:", subcmd, id);
        // Capture log message from sg log commands
        if (subcmd === "log") {
          let tail = segment.slice(m.index + m[0].length).trim();
          let msg = null;
          if (tail.startsWith('"')) {
            const end = tail.indexOf('"', 1);
            msg = end > 0 ? tail.slice(1, end) : tail.slice(1);
          } else if (tail.startsWith("'")) {
            const end = tail.indexOf("'", 1);
            msg = end > 0 ? tail.slice(1, end) : tail.slice(1);
          } else if (tail && !tail.startsWith('-')) {
            msg = tail;
          }
          if (msg) {
            lastLogMessage = msg;
            debug("extractLogMessage:", msg);
          }
        }
        // Multi-ID commands (e.g. "sg claim abc def"): pick up trailing IDs
        const tailIds = extractSgIdsFromTail(segment, m.index + m[0].length, subcmd);
        for (const tid of tailIds) {
          const tex = trackedSagas.get(tid);
          trackedSagas.set(tid, {
            id: tid,
            title: tex?.title || "",
            status: tex?.status || "unknown",
            claimed_by: tex?.claimed_by || null,
            interaction: subcmd, // same interaction for all IDs in multi-id command
          });
          debug("extractSagaIds tail:", subcmd, tid);
        }
      }
    }
    return found;
  }

  function extractSagaIdFromOutput(text) {
    const idPattern = /(?:created|saga|Saga)\s+(?:saga\s+)?([a-z0-9]{5,}(?:\.\d+)?)/i;
    const m = text.match(idPattern);
    if (m) {
      const existing = trackedSagas.get(m[1]);
      trackedSagas.set(m[1], {
        id: m[1],
        title: existing?.title || "",
        status: existing?.status || "unknown",
        claimed_by: existing?.claimed_by || null,
        interaction: "new",
      });
      debug("extractSagaIdFromOutput: new", m[1]);
      return true;
    }
    return false;
  }

  /// Parse saga data directly from `sg context --format json` output
  /// embedded in tool output, avoiding a redundant sg context round-trip.
  function parseSagaFromToolOutput(toolOutput) {
    if (!toolOutput || typeof toolOutput !== "string") return false;
    let found = false;
    try {
      const data = JSON.parse(toolOutput);
      if (data?.saga?.id) {
        const existing = trackedSagas.get(data.saga.id);
        trackedSagas.set(data.saga.id, {
          id: data.saga.id,
          title: data.saga.title || "",
          status: data.saga.status || "unknown",
          claimed_by: data.saga.claimed_by || null,
          interaction: existing?.interaction || "context",
        });
        debug("parseSagaFromToolOutput:", data.saga.id);
        found = true;
      }
    } catch {
      // Not JSON — might be human-readable output or mixed. Fall through.
    }
    return found;
  }

  /// Extract the bash command string from tool args, handling
  /// different argument structures across opencode versions.
  function extractBashCommand(args) {
    if (!args) return null;
    // Standard structure: { command: "..." }
    if (typeof args.command === "string") return args.command;
    // Some versions nest under a different key
    if (typeof args.cmd === "string") return args.cmd;
    // Bash tool may pass command as the first positional arg
    if (typeof args.input === "string") return args.input;
    // Command might be in a script field
    if (typeof args.script === "string") return args.script;
    return null;
  }

  async function writeSignal(status) {
    lastStatus = status || lastStatus;
    try {
      await mkdir(SIGNAL_DIR, { recursive: true });
      await refreshSagas();
      const sagas = [...trackedSagas.values()].reverse().slice(0, 10);
      await writeFile(signalPath, JSON.stringify({
        version: 1,
        agent_type: "opencode",
        status: lastStatus,
        activity: currentActivity,
        tool_executing: currentTool,
        task: currentTask,
        label: currentLabel,
        working_dir: ctx.directory,
        worktree: currentWorktree,
        current_file: currentFile,
        last_update: new Date().toISOString(),
        sagas: sagas.length > 0 ? sagas : undefined,
        last_log: lastLogMessage || undefined,
      }, null, 2));
    } catch (err) {
      console.error("[valkyrie plugin] Failed to write signal:", err.message);
    }
  }

  async function cleanup() {
    try {
      await unlink(signalPath);
    } catch {}
  }

  process.on("exit", cleanup);
  process.on("SIGTERM", cleanup);
  process.on("SIGINT", cleanup);

  await writeSignal("idle");
  debug("plugin initialized, pane:", paneId);

  const HEARTBEAT_INTERVAL = 15000;
  setInterval(() => writeSignal(), HEARTBEAT_INTERVAL);

  return {
    async event({ event }) {
      switch (event.type) {
        case "session.status":
          if (event.properties?.sessionID && event.properties.sessionID !== currentSessionId) {
            currentSessionId = event.properties.sessionID;
            currentLabel = null;
          }
          if (event.properties?.sessionID && !currentLabel) {
            await refreshSessionLabelById(event.properties.sessionID);
          }
          if (event.properties.status.type === "busy") {
            currentActivity = currentActivity || "thinking";
            await writeSignal("running");
          } else {
            currentActivity = null;
            await writeSignal("idle");
          }
          break;
          
        case "session.created":
        case "session.updated": {
          const session = extractSession(event);
          if (setLabelFromSession(session)) {
            await writeSignal();
          }
          break;
        }

        case "session.idle": {
          const sessionId = extractSessionId(event);
          if (sessionId && sessionId !== currentSessionId) {
            currentSessionId = sessionId;
            currentLabel = null;
          }
          if (sessionId && !currentLabel) {
            await refreshSessionLabelById(sessionId);
          }
          currentActivity = null;
          currentTool = null;
          await writeSignal("idle");
          break;
        }

        case "session.error": {
          const sessionId = extractSessionId(event);
          if (sessionId && sessionId !== currentSessionId) {
            currentSessionId = sessionId;
            currentLabel = null;
          }
          currentActivity = null;
          currentTool = null;
          await writeSignal("error");
          break;
        }

        case "session.deleted": {
          const sessionId = extractSessionId(event);
          if (sessionId && sessionId === currentSessionId) {
            currentSessionId = null;
            currentLabel = null;
            await writeSignal();
          }
          break;
        }
          
        case "permission.asked":
          permissionPending = true;
          currentActivity = "waiting";
          await writeSignal("waiting_input");
          break;

        case "permission.updated":
          if (permissionPending) {
            permissionPending = false;
            currentActivity = currentTool ? (TOOL_ACTIVITY[currentTool] || "thinking") : "thinking";
            await writeSignal("running");
          }
          break;
          
        case "file.edited":
          currentFile = event.properties.file;
          currentWorktree = await findWorktree(currentFile);
          await writeSignal();
          break;

        case "message.updated": {
          const msg = event.properties;
          if (msg && msg.role === "user" && msg.content) {
            const text = typeof msg.content === "string"
              ? msg.content
              : Array.isArray(msg.content)
                ? msg.content.filter(p => p.type === "text").map(p => p.text).join(" ")
                : "";
            if (text) {
              currentTask = text.slice(0, 80);
              await writeSignal();
            }
          }
          break;
        }

        // command.executed fires when opencode's internal command system
        // runs a slash command or tool. This catches sg commands that might
        // not go through the bash tool (e.g., when opencode routes sg as
        // a first-class command rather than a shell invocation).
        case "command.executed": {
          const name = event.properties?.name;
          const args = event.properties?.arguments;
          debug("command.executed:", name, args?.slice(0, 100));
          // If the command is "sg" or starts with "sg", extract saga IDs
          const cmdText = [name, args].filter(Boolean).join(" ");
          if (cmdText) {
            const found = extractSagaIds(cmdText);
            if (found) sagaRefreshNeeded = true;
            // Also check for sg new output
            SAGA_NEW_PATTERN.lastIndex = 0;
            if (SAGA_NEW_PATTERN.test(cmdText)) {
              sagaRefreshNeeded = true;
            }
          }
          break;
        }
      }
    },

    "tool.execute.before": async (input, output) => {
      currentTool = input.tool;
      currentActivity = TOOL_ACTIVITY[input.tool] || "thinking";

      if (input.tool === "bash") {
        const cmd = extractBashCommand(output.args);
        if (cmd) {
          lastBashCommand = cmd;
          const hasChain = /[&|;]/.test(cmd);
          debug("bash before:", cmd.slice(0, 120), hasChain ? "[chained]" : "");
          const found = extractSagaIds(cmd);
          if (found) sagaRefreshNeeded = true;
        } else {
          // Log the actual arg structure so we can debug mismatches
          debug("bash before: no command found, args keys:", Object.keys(output.args || {}).join(","));
          debug("bash before: args sample:", JSON.stringify(output.args).slice(0, 200));
        }
      }

      const filePath = extractFilePath(input.tool, output.args);
      if (filePath) {
        currentFile = filePath;
        const wt = await findWorktree(filePath);
        if (wt) currentWorktree = wt;
      }
      await writeSignal();
    },

    "tool.execute.after": async (input, output) => {
      // input is {tool, sessionID, callID} — no args in after hook.
      // Use the command saved from the before hook.
      const cmd = input.tool === "bash" ? lastBashCommand : null;
      lastBashCommand = null;

      if (cmd) {
        const found = extractSagaIds(cmd);
        if (found) sagaRefreshNeeded = true;
        SAGA_NEW_PATTERN.lastIndex = 0;
        if (SAGA_NEW_PATTERN.test(cmd)) {
          if (output.output) {
            extractSagaIdFromOutput(output.output);
          }
          sagaRefreshNeeded = true;
        }
      }

      // Try to parse saga data directly from bash tool output.
      // When the agent runs `sg context <id> --format json`, the output
      // is the saga JSON — parse it instead of making a redundant sg call.
      if (input.tool === "bash" && output.output) {
        const parsed = parseSagaFromToolOutput(output.output);
        if (parsed) sagaRefreshNeeded = true;
      }

      if (currentTool === input.tool) {
        currentTool = null;
      }
      // Tool finished — permission is no longer pending (was auto-approved
      // or just completed). Clear the flag and ensure we're not stuck
      // in waiting_input.
      permissionPending = false;
      currentActivity = currentTool ? (TOOL_ACTIVITY[currentTool] || "thinking") : "thinking";
      await writeSignal("running");
    },
  };
}
