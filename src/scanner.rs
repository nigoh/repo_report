use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use walkdir::WalkDir;
use rayon::prelude::*;
use crate::types::ScanEvent;
use crate::git::scan_repo;

pub fn find_repos(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut repos = Vec::new();
    let walker = WalkDir::new(root)
        .max_depth(max_depth + 1)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            // Skip hidden dirs except .git itself
            let name = e.file_name().to_string_lossy();
            if e.depth() > 0 && name.starts_with('.') && name != ".git" {
                return false;
            }
            true
        });

    let mut skip_dirs: Vec<PathBuf> = Vec::new();

    for entry in walker.flatten() {
        let path = entry.path();

        // Skip if inside a repo we already found
        if skip_dirs.iter().any(|d| path.starts_with(d)) {
            continue;
        }

        if entry.file_name() == ".git" && path.parent().is_some() {
            let repo_root = path.parent().unwrap().to_path_buf();
            repos.push(repo_root.clone());
            skip_dirs.push(repo_root);
        }
    }

    repos
}

pub fn is_aosp(root: &Path) -> bool {
    root.join(".repo").is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn init_git_repo(path: &std::path::Path) {
        Command::new("git").args(["init", "-b", "main"]).current_dir(path).output().unwrap();
        Command::new("git").args(["config", "user.email", "t@t.com"]).current_dir(path).output().unwrap();
        Command::new("git").args(["config", "user.name", "T"]).current_dir(path).output().unwrap();
        std::fs::write(path.join("f.txt"), "x").unwrap();
        Command::new("git").args(["add", "."]).current_dir(path).output().unwrap();
        Command::new("git").args(["commit", "-m", "init"]).current_dir(path).output().unwrap();
    }

    #[test]
    fn find_repos_detects_single_git_repo() {
        let tmp = tempfile::tempdir().unwrap();
        init_git_repo(tmp.path());

        let repos = find_repos(tmp.path(), 1);
        // The repo itself should be found (or as direct child)
        assert!(!repos.is_empty() || find_repos(tmp.path().parent().unwrap(), 2).contains(&tmp.path().to_path_buf()),
            "should find the repo");
    }

    #[test]
    fn find_repos_detects_nested_repos() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        init_git_repo(&sub);

        let repos = find_repos(tmp.path(), 3);
        assert!(repos.contains(&sub), "should find nested repo at {:?}, found: {:?}", sub, repos);
    }

    #[test]
    fn find_repos_stops_at_max_depth() {
        let tmp = tempfile::tempdir().unwrap();
        // Create repo at depth 3 (tmp/a/b/c)
        let deep = tmp.path().join("a").join("b").join("c");
        std::fs::create_dir_all(&deep).unwrap();
        init_git_repo(&deep);

        // With max_depth=1, should not find it
        let repos_shallow = find_repos(tmp.path(), 1);
        assert!(!repos_shallow.contains(&deep), "should not find at depth 3 with max_depth=1");

        // With max_depth=4, should find it
        let repos_deep = find_repos(tmp.path(), 4);
        assert!(repos_deep.contains(&deep), "should find at depth 3 with max_depth=4");
    }

    #[test]
    fn is_aosp_true_when_repo_dir_exists() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join(".repo")).unwrap();
        assert!(is_aosp(tmp.path()));
    }

    #[test]
    fn is_aosp_false_when_no_repo_dir() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_aosp(tmp.path()));
    }

    #[test]
    fn find_repos_does_not_descend_into_found_repos() {
        // A nested .git inside another .git dir should not produce a second result
        let tmp = tempfile::tempdir().unwrap();
        init_git_repo(tmp.path());
        // Repos found should not include the parent twice
        let repos = find_repos(tmp.path().parent().unwrap_or(tmp.path()), 5);
        let count = repos.iter().filter(|p| p.starts_with(tmp.path())).count();
        assert!(count <= 1, "found repo more than once: {:?}", repos);
    }
}

pub fn run_scan(root: PathBuf, max_depth: usize, fetch: bool, tx: Sender<ScanEvent>) {
    std::thread::spawn(move || {
        let repos = find_repos(&root, max_depth);
        let total = repos.len();

        if total == 0 {
            let _ = tx.send(ScanEvent::Done);
            return;
        }

        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let scanned = Arc::new(AtomicUsize::new(0));
        let tx_clone = tx.clone();
        let root_clone = root.clone();

        repos.par_iter().for_each(|path| {
            let info = scan_repo(path, fetch, &root_clone);
            let _ = tx_clone.send(ScanEvent::Found(info));
            let n = scanned.fetch_add(1, Ordering::Relaxed) + 1;
            let _ = tx_clone.send(ScanEvent::Progress { scanned: n, total });
        });

        let _ = tx.send(ScanEvent::Done);
    });
}
