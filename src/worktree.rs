use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub relative: String,
    pub branch: Option<String>,
    pub head: Option<String>,
}

pub struct WorktreeCache {
    worktrees: HashMap<PathBuf, WorktreeInfo>,
    root: Option<PathBuf>,
}

impl WorktreeCache {
    pub fn new() -> Self {
        Self {
            worktrees: HashMap::new(),
            root: None,
        }
    }

    pub fn set_root(&mut self, root: PathBuf) {
        self.root = Some(root);
        self.refresh();
    }

    pub fn refresh(&mut self) {
        if let Some(root) = &self.root {
            self.worktrees = list_worktrees(root)
                .into_iter()
                .map(|wt| (wt.path.clone(), wt))
                .collect();
        }
    }

    pub fn find_worktree(&self, working_dir: &str) -> Option<&WorktreeInfo> {
        let path = PathBuf::from(working_dir);
        
        for (wt_path, info) in &self.worktrees {
            if path.starts_with(wt_path) {
                return Some(info);
            }
        }
        None
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }
}

impl Default for WorktreeCache {
    fn default() -> Self {
        Self::new()
    }
}

pub fn list_worktrees(root: &Path) -> Vec<WorktreeInfo> {
    let output = match Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(root)
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    if !output.status.success() {
        return Vec::new();
    }

    parse_worktree_list(&String::from_utf8_lossy(&output.stdout), root)
}

fn parse_worktree_list(output: &str, root: &Path) -> Vec<WorktreeInfo> {
    let mut worktrees = Vec::new();
    let mut current: Option<WorktreeInfo> = None;

    for line in output.lines() {
        if line.starts_with("worktree ") {
            if let Some(wt) = current.take() {
                worktrees.push(wt);
            }
            let path = line.strip_prefix("worktree ").unwrap_or("");
            let path = PathBuf::from(path);
            let relative = path.strip_prefix(root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| path.display().to_string());
            
            current = Some(WorktreeInfo {
                path,
                relative,
                branch: None,
                head: None,
            });
        } else if line.starts_with("branch ") {
            if let Some(ref mut wt) = current {
                wt.branch = Some(line.strip_prefix("branch ").unwrap_or("").to_string());
            }
        } else if line.starts_with("HEAD ") {
            if let Some(ref mut wt) = current {
                wt.head = Some(line.strip_prefix("HEAD ").unwrap_or("").to_string());
            }
        }
    }

    if let Some(wt) = current {
        worktrees.push(wt);
    }

    worktrees
}

pub fn detect_worktree(working_dir: &str, root: &Path) -> Option<WorktreeInfo> {
    let path = PathBuf::from(working_dir);
    
    if !path.join(".git").exists() {
        return None;
    }
    
    let git_file = path.join(".git");
    if git_file.is_file() {
        if let Ok(content) = std::fs::read_to_string(&git_file) {
            if content.starts_with("gitdir:") {
                let relative = path.strip_prefix(root)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| path.display().to_string());
                
                return Some(WorktreeInfo {
                    path,
                    relative,
                    branch: None,
                    head: None,
                });
            }
        }
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_worktree_list() {
        let output = "worktree /home/user/project
HEAD abc123
branch main

worktree /home/user/project/.worktrees/feature-auth
HEAD def456
branch feature-auth
";
        let root = PathBuf::from("/home/user/project");
        let worktrees = parse_worktree_list(output, &root);
        
        assert_eq!(worktrees.len(), 2);
        assert_eq!(worktrees[0].relative, "");
        assert_eq!(worktrees[1].relative, ".worktrees/feature-auth");
        assert_eq!(worktrees[1].branch, Some("feature-auth".to_string()));
    }
}
