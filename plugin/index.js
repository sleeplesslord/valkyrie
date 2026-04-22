import { writeFile, mkdir, unlink } from "fs/promises";
import { homedir } from "os";
import { join, dirname } from "path";
import { exec } from "child_process";
import { promisify } from "util";

const execAsync = promisify(exec);
const SIGNAL_DIR = join(homedir(), ".agent-sidebar", "agents");

const TOOL_ACTIVITY = {
  read: "exploring",
  glob: "exploring",
  grep: "exploring",
  edit: "coding",
  write: "coding",
  bash: "running",
  webfetch: "researching",
};

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

export default async function AgentSidebarPlugin(ctx) {
  const paneId = process.env.TMUX_PANE;
  
  if (!paneId) {
    return {};
  }

  const signalPath = join(SIGNAL_DIR, `${paneId}.json`);
  let currentFile = null;
  let currentWorktree = null;
  let lastStatus = "idle";
  let currentActivity = null;
  let currentTool = null;
  let currentTask = null;

  async function writeSignal(status) {
    lastStatus = status || lastStatus;
    try {
      await mkdir(SIGNAL_DIR, { recursive: true });
      await writeFile(signalPath, JSON.stringify({
        version: 1,
        agent_type: "opencode",
        status: lastStatus,
        activity: currentActivity,
        tool_executing: currentTool,
        task: currentTask,
        working_dir: ctx.directory,
        worktree: currentWorktree,
        current_file: currentFile,
        last_update: new Date().toISOString(),
      }, null, 2));
    } catch (err) {
      console.error("[agent-sidebar plugin] Failed to write signal:", err.message);
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

  return {
    async event({ event }) {
      switch (event.type) {
        case "session.status":
          if (event.properties.status.type === "busy") {
            currentActivity = currentActivity || "thinking";
            await writeSignal("running");
          } else {
            currentActivity = null;
            currentTool = null;
            await writeSignal("idle");
          }
          break;
          
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
      await writeSignal();
    },

    "tool.execute.after": async (input) => {
      if (currentTool === input.tool) {
        currentTool = null;
      }
      currentActivity = currentTool ? (TOOL_ACTIVITY[currentTool] || "thinking") : "thinking";
      await writeSignal();
    },
  };
}
