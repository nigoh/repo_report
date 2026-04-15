use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::aosp;
use crate::scanner::run_scan;
use crate::types::{AospEvent, AospOp, InputMode, Overlay, RepoInfo, RepoStatus, ScanEvent, SortMode};

pub struct App {
    pub repos: Vec<RepoInfo>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub filter: String,
    pub input_mode: InputMode,
    pub sort_mode: SortMode,
    pub show_header: bool,
    pub overlay: Option<Overlay>,
    pub ticker_offset: usize,
    pub ticker_tick: u64,
    pub scan_total: usize,
    pub scan_done: usize,
    pub scanning: bool,
    pub fetch: bool,
    pub scan_root: PathBuf,
    pub is_aosp: bool,
    pub manifest_branch: String,
    pub max_depth: usize,
    pub jobs: usize,
    pub should_quit: bool,
    // git diff overlay
    pub diff_lines: Vec<String>,
    pub diff_scroll: usize,
    // AOSP command state
    pub aosp_op: Option<AospOp>,
    pub aosp_output: Vec<String>,
    pub aosp_scroll: usize,
    pub aosp_running: bool,
    pub aosp_exit_ok: Option<bool>,
    pub aosp_prompt_buf: String,
    // private channels
    scan_rx: Option<Receiver<ScanEvent>>,
    aosp_rx: Option<Receiver<AospEvent>>,
}

impl App {
    pub fn new(scan_root: PathBuf, fetch: bool, max_depth: usize, is_aosp: bool, jobs: usize) -> Self {
        let manifest_branch = if is_aosp {
            read_manifest_branch(&scan_root)
        } else {
            String::new()
        };

        Self {
            repos: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            filter: String::new(),
            input_mode: InputMode::Normal,
            sort_mode: SortMode::Path,
            show_header: true,
            overlay: None,
            ticker_offset: 0,
            ticker_tick: 0,
            scan_total: 0,
            scan_done: 0,
            scanning: true,
            fetch,
            scan_root,
            is_aosp,
            manifest_branch,
            max_depth,
            jobs,
            should_quit: false,
            diff_lines: Vec::new(),
            diff_scroll: 0,
            aosp_op: None,
            aosp_output: Vec::new(),
            aosp_scroll: 0,
            aosp_running: false,
            aosp_exit_ok: None,
            aosp_prompt_buf: String::new(),
            scan_rx: None,
            aosp_rx: None,
        }
    }

    // ─── Scan ────────────────────────────────────────────────────────────────

    pub fn start_scan(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.scan_rx = Some(rx);
        self.repos.clear();
        self.filtered.clear();
        self.selected = 0;
        self.scroll_offset = 0;
        self.scan_total = 0;
        self.scan_done = 0;
        self.scanning = true;
        run_scan(self.scan_root.clone(), self.max_depth, self.fetch, tx);
    }

    pub fn drain_scan(&mut self) {
        if self.scan_rx.is_none() { return; }
        let mut count = 0;
        let mut new_repos: Vec<RepoInfo> = Vec::new();
        let mut new_done = self.scan_done;
        let mut new_total = self.scan_total;
        let mut finished = false;

        {
            let rx = self.scan_rx.as_ref().unwrap();
            while count < 50 {
                match rx.try_recv() {
                    Ok(ScanEvent::Found(info)) => { new_repos.push(info); count += 1; }
                    Ok(ScanEvent::Progress { scanned, total }) => { new_done = scanned; new_total = total; }
                    Ok(ScanEvent::Done) => { finished = true; break; }
                    Err(_) => break,
                }
            }
        }

        for info in new_repos { self.repos.push(info); }
        self.scan_done = new_done;
        self.scan_total = new_total;
        if finished {
            self.scanning = false;
            self.sort();
        }
        self.rebuild_filtered();
    }

    // ─── AOSP commands ───────────────────────────────────────────────────────

    /// Start a text-input prompt for ops that need an argument (branch, cmd, etc.)
    pub fn start_aosp_prompt(&mut self, op: AospOp) {
        self.aosp_prompt_buf.clear();
        self.input_mode = InputMode::AospPrompt(op);
        self.overlay = Some(Overlay::AospPrompt);
    }

