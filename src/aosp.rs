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
