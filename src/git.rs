use std::path::Path;
use std::process::Command;
use crate::types::{RepoInfo, RepoStatus};

fn git(path: &Path, args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(["-C", path.to_str()?])
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn git_any(path: &Path, args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(["-C", path.to_str()?])
        .args(args)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

pub fn scan_repo(path: &Path, fetch: bool, relative_to: &Path) -> RepoInfo {
    let repo_str = path
        .strip_prefix(relative_to)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    let repo_str = if repo_str.is_empty() { ".".to_string() } else { repo_str };

    if fetch {
        let _ = Command::new("git")
            .args(["-C", &path.to_string_lossy()])
            .args(["fetch", "--quiet", "--all"])
            .output();
    }

    let branch = git(path, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_else(|| "?".to_string());

    let sha = git(path, &["rev-parse", "--short", "HEAD"])
        .unwrap_or_else(|| "?".to_string());

    let date = git(path, &["log", "-1", "--format=%ci"])
        .unwrap_or_default();

    let message = git(path, &["log", "-1", "--format=%s"])
        .unwrap_or_default();

    let remote = git(path, &["remote", "get-url", "origin"])
        .unwrap_or_default();

    let stash = git_any(path, &["stash", "list"])
        .map(|s| if s.is_empty() { 0 } else { s.lines().count() as i32 })
        .unwrap_or(0);

    let dirty = git_any(path, &["status", "--porcelain"])
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    // Get upstream tracking info
    let upstream = git(path, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]);

    let (ahead, behind, status) = if let Some(_up) = upstream {
        let counts = git_any(path, &["rev-list", "--left-right", "--count", "HEAD...@{u}"])
            .unwrap_or_default();
        let parts: Vec<&str> = counts.split_whitespace().collect();
        let a = parts.first().and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
        let b = parts.get(1).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
        let st = match (a, b) {
            (0, 0) => RepoStatus::UpToDate,
            (0, _) => RepoStatus::Behind,
            (_, 0) => RepoStatus::Ahead,
            _ => RepoStatus::Diverged,
        };
        (a, b, st)
    } else {
        (0, 0, RepoStatus::NoUpstream)
    };

    RepoInfo {
        repo: repo_str,
        branch,
        sha,
        date,
        ahead,
        behind,
        dirty,
        status,
        remote,
        message,
        stash,
    }
}
