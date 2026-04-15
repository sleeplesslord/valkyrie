import { writeFile, mkdir, unlink } from "fs/promises";
import { homedir } from "os";
import { join, dirname } from "path";
import { exec } from "child_process";
import { promisify } from "util";

const execAsync = promisify(exec);
const SIGNAL_DIR = join(homedir(), ".agent-sidebar", "agents");

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

  async function writeSignal(status) {
    lastStatus = status || lastStatus;
    try {
      await mkdir(SIGNAL_DIR, { recursive: true });
      await writeFile(signalPath, JSON.stringify({
        version: 1,
        agent_type: "opencode",
        status: lastStatus,
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
            await writeSignal("running");
          } else {
            await writeSignal("idle");
          }
          break;
          
        case "session.idle":
          await writeSignal("idle");
          break;
          
        case "session.error":
          await writeSignal("error");
          break;
          
        case "permission.updated":
          await writeSignal("waiting_input");
          break;
          
        case "file.edited":
          currentFile = event.properties.file;
          currentWorktree = await findWorktree(currentFile);
          await writeSignal();
          break;
      }
    },
  };
}
