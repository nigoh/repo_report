use crate::types::RepoInfo;

pub fn print_tsv(repos: &[RepoInfo]) {
    println!("repo\tbranch\tsha\tdate\tahead\tbehind\tdirty\tstatus\tremote\tmessage\tstash");
    for r in repos {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            r.repo, r.branch, r.sha, r.date,
            r.ahead, r.behind, r.dirty_str(), r.status.as_str(),
            r.remote, r.message, r.stash
        );
    }
}

pub fn print_json(repos: &[RepoInfo]) {
    // Use serde_json for proper escaping
    let json = serde_json::to_string_pretty(repos).unwrap_or_else(|_| "[]".to_string());
    println!("{json}");
}

pub fn print_table(repos: &[RepoInfo], msg_width: usize) {
    let header = format!(
        "{:<40}  {:<14}  {:<8}  {:>5}  {:>6}  {:<6}  {:<10}  {:<msg_width$}",
        "REPO", "BRANCH", "SHA", "AHEAD", "BEHIND", "DIRTY", "STATUS", "MESSAGE"
    );
    println!("{header}");
    println!("{}", "─".repeat(header.len()));
    for r in repos {
        let msg = if r.message.len() > msg_width {
            format!("{}…", &r.message[..msg_width.saturating_sub(1)])
        } else {
            r.message.clone()
        };
        println!(
            "{:<40}  {:<14}  {:<8}  {:>5}  {:>6}  {:<6}  {:<10}  {:<msg_width$}",
            trunc(&r.repo, 40), trunc(&r.branch, 14), trunc(&r.sha, 8),
            r.ahead, r.behind, r.dirty_str(), r.status.as_str(), msg
        );
    }
}

fn trunc(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}…", &s[..max.saturating_sub(1)]) }
}