    /// Show a y/N confirmation dialog before destructive ops
    pub fn start_aosp_confirm(&mut self, op: AospOp) {
        self.input_mode = InputMode::AospConfirm(op);
        self.overlay = Some(Overlay::AospConfirm);
    }

    /// Fire off a background AOSP command, opening the output overlay.
    pub fn launch_aosp_op(&mut self, op: AospOp, arg: Option<String>) {
        let (tx, rx): (Sender<AospEvent>, Receiver<AospEvent>) = mpsc::channel();
        self.aosp_rx = Some(rx);
        self.aosp_output.clear();
        self.aosp_scroll = 0;
        self.aosp_running = true;
        self.aosp_exit_ok = None;
        self.input_mode = InputMode::Normal;

        let is_manifest = op == AospOp::RepoManifest;
        self.aosp_op = Some(op.clone());
        self.overlay = Some(if is_manifest { Overlay::AospManifest } else { Overlay::AospCommand });

        let root = self.scan_root.clone();
        let jobs = self.jobs;
        std::thread::spawn(move || {
            aosp::run_aosp_op(root, op, arg, jobs, tx);
        });
    }

    pub fn drain_aosp(&mut self) {
        if self.aosp_rx.is_none() { return; }
        let mut count = 0;
        let mut lines: Vec<String> = Vec::new();
        let mut finished: Option<bool> = None;

        {
            let rx = self.aosp_rx.as_ref().unwrap();
            while count < 200 {
                match rx.try_recv() {
                    Ok(AospEvent::Line(l)) => { lines.push(l); count += 1; }
                    Ok(AospEvent::Done(ok)) => { finished = Some(ok); break; }
                    Err(_) => break,
                }
            }
        }

        for l in lines { self.aosp_output.push(l); }
        if let Some(ok) = finished {
            self.aosp_running = false;
            self.aosp_exit_ok = Some(ok);
            // After sync, trigger a rescan
            if matches!(self.aosp_op, Some(AospOp::RepoSync) | Some(AospOp::RepoSyncN)) && ok {
                self.start_scan();
            }
        }
    }

    // ─── Ticker ──────────────────────────────────────────────────────────────

    pub fn tick(&mut self) {
        self.ticker_tick += 1;
        if self.ticker_tick % 3 == 0 {
            self.ticker_offset = self.ticker_offset.wrapping_add(1);
        }
    }

    pub fn ticker_text(&self) -> String {
        let (total, dirty, behind, diverged) = self.counts();
        let spinner_chars = ['|', '/', '-', '\\'];
        let spin = spinner_chars[(self.ticker_tick as usize / 3) % 4];

        let aosp_badge = if self.is_aosp {
            if self.manifest_branch.is_empty() {
                "  [AOSP]".to_string()
            } else {
                format!("  [AOSP] repo:{}", self.manifest_branch)
            }
        } else {
            String::new()
        };

        let aosp_status = if self.aosp_running {
            if let Some(op) = &self.aosp_op {
                format!("  {} {} ", spin, op.label())
            } else {
                format!("  {} running ", spin)
            }
        } else {
            String::new()
        };

        if self.scanning {
            format!(
                " Scanning… {}/{} repos{} dirty:{}  behind:{}  diverged:{}{} ",
                self.scan_done, self.scan_total, aosp_badge, dirty, behind, diverged, aosp_status
            )
        } else {
            format!(
                " {} repos{}  dirty:{}  behind:{}  diverged:{}  sort:{}{} ",
                total, aosp_badge, dirty, behind, diverged,
                self.sort_mode.label(), aosp_status
            )
        }
    }

    // ─── Filtering / sorting ─────────────────────────────────────────────────

