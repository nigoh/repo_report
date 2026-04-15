use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use crate::types::{RepoInfo, RepoStatus, SortMode, ScanEvent, Overlay};
use crate::scanner::run_scan;

pub struct App {
    pub repos: Vec<RepoInfo>,
    pub filtered: Vec<usize>,   // indices into repos
    pub selected: usize,
    pub scroll_offset: usize,
    pub filter: String,
    pub filter_mode: bool,
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
    pub _is_aosp: bool,
    pub max_depth: usize,
    pub should_quit: bool,
    pub diff_lines: Vec<String>,
    pub diff_scroll: usize,
    rx: Option<Receiver<ScanEvent>>,
}

impl App {
    pub fn new(scan_root: PathBuf, fetch: bool, max_depth: usize, is_aosp: bool) -> Self {
        Self {
            repos: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            filter: String::new(),
            filter_mode: false,
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
            _is_aosp: is_aosp,
            max_depth,
            should_quit: false,
            diff_lines: Vec::new(),
            diff_scroll: 0,
            rx: None,
        }
    }

    pub fn start_scan(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
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
        if self.rx.is_none() { return; }
        let mut count = 0;
        let mut new_repos: Vec<crate::types::RepoInfo> = Vec::new();
        let mut new_done = self.scan_done;
        let mut new_total = self.scan_total;
        let mut scan_finished = false;

        {
            let rx = self.rx.as_ref().unwrap();
            while count < 50 {
                match rx.try_recv() {
                    Ok(ScanEvent::Found(info)) => {
                        new_repos.push(info);
                        count += 1;
                    }
                    Ok(ScanEvent::Progress { scanned, total }) => {
                        new_done = scanned;
                        new_total = total;
                    }
                    Ok(ScanEvent::Done) => {
                        scan_finished = true;
                        break;
                    }
                    Err(_) => break,
                }
            }
        }

        for info in new_repos {
            self.repos.push(info);
        }
        self.scan_done = new_done;
        self.scan_total = new_total;
        if scan_finished {
            self.scanning = false;
            self.sort();
        }
        self.rebuild_filtered();
    }

    pub fn tick(&mut self) {
        self.ticker_tick += 1;
        if self.ticker_tick % 3 == 0 {
            self.ticker_offset = self.ticker_offset.wrapping_add(1);
        }
    }

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
            SortMode::Status => self.repos.sort_by(|a, b| {
                status_order(&a.status).cmp(&status_order(&b.status))
            }),
            SortMode::Date => self.repos.sort_by(|a, b| b.date.cmp(&a.date)),
            SortMode::Branch => self.repos.sort_by(|a, b| a.branch.cmp(&b.branch)),
            SortMode::AheadDesc => self.repos.sort_by(|a, b| b.ahead.cmp(&a.ahead)),
            SortMode::BehindDesc => self.repos.sort_by(|a, b| b.behind.cmp(&a.behind)),
        }
        self.rebuild_filtered();
    }

    pub fn move_down(&mut self) {
        if !self.filtered.is_empty() && self.selected + 1 < self.filtered.len() {
            self.selected += 1;
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_top(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
    }

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
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }
        if self.selected >= self.scroll_offset + visible {
            self.scroll_offset = self.selected - visible + 1;
        }
    }

    pub fn selected_repo(&self) -> Option<&RepoInfo> {
        self.filtered.get(self.selected).and_then(|&i| self.repos.get(i))
    }

    pub fn counts(&self) -> (usize, usize, usize, usize) {
        // (total, dirty, behind, diverged)
        let total = self.repos.len();
        let dirty = self.repos.iter().filter(|r| r.dirty).count();
        let behind = self.repos.iter().filter(|r| r.status == RepoStatus::Behind).count();
        let diverged = self.repos.iter().filter(|r| r.status == RepoStatus::Diverged).count();
        (total, dirty, behind, diverged)
    }

    pub fn ticker_text(&self) -> String {
        let (total, dirty, behind, diverged) = self.counts();
        if self.scanning {
            format!(
                " Scanning… {}/{} repos found  dirty:{}  behind:{}  diverged:{} ",
                self.scan_done, self.scan_total, dirty, behind, diverged
            )
        } else {
            format!(
                " {} repos  dirty:{}  behind:{}  diverged:{}  sort:{}  root:{}  ",
                total, dirty, behind, diverged,
                self.sort_mode.label(),
                self.scan_root.display()
            )
        }
    }

    pub fn load_diff(&mut self) {
        if let Some(repo) = self.selected_repo() {
            let path = self.scan_root.join(&repo.repo);
            let output = std::process::Command::new("git")
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
