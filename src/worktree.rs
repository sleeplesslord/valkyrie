use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: Option<String>,
}

/// Caches git worktree information discovered from multiple project roots.
///
/// Each root is typically a git repository containing a main checkout
/// plus `.worktrees/` subdirectories. The cache maps any working directory
/// to its containing worktree using longest-prefix matching.
pub struct WorktreeCache {
    worktrees: HashMap<PathBuf, WorktreeInfo>,
    roots: Vec<PathBuf>,
}

impl WorktreeCache {
    pub fn new() -> Self {
        Self {
            worktrees: HashMap::new(),
            roots: Vec::new(),
        }
    }

    /// Set multiple project roots and refresh the cache.
    /// Each root is queried via `git worktree list`.
    pub fn set_roots(&mut self, roots: Vec<PathBuf>) {
        self.roots = roots;
        self.refresh();
    }

    /// Add a single project root and refresh.
    /// No-op if the root is already known.
    pub fn add_root(&mut self, root: PathBuf) {
        if !self.roots.contains(&root) {
            self.roots.push(root);
            self.refresh();
        }
    }

    /// Re-read `git worktree list` for all known roots.
    pub fn refresh(&mut self) {
        self.worktrees.clear();
        for root in &self.roots {
            for wt in list_worktrees(root) {
                self.worktrees.insert(wt.path.clone(), wt);
            }
        }
    }

    /// Find the worktree containing `working_dir` using longest-prefix match.
    ///
    /// Prefers the most specific (deepest) worktree so that nested worktrees
    /// win over the root. E.g. `/proj/.worktrees/feat` beats `/proj` when
    /// the working_dir is inside `.worktrees/feat/`.
    pub fn find_worktree(&self, working_dir: &str) -> Option<&WorktreeInfo> {
        let path = PathBuf::from(working_dir);

        let mut best: Option<(&PathBuf, &WorktreeInfo)> = None;

        for (wt_path, info) in &self.worktrees {
            if path.starts_with(wt_path) {
                let is_better = best
                    .map(|(best_path, _)| {
                        wt_path.components().count() > best_path.components().count()
                    })
                    .unwrap_or(true);
                if is_better {
                    best = Some((wt_path, info));
                }
            }
        }

        best.map(|(_, info)| info)
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

fn parse_worktree_list(output: &str, _root: &Path) -> Vec<WorktreeInfo> {
    let mut worktrees = Vec::new();
    let mut current: Option<WorktreeInfo> = None;

    for line in output.lines() {
        if line.starts_with("worktree ") {
            if let Some(wt) = current.take() {
                worktrees.push(wt);
            }
            let path = line.strip_prefix("worktree ").unwrap_or("");
            let path = PathBuf::from(path);

            current = Some(WorktreeInfo {
                path,
                branch: None,
            });
        } else if line.starts_with("branch ") {
            if let Some(ref mut wt) = current {
                wt.branch = Some(line.strip_prefix("branch ").unwrap_or("").to_string());
            }
        }
        // HEAD lines are ignored — display labels come from trim_display_path
    }

    if let Some(wt) = current {
        worktrees.push(wt);
    }

    worktrees
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
        assert_eq!(worktrees[0].path, PathBuf::from("/home/user/project"));
        assert_eq!(worktrees[0].branch, Some("main".to_string()));
        assert_eq!(
            worktrees[1].path,
            PathBuf::from("/home/user/project/.worktrees/feature-auth")
        );
        assert_eq!(worktrees[1].branch, Some("feature-auth".to_string()));
    }

    #[test]
    fn test_find_worktree_longest_prefix_match() {
        let mut cache = WorktreeCache::new();
        cache.roots = vec![PathBuf::from("/home/user/project")];
        cache.worktrees = vec![
            (
                PathBuf::from("/home/user/project"),
                WorktreeInfo {
                    path: PathBuf::from("/home/user/project"),
                    branch: Some("main".to_string()),
                },
            ),
            (
                PathBuf::from("/home/user/project/.worktrees/feature-auth"),
                WorktreeInfo {
                    path: PathBuf::from("/home/user/project/.worktrees/feature-auth"),
                    branch: Some("feature-auth".to_string()),
                },
            ),
        ]
        .into_iter()
        .collect();

        // The project root itself matches only the root worktree.
        let root_match = cache.find_worktree("/home/user/project");
        assert!(root_match.is_some());
        assert_eq!(root_match.unwrap().path, PathBuf::from("/home/user/project"));

        // A path inside a specific worktree should match that worktree, not the root.
        let feature_match = cache.find_worktree("/home/user/project/.worktrees/feature-auth/src");
        assert!(feature_match.is_some());
        assert_eq!(
            feature_match.unwrap().path,
            PathBuf::from("/home/user/project/.worktrees/feature-auth")
        );
    }

    #[test]
    fn test_multiple_roots() {
        let mut cache = WorktreeCache::new();
        cache.roots = vec![
            PathBuf::from("/home/user/proj-a"),
            PathBuf::from("/home/user/proj-b"),
        ];
        cache.worktrees = vec![
            (
                PathBuf::from("/home/user/proj-a"),
                WorktreeInfo {
                    path: PathBuf::from("/home/user/proj-a"),
                    branch: Some("main".to_string()),
                },
            ),
            (
                PathBuf::from("/home/user/proj-b/.worktrees/feat"),
                WorktreeInfo {
                    path: PathBuf::from("/home/user/proj-b/.worktrees/feat"),
                    branch: Some("feat".to_string()),
                },
            ),
        ]
        .into_iter()
        .collect();

        assert_eq!(
            cache.find_worktree("/home/user/proj-a").unwrap().path,
            PathBuf::from("/home/user/proj-a")
        );
        assert_eq!(
            cache
                .find_worktree("/home/user/proj-b/.worktrees/feat/src")
                .unwrap()
                .path,
            PathBuf::from("/home/user/proj-b/.worktrees/feat")
        );
        assert!(cache.find_worktree("/home/user/other-project").is_none());
    }
}
