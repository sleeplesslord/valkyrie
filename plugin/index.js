import { writeFile, mkdir, unlink } from "fs/promises";
import { homedir } from "os";
import { join, dirname } from "path";
import { exec } from "child_process";
import { promisify } from "util";

const execAsync = promisify(exec);
const SIGNAL_DIR = join(homedir(), ".valkyrie", "agents");

const TOOL_ACTIVITY = {
  read: "exploring",
  glob: "exploring",
  grep: "exploring",
  edit: "coding",
  write: "coding",
  bash: "running",
  webfetch: "researching",
};

const SAGA_ID_PATTERN = /\bsg\s+(?:claim|done|log|context|edit|label|priority|depend|relate|unclaim)\s+([\w.]+)/g;
const SAGA_NEW_PATTERN = /\bsg\s+new\b/g;
const SAGA_CLAIM_PATTERN = /\bsg\s+claim\s+([\w.]+)/g;

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

async function fetchSagaInfo(sagaIds) {
  const sagas = [];
  for (const id of sagaIds) {
    try {
      const { stdout } = await execAsync(
        `sg context ${id} --format json 2>/dev/null`
      );
      const data = JSON.parse(stdout);
      if (data && data.saga) {
        sagas.push({
          id: data.saga.id,
          title: data.saga.title || "",
          status: data.saga.status || "unknown",
        });
      }
    } catch {}
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
  const trackedSagas = new Map();
  let sagaRefreshNeeded = false;
  let lastSagaRefresh = 0;
  const SAGA_REFRESH_INTERVAL = 10000;

  async function refreshSagas() {
    const now = Date.now();
    if (!sagaRefreshNeeded && now - lastSagaRefresh < SAGA_REFRESH_INTERVAL) {
      return;
    }
    sagaRefreshNeeded = false;
    lastSagaRefresh = now;
    const ids = [...trackedSagas.keys()];
    if (ids.length === 0) return;
    const sagas = await fetchSagaInfo(ids);
    for (const s of sagas) {
      trackedSagas.set(s.id, s);
    }
    for (const id of ids) {
      if (!trackedSagas.has(id)) {
        trackedSagas.delete(id);
      }
    }
  }

  function extractSagaIds(text) {
    let found = false;
    let m;
    SAGA_ID_PATTERN.lastIndex = 0;
    while ((m = SAGA_ID_PATTERN.exec(text)) !== null) {
      trackedSagas.set(m[1], { id: m[1], title: "", status: "unknown" });
      found = true;
    }
    SAGA_CLAIM_PATTERN.lastIndex = 0;
    while ((m = SAGA_CLAIM_PATTERN.exec(text)) !== null) {
      trackedSagas.set(m[1], { id: m[1], title: "", status: "unknown" });
      found = true;
    }
    return found;
  }

  function extractSagaIdFromOutput(text) {
    const idPattern = /(?:created|saga|Saga)\s+(?:saga\s+)?([a-z0-9]{5,}(?:\.\d+)?)/i;
    const m = text.match(idPattern);
    if (m) {
      trackedSagas.set(m[1], { id: m[1], title: "", status: "unknown" });
      return true;
    }
    return false;
  }

  async function writeSignal(status) {
    lastStatus = status || lastStatus;
    try {
      await mkdir(SIGNAL_DIR, { recursive: true });
      await refreshSagas();
      const sagas = [...trackedSagas.values()].slice(0, 10);
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

  async function fetchSessionLabel() {
    try {
      const result = await Promise.race([
        ctx.client.session.list(),
        new Promise((_, reject) => setTimeout(() => reject(new Error("timeout")), 5000)),
      ]);
      const sessions = result.data;
      if (sessions && sessions.length > 0) {
        const latest = sessions[sessions.length - 1];
        if (latest.title && latest.title !== currentLabel) {
          currentLabel = latest.title;
          await writeSignal();
        }
      }
    } catch {}
  }

  fetchSessionLabel();
  await writeSignal("idle");

  const HEARTBEAT_INTERVAL = 15000;
  setInterval(() => writeSignal(), HEARTBEAT_INTERVAL);

  return {
    async event({ event }) {
      switch (event.type) {
        case "session.status":
          if (event.properties.status.type === "busy") {
            currentActivity = currentActivity || "thinking";
            await writeSignal("running");
          } else {
            currentActivity = null;
            await writeSignal("idle");
          }
          break;
          
        case "session.updated": {
          const session = event.properties;
          if (session && session.title && session.title !== currentLabel) {
            currentLabel = session.title;
            await writeSignal();
          }
          break;
        }

        case "session.idle":
          currentActivity = null;
          currentTool = null;
          await writeSignal("idle");
          break;
          
        case "session.error":
          currentActivity = null;
          currentTool = null;
          await writeSignal("error");
          break;
          
        case "permission.asked":
          currentActivity = "waiting";
          await writeSignal("waiting_input");
          break;

        case "permission.updated":
          if (lastStatus === "waiting_input") {
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
      }
    },

    "tool.execute.before": async (input, output) => {
      currentTool = input.tool;
      currentActivity = TOOL_ACTIVITY[input.tool] || "thinking";
      if (input.tool === "bash" && output.args?.command) {
        const found = extractSagaIds(output.args.command);
        if (found) sagaRefreshNeeded = true;
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
      if (input.tool === "bash" && input.args?.command) {
        SAGA_NEW_PATTERN.lastIndex = 0;
        if (SAGA_NEW_PATTERN.test(input.args.command)) {
          if (output.output) {
            extractSagaIdFromOutput(output.output);
          }
          sagaRefreshNeeded = true;
        }
      }
      if (currentTool === input.tool) {
        currentTool = null;
      }
      currentActivity = currentTool ? (TOOL_ACTIVITY[currentTool] || "thinking") : "thinking";
      await writeSignal();
    },
  };
}
