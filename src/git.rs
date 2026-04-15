use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DiffStats {
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
}

impl std::fmt::Display for DiffStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.files_changed == 0 {
            write!(f, "")
        } else {
            write!(f, "+{} -{}", self.insertions, self.deletions)
        }
    }
}

pub fn get_diff_stats(working_dir: &str) -> Option<DiffStats> {
    let path = Path::new(working_dir);
    
    if !path.join(".git").exists() {
        return None;
    }

    let output = Command::new("git")
        .args(["diff", "--shortstat"])
        .current_dir(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_shortstat(&stdout)
}

fn parse_shortstat(s: &str) -> Option<DiffStats> {
    let s = s.trim();
    if s.is_empty() {
        return Some(DiffStats::default());
    }

    let mut stats = DiffStats::default();
    
    let parts: Vec<&str> = s.split(',').collect();
    for part in parts {
        let part = part.trim();
        if part.contains("file") {
            stats.files_changed = part.split_whitespace().next()?.parse().ok()?;
        } else if part.contains("insertion") {
            stats.insertions = part.split_whitespace().next()?.parse().ok()?;
        } else if part.contains("deletion") {
            stats.deletions = part.split_whitespace().next()?.parse().ok()?;
        }
    }

    Some(stats)
}

pub fn is_git_repo(working_dir: &str) -> bool {
    Path::new(working_dir).join(".git").exists()
}

pub fn get_diff(working_dir: &str) -> Option<String> {
    let path = Path::new(working_dir);
    
    if !path.join(".git").exists() {
        return None;
    }

    let output = Command::new("git")
        .args(["diff", "--color=never"])
        .current_dir(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let diff = String::from_utf8_lossy(&output.stdout);
    if diff.is_empty() {
        Some("No uncommitted changes".to_string())
    } else {
        Some(diff.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_shortstat_empty() {
        assert_eq!(parse_shortstat(""), Some(DiffStats::default()));
        assert_eq!(parse_shortstat("  "), Some(DiffStats::default()));
    }

    #[test]
    fn test_parse_shortstat_full() {
        let input = " 3 files changed, 42 insertions(+), 10 deletions(-)";
        let stats = parse_shortstat(input).unwrap();
        assert_eq!(stats.files_changed, 3);
        assert_eq!(stats.insertions, 42);
        assert_eq!(stats.deletions, 10);
    }

    #[test]
    fn test_parse_shortstat_only_insertions() {
        let input = " 1 file changed, 5 insertions(+)";
        let stats = parse_shortstat(input).unwrap();
        assert_eq!(stats.files_changed, 1);
        assert_eq!(stats.insertions, 5);
        assert_eq!(stats.deletions, 0);
    }
}
