use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;

use crate::types::{AospEvent, AospOp};

/// Commands blocked in `repo forall -c` to prevent accidental mass destruction
const FORALL_BLOCKLIST: &[&str] = &[
    "reset --hard",
    "clean -f",
    "clean -fd",
    "rm -rf",
    "rm -fr",
    "push --force",
    "push -f",
    "format",
];

/// Main entry point. Called from a background thread spawned by App::launch_aosp_op.
pub fn run_aosp_op(root: PathBuf, op: AospOp, arg: Option<String>, jobs: usize, tx: Sender<AospEvent>) {
    match op {
        AospOp::RepoSync => run_cmd(&root, "repo", &["sync", "-c", &format!("-j{}", jobs), "--no-tags"], &tx),
        AospOp::RepoSyncN => run_cmd(&root, "repo", &["sync", "-n", &format!("-j{}", jobs)], &tx),
        AospOp::RepoStatus => run_cmd(&root, "repo", &["status"], &tx),
        AospOp::RepoBranches => run_cmd(&root, "repo", &["branches"], &tx),
        AospOp::RepoOverview => run_cmd(&root, "repo", &["overview"], &tx),
        AospOp::RepoManifest => read_manifest(&root, &tx),
        AospOp::RepoStart => {
            let branch = arg.unwrap_or_default();
            if branch.is_empty() {
                send_err(&tx, "branch name is empty");
                return;
            }
            run_cmd(&root, "repo", &["start", &branch, "--all"], &tx);
        }
        AospOp::RepoAbandon => {
            let branch = arg.unwrap_or_default();
            if branch.is_empty() {
                send_err(&tx, "branch name is empty");
                return;
            }
            run_cmd(&root, "repo", &["abandon", &branch], &tx);
        }
        AospOp::RepoForall => {
            let cmd = arg.unwrap_or_default();
            if cmd.is_empty() {
                send_err(&tx, "command is empty");
                return;
            }
            // Safety blocklist check
            let cmd_lower = cmd.to_lowercase();
            for blocked in FORALL_BLOCKLIST {
                if cmd_lower.contains(blocked) {
                    send_err(&tx, &format!("blocked: '{}' matches blocklist pattern '{}'", cmd, blocked));
                    let _ = tx.send(AospEvent::Done(false));
                    return;
                }
            }
            run_cmd(&root, "repo", &["forall", "-c", &cmd], &tx);
        }
        AospOp::RepoDownload => {
            let raw = arg.unwrap_or_default();
            let parts: Vec<&str> = raw.splitn(2, ' ').collect();
            if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
                send_err(&tx, "usage: <project> <change-id>  e.g. platform/frameworks/base 12345");
                return;
            }
            run_cmd(&root, "repo", &["download", parts[0], parts[1]], &tx);
        }
        AospOp::MakeBuild => run_cmd(&root, "make", &[&format!("-j{}", jobs)], &tx),
        AospOp::MakeClean => run_cmd(&root, "make", &["clean"], &tx),
        AospOp::MakeClobber => run_cmd(&root, "make", &["clobber"], &tx),
    }
}

/// Stream a command's stdout+stderr to the tx channel, then send Done.
fn run_cmd(root: &PathBuf, program: &str, args: &[&str], tx: &Sender<AospEvent>) {
    let mut child = match Command::new(program)
        .args(args)
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            send_err(tx, &format!("'{}' not found on PATH", program));
            let _ = tx.send(AospEvent::Done(false));
            return;
        }
        Err(e) => {
            send_err(tx, &format!("failed to spawn '{}': {}", program, e));
            let _ = tx.send(AospEvent::Done(false));
            return;
        }
    };

    // Stream stderr in a separate thread with [err] prefix
    if let Some(stderr) = child.stderr.take() {
        let tx2 = tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().flatten() {
                let _ = tx2.send(AospEvent::Line(format!("[err] {}", line)));
            }
        });
    }

    // Stream stdout in this thread
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().flatten() {
            if tx.send(AospEvent::Line(line)).is_err() {
                break;
            }
        }
    }

    let ok = child.wait().map(|s| s.success()).unwrap_or(false);
    let _ = tx.send(AospEvent::Done(ok));
}

