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

/// AOSP operations that can be launched from the TUI
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AospOp {
    RepoSync,       // F: repo sync -c -j{jobs} --no-tags
    RepoSyncN,      // n: repo sync -n
    RepoStatus,     // T: repo status
    RepoBranches,   // b: repo branches
    RepoOverview,   // o: repo overview
    RepoManifest,   // m: .repo/manifest.xml viewer
    RepoStart,      // B: repo start <branch> --all
    RepoAbandon,    // A: repo abandon <branch>
    RepoForall,     // :: repo forall -c <cmd>
    RepoDownload,   // D: repo download <project> <change>
    MakeBuild,      // M: make -j{jobs}
    MakeClean,      // C: make clean
    #[allow(dead_code)]
    MakeClobber,    // (future: make clobber)
}

impl AospOp {
    pub fn label(&self) -> &'static str {
        match self {
            AospOp::RepoSync => "repo sync -c --no-tags",
            AospOp::RepoSyncN => "repo sync -n (fetch only)",
            AospOp::RepoStatus => "repo status",
            AospOp::RepoBranches => "repo branches",
            AospOp::RepoOverview => "repo overview",
            AospOp::RepoManifest => "manifest.xml",
            AospOp::RepoStart => "repo start <branch> --all",
            AospOp::RepoAbandon => "repo abandon <branch>",
            AospOp::RepoForall => "repo forall -c <cmd>",
            AospOp::RepoDownload => "repo download <project> <change>",
            AospOp::MakeBuild => "make -j{jobs}",
            AospOp::MakeClean => "make clean",
            AospOp::MakeClobber => "make clobber",
        }
    }

    pub fn prompt_hint(&self) -> Option<&'static str> {
        match self {
            AospOp::RepoStart => Some("Branch name"),
            AospOp::RepoAbandon => Some("Branch name to abandon"),
            AospOp::RepoForall => Some("Command (e.g. git log -1 --oneline)"),
            AospOp::RepoDownload => Some("project change-id  (e.g. platform/frameworks/base 12345)"),
            _ => None,
        }
    }
}

/// Events streamed from a background AOSP command thread
#[derive(Debug, Clone)]
pub enum AospEvent {
    Line(String),
    Done(bool), // true = success
}

/// Active keyboard input mode
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Filter,
    AospPrompt(AospOp),  // waiting for text input before executing op
    AospConfirm(AospOp), // waiting for y/n before executing destructive op
}

/// TUI overlay variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    Help,
    Detail,
    Diff,
    AospCommand,  // scrollable command output
    AospManifest, // read-only manifest.xml view
    AospConfirm,  // destructive-op confirmation dialog
    AospPrompt,   // single-line text input box
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── SortMode ──────────────────────────────────────────────────────────────

    #[test]
    fn sort_mode_cycles_through_all_variants() {
        let start = SortMode::Path;
        let mut mode = start;
        let variants = [
            SortMode::Path,
            SortMode::Status,
            SortMode::Date,
            SortMode::Branch,
            SortMode::AheadDesc,
            SortMode::BehindDesc,
        ];
        for expected in variants {
            assert_eq!(mode, expected);
            mode = mode.next();
        }
        // Should wrap back to Path
        assert_eq!(mode, SortMode::Path);
    }

    #[test]
    fn sort_mode_labels_are_non_empty() {
        for mode in [SortMode::Path, SortMode::Status, SortMode::Date,
                     SortMode::Branch, SortMode::AheadDesc, SortMode::BehindDesc] {
            assert!(!mode.label().is_empty(), "label for {:?} should not be empty", mode);
        }
    }

    // ── RepoStatus ────────────────────────────────────────────────────────────

    #[test]
    fn repo_status_as_str_round_trips() {
        let cases = [
            (RepoStatus::UpToDate, "up-to-date"),
            (RepoStatus::Behind, "behind"),
            (RepoStatus::Ahead, "ahead"),
            (RepoStatus::Diverged, "diverged"),
            (RepoStatus::NoUpstream, "no-upstream"),
        ];
        for (status, expected) in cases {
            assert_eq!(status.as_str(), expected);
        }
    }

    // ── AospOp ────────────────────────────────────────────────────────────────

    #[test]
    fn aosp_op_labels_are_non_empty() {
        let ops = [
            AospOp::RepoSync, AospOp::RepoSyncN, AospOp::RepoStatus,
            AospOp::RepoBranches, AospOp::RepoOverview, AospOp::RepoManifest,
            AospOp::RepoStart, AospOp::RepoAbandon, AospOp::RepoForall,
            AospOp::RepoDownload, AospOp::MakeBuild, AospOp::MakeClean,
        ];
        for op in ops {
            assert!(!op.label().is_empty(), "label for {:?} is empty", op);
        }
    }

    #[test]
    fn aosp_op_prompt_hint_present_for_interactive_ops() {
        assert!(AospOp::RepoStart.prompt_hint().is_some());
        assert!(AospOp::RepoAbandon.prompt_hint().is_some());
        assert!(AospOp::RepoForall.prompt_hint().is_some());
        assert!(AospOp::RepoDownload.prompt_hint().is_some());
    }

    #[test]
    fn aosp_op_prompt_hint_absent_for_direct_ops() {
        assert!(AospOp::RepoSync.prompt_hint().is_none());
        assert!(AospOp::RepoSyncN.prompt_hint().is_none());
        assert!(AospOp::RepoStatus.prompt_hint().is_none());
        assert!(AospOp::MakeBuild.prompt_hint().is_none());
        assert!(AospOp::MakeClean.prompt_hint().is_none());
    }

    // ── RepoInfo ──────────────────────────────────────────────────────────────

    #[test]
    fn repo_info_dirty_str() {
        let make = |dirty: bool| RepoInfo {
            repo: "x".into(), branch: "main".into(), sha: "abc".into(),
            date: String::new(), ahead: 0, behind: 0, dirty,
            status: RepoStatus::UpToDate, remote: String::new(),
            message: String::new(), stash: 0,
        };
        assert_eq!(make(true).dirty_str(), "dirty");
        assert_eq!(make(false).dirty_str(), "clean");
    }
}
