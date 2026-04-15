use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RepoStatus {
    UpToDate,
    Behind,
    Ahead,
    Diverged,
    NoUpstream,
}

impl RepoStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RepoStatus::UpToDate => "up-to-date",
            RepoStatus::Behind => "behind",
            RepoStatus::Ahead => "ahead",
            RepoStatus::Diverged => "diverged",
            RepoStatus::NoUpstream => "no-upstream",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoInfo {
    pub repo: String,
    pub branch: String,
    pub sha: String,
    pub date: String,
    pub ahead: i32,
    pub behind: i32,
    pub dirty: bool,
    pub status: RepoStatus,
    pub remote: String,
    pub message: String,
    pub stash: i32,
}

impl RepoInfo {
    pub fn dirty_str(&self) -> &'static str {
        if self.dirty { "dirty" } else { "clean" }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Path,
    Status,
    Date,
    Branch,
    AheadDesc,
    BehindDesc,
}

impl SortMode {
    pub fn next(self) -> Self {
        match self {
            SortMode::Path => SortMode::Status,
            SortMode::Status => SortMode::Date,
            SortMode::Date => SortMode::Branch,
            SortMode::Branch => SortMode::AheadDesc,
            SortMode::AheadDesc => SortMode::BehindDesc,
            SortMode::BehindDesc => SortMode::Path,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SortMode::Path => "path",
            SortMode::Status => "status",
            SortMode::Date => "date",
            SortMode::Branch => "branch",
            SortMode::AheadDesc => "ahead↓",
            SortMode::BehindDesc => "behind↓",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Tui,
    Tsv,
    Json,
    Table,
}

#[derive(Debug, Clone)]
pub enum ScanEvent {
    Found(RepoInfo),
    Progress { scanned: usize, total: usize },
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    Help,
    Detail,
    Diff,
}
