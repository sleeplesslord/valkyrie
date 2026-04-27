use anyhow::Result;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct PaneInfo {
    pub session_name: String,
    pub window_id: String,
    pub pane_id: String,
    pub pane_title: String,
    pub current_command: String,
    pub current_path: String,
    #[allow(dead_code)]
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct Tmux;

#[allow(dead_code)]
impl Tmux {
    pub fn new() -> Self {
        Self
    }

    pub fn is_in_tmux() -> bool {
        std::env::var("TMUX").is_ok()
    }

    pub fn kill_pane(pane_id: &str) -> Result<()> {
        let status = Command::new("tmux")
            .args(["kill-pane", "-t", pane_id])
            .status()?;
        if !status.success() {
            anyhow::bail!("Failed to kill pane {}", pane_id);
        }
        Ok(())
    }

    pub fn current_pane_id() -> Option<String> {
        std::env::var("TMUX_PANE").ok()
    }

    pub fn list_panes(&self) -> Result<Vec<PaneInfo>> {
        let output = Command::new("tmux")
            .args([
                "list-panes",
                "-a",
                "-F",
                "#{session_name}:#{window_id}:#{pane_id}|#{pane_title}|#{pane_current_command}|#{pane_current_path}|#{pane_active}",
            ])
            .output()?;

        if !output.status.success() {
            anyhow::bail!(
                "tmux list-panes failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut panes = Vec::new();

        for line in stdout.lines() {
            if let Some(pane) = self.parse_pane_line(line) {
                panes.push(pane);
            }
        }

        Ok(panes)
    }

    fn parse_pane_line(&self, line: &str) -> Option<PaneInfo> {
        let parts: Vec<&str> = line.splitn(5, '|').collect();
        if parts.len() != 5 {
            return None;
        }

        let ids: Vec<&str> = parts[0].split(':').collect();
        if ids.len() != 3 {
            return None;
        }

        Some(PaneInfo {
            session_name: ids[0].to_string(),
            window_id: ids[1].to_string(),
            pane_id: ids[2].to_string(),
            pane_title: parts[1].to_string(),
            current_command: parts[2].to_string(),
            current_path: parts[3].to_string(),
            is_active: parts[4] == "1",
        })
    }

    pub fn select_pane(&self, session: &str, window: &str, pane: &str) -> Result<()> {
        let target = format!("{}:{}.{}", session, window, pane);
        let status = Command::new("tmux")
            .args(["select-pane", "-t", &target])
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to select pane {}", target);
        }
        Ok(())
    }

    pub fn rename_pane(&self, pane_id: &str, title: &str) -> Result<()> {
        let status = Command::new("tmux")
            .args(["select-pane", "-t", pane_id, "-T", title])
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to rename pane {}", pane_id);
        }
        Ok(())
    }

    pub fn rename_window(&self, window_id: &str, name: &str) -> Result<()> {
        let status = Command::new("tmux")
            .args(["rename-window", "-t", window_id, name])
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to rename window {}", window_id);
        }
        Ok(())
    }

    pub fn get_current_window(&self) -> Result<String> {
        let output = Command::new("tmux")
            .args(["display-message", "-p", "#{window_id}"])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to get current window");
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn get_current_session(&self) -> Result<String> {
        let output = Command::new("tmux")
            .args(["display-message", "-p", "#{session_name}"])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to get current session");
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn pane_exists(&self, pane_id: &str) -> bool {
        Command::new("tmux")
            .args(["list-panes", "-a", "-F", "#{pane_id}"])
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .any(|l| l == pane_id)
            })
            .unwrap_or(false)
    }

    pub fn get_pane_window(&self, pane_id: &str) -> Option<String> {
        let output = Command::new("tmux")
            .args(["list-panes", "-a", "-F", "#{pane_id}:#{window_id}"])
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() == 2 && parts[0] == pane_id {
                return Some(parts[1].to_string());
            }
        }
        None
    }

    pub fn break_pane(&self, pane_id: &str, target_window: &str) -> Result<()> {
        let status = Command::new("tmux")
            .args(["break-pane", "-s", pane_id, "-t", target_window])
            .status()?;

        if !status.success() {
            anyhow::bail!(
                "Failed to break pane {} to window {}",
                pane_id,
                target_window
            );
        }
        Ok(())
    }

    pub fn join_pane(&self, pane_id: &str, target_window: &str, width: u16) -> Result<()> {
        let status = Command::new("tmux")
            .args([
                "join-pane",
                "-hb",
                "-l",
                &width.to_string(),
                "-s",
                pane_id,
                "-t",
                target_window,
            ])
            .status()?;

        if !status.success() {
            anyhow::bail!(
                "Failed to join pane {} to window {}",
                pane_id,
                target_window
            );
        }
        Ok(())
    }

    pub fn new_window(&self, name: &str) -> Result<String> {
        let output = Command::new("tmux")
            .args(["new-window", "-P", "-F", "#{window_id}", "-n", name])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to create new window");
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn run_in_window(&self, name: &str, command: &str) -> Result<String> {
        let output = Command::new("tmux")
            .args([
                "new-window",
                "-P",
                "-F",
                "#{window_id}",
                "-n",
                name,
                "--",
                command,
            ])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Failed to create window for command: {}", command);
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn new_window_cwd(&self, name: &str, cwd: &str) -> Result<String> {
        let output = Command::new("tmux")
            .args([
                "new-window",
                "-P",
                "-F",
                "#{window_id}",
                "-n",
                name,
                "-c",
                cwd,
            ])
            .output()?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to create new window with cwd={}: {}",
                cwd,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn select_window(&self, window_id: &str) -> Result<()> {
        let status = Command::new("tmux")
            .args(["select-window", "-t", window_id])
            .status()?;

        if !status.success() {
            anyhow::bail!("Failed to select window {}", window_id);
        }
        Ok(())
    }
}

impl Default for Tmux {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pane_line() {
        let tmux = Tmux::new();
        let line = "main:@0:%0|zsh|zsh|/home/user/project|1";
        let pane = tmux.parse_pane_line(line).unwrap();

        assert_eq!(pane.session_name, "main");
        assert_eq!(pane.window_id, "@0");
        assert_eq!(pane.pane_id, "%0");
        assert_eq!(pane.pane_title, "zsh");
        assert_eq!(pane.current_command, "zsh");
        assert_eq!(pane.current_path, "/home/user/project");
        assert!(pane.is_active);
    }

    #[test]
    fn test_parse_pane_line_inactive() {
        let tmux = Tmux::new();
        let line = "main:@1:%1|valkyrie|valkyrie|/home/user/project|0";
        let pane = tmux.parse_pane_line(line).unwrap();

        assert!(!pane.is_active);
    }
}