    pub fn rebuild_filtered(&mut self) {
        let filter = self.filter.to_lowercase();
        self.filtered = (0..self.repos.len())
            .filter(|&i| {
                if filter.is_empty() { return true; }
                let r = &self.repos[i];
                r.repo.to_lowercase().contains(&filter)
                    || r.branch.to_lowercase().contains(&filter)
                    || r.status.as_str().contains(&filter)
            })
            .collect();

        if self.selected >= self.filtered.len().max(1) {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn sort(&mut self) {
        match self.sort_mode {
            SortMode::Path => self.repos.sort_by(|a, b| a.repo.cmp(&b.repo)),
            SortMode::Status => self.repos.sort_by(|a, b| status_order(&a.status).cmp(&status_order(&b.status))),
            SortMode::Date => self.repos.sort_by(|a, b| b.date.cmp(&a.date)),
            SortMode::Branch => self.repos.sort_by(|a, b| a.branch.cmp(&b.branch)),
            SortMode::AheadDesc => self.repos.sort_by(|a, b| b.ahead.cmp(&a.ahead)),
            SortMode::BehindDesc => self.repos.sort_by(|a, b| b.behind.cmp(&a.behind)),
        }
        self.rebuild_filtered();
    }

    // ─── Navigation ──────────────────────────────────────────────────────────

    pub fn move_down(&mut self) {
        if !self.filtered.is_empty() && self.selected + 1 < self.filtered.len() {
            self.selected += 1;
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 { self.selected -= 1; }
    }

    pub fn move_top(&mut self) { self.selected = 0; self.scroll_offset = 0; }

    pub fn move_bottom(&mut self) {
        self.selected = self.filtered.len().saturating_sub(1);
    }

    pub fn page_down(&mut self, page_size: usize) {
        let max = self.filtered.len().saturating_sub(1);
        self.selected = (self.selected + page_size).min(max);
    }

    pub fn page_up(&mut self, page_size: usize) {
        self.selected = self.selected.saturating_sub(page_size);
    }

    pub fn clamp_scroll(&mut self, visible: usize) {
        if self.filtered.is_empty() { return; }
        if self.selected < self.scroll_offset { self.scroll_offset = self.selected; }
        if self.selected >= self.scroll_offset + visible {
            self.scroll_offset = self.selected - visible + 1;
        }
    }

    // ─── Helpers ─────────────────────────────────────────────────────────────

    pub fn selected_repo(&self) -> Option<&RepoInfo> {
        self.filtered.get(self.selected).and_then(|&i| self.repos.get(i))
    }

    pub fn counts(&self) -> (usize, usize, usize, usize) {
        let total = self.repos.len();
        let dirty = self.repos.iter().filter(|r| r.dirty).count();
        let behind = self.repos.iter().filter(|r| r.status == RepoStatus::Behind).count();
        let diverged = self.repos.iter().filter(|r| r.status == RepoStatus::Diverged).count();
        (total, dirty, behind, diverged)
    }

    pub fn load_diff(&mut self) {
        if let Some(repo) = self.selected_repo() {
            let path = self.scan_root.join(&repo.repo);
            let output = Command::new("git")
                .args(["-C", &path.to_string_lossy()])
                .args(["diff", "HEAD"])
                .output();
            self.diff_lines = output
                .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(String::from).collect())
                .unwrap_or_default();
            self.diff_scroll = 0;
        }
    }
}

fn status_order(s: &RepoStatus) -> u8 {
    match s {
        RepoStatus::Diverged => 0,
        RepoStatus::Behind => 1,
        RepoStatus::Ahead => 2,
        RepoStatus::NoUpstream => 3,
        RepoStatus::UpToDate => 4,
    }
}

/// Read the manifest branch from .repo/manifest.xml or `repo info -l`.
/// Runs synchronously once at startup (before TUI loop starts).
fn read_manifest_branch(root: &PathBuf) -> String {
    // Fast path: parse manifest.xml directly
    let manifest = root.join(".repo").join("manifest.xml");
    if let Ok(content) = std::fs::read_to_string(&manifest) {
        for line in content.lines() {
            let trimmed = line.trim();
            // Look for <manifest> or <default> with revision attribute
            if trimmed.contains("revision=") {
                if let Some(start) = trimmed.find("revision=\"") {
                    let rest = &trimmed[start + 10..];
                    if let Some(end) = rest.find('"') {
                        let rev = &rest[..end];
                        if !rev.starts_with("refs/") {
                            return rev.to_string();
                        }
                        // Strip refs/heads/ prefix
                        return rev.trim_start_matches("refs/heads/").to_string();
                    }
                }
            }
        }
    }

    // Fallback: `repo info -l`
    if let Ok(out) = Command::new("repo")
        .args(["info", "-l"])
        .current_dir(root)
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.contains("Manifest branch:") {
                if let Some(branch) = line.split(':').nth(1) {
                    return branch.trim().to_string();
                }
            }
        }
    }

    String::new()
}