/// Read .repo/manifest.xml directly (no subprocess) and stream its lines.
fn read_manifest(root: &PathBuf, tx: &Sender<AospEvent>) {
    let manifest = root.join(".repo").join("manifest.xml");
    match std::fs::read_to_string(&manifest) {
        Ok(content) => {
            for line in content.lines() {
                if tx.send(AospEvent::Line(line.to_string())).is_err() {
                    break;
                }
            }
            let _ = tx.send(AospEvent::Done(true));
        }
        Err(e) => {
            send_err(tx, &format!("cannot read manifest.xml: {}", e));
            let _ = tx.send(AospEvent::Done(false));
        }
    }
}

fn send_err(tx: &Sender<AospEvent>, msg: &str) {
    let _ = tx.send(AospEvent::Line(format!("[err] {}", msg)));
}

/// Check if a `repo forall -c` command would be blocked.
/// Exposed for testing; the main path calls this inline via `run_aosp_op`.
pub fn is_forall_blocked(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    FORALL_BLOCKLIST.iter().any(|pat| lower.contains(pat))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::path::PathBuf;

    // ── Forall blocklist ──────────────────────────────────────────────────────

    #[test]
    fn blocklist_catches_reset_hard() {
        assert!(is_forall_blocked("git reset --hard HEAD"));
        assert!(is_forall_blocked("GIT RESET --HARD HEAD")); // case-insensitive
    }

    #[test]
    fn blocklist_catches_clean_f() {
        assert!(is_forall_blocked("git clean -f"));
        assert!(is_forall_blocked("git clean -fd"));
    }

    #[test]
    fn blocklist_catches_rm_rf() {
        assert!(is_forall_blocked("rm -rf /tmp/foo"));
        assert!(is_forall_blocked("rm -fr /tmp/foo"));
    }

    #[test]
    fn blocklist_catches_force_push() {
        assert!(is_forall_blocked("git push --force"));
        assert!(is_forall_blocked("git push -f origin main"));
    }

    #[test]
    fn blocklist_allows_safe_commands() {
        assert!(!is_forall_blocked("git log -1 --oneline"));
        assert!(!is_forall_blocked("git status"));
        assert!(!is_forall_blocked("git diff HEAD~1"));
        assert!(!is_forall_blocked("git fetch --all"));
        assert!(!is_forall_blocked("git branch -a"));
    }

    // ── Manifest reading ──────────────────────────────────────────────────────

    #[test]
    fn read_manifest_streams_lines_and_signals_done() {
        // Create a temp dir with a fake manifest
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo_dir = tmp.path().join(".repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        std::fs::write(
            repo_dir.join("manifest.xml"),
            "<?xml version=\"1.0\"?>\n<manifest>\n  <default revision=\"main\"/>\n</manifest>\n",
        ).unwrap();

        let (tx, rx) = mpsc::channel();
        let root = PathBuf::from(tmp.path());
        read_manifest(&root, &tx);
        drop(tx); // ensure channel is closed

        let mut lines = Vec::new();
        let mut done = None;
        while let Ok(ev) = rx.recv() {
            match ev {
                AospEvent::Line(l) => lines.push(l),
                AospEvent::Done(ok) => done = Some(ok),
            }
        }

        assert!(!lines.is_empty(), "expected manifest lines");
        assert!(lines.iter().any(|l| l.contains("manifest")));
        assert_eq!(done, Some(true));
    }

    #[test]
    fn read_manifest_errors_when_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (tx, rx) = mpsc::channel();
        let root = PathBuf::from(tmp.path());
        read_manifest(&root, &tx);
        drop(tx);

        let events: Vec<_> = std::iter::from_fn(|| rx.recv().ok()).collect();
        let has_err = events.iter().any(|e| matches!(e, AospEvent::Line(l) if l.contains("[err]")));
        let done_false = events.iter().any(|e| matches!(e, AospEvent::Done(false)));
        assert!(has_err);
        assert!(done_false);
    }
}
