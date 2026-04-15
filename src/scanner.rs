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
