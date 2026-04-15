mod aosp;
mod app;
mod git;
mod output;
mod scanner;
mod types;
mod ui;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::App;
use crate::scanner::{find_repos, is_aosp};
use crate::types::{AospOp, InputMode, Overlay, OutputFormat};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Format {
    Tui,
    Tsv,
    Json,
    Table,
}

#[derive(Parser, Debug)]
#[command(
    name = "repo-report-tui",
    version = "0.3.0",
    about = "Git repository status reporter — Ratatui edition"
)]
struct Cli {
    /// Root directory to scan
    #[arg(default_value = ".")]
    root: PathBuf,

    /// Output format (default: tui when stdout is a tty, tsv otherwise)
    #[arg(short = 'F', long)]
    format: Option<Format>,

    /// Fetch from remotes before reporting
    #[arg(short, long)]
    fetch: bool,

    /// Maximum directory depth to scan for repos
    #[arg(long, default_value = "5")]
    max_depth: usize,

    /// Column width for commit message in table format
    #[arg(long, default_value = "60")]
    msg_width: usize,

    /// Non-interactive mode (equivalent to --format tsv)
    #[arg(long)]
    non_interactive: bool,

    /// Parallel job count (default: number of logical CPUs)
    #[arg(short, long)]
    jobs: Option<usize>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = cli.root.canonicalize().unwrap_or(cli.root.clone());

    let jobs = cli.jobs.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    });

    let is_tty = atty_check();
    let format = match cli.format {
        Some(Format::Tsv) => OutputFormat::Tsv,
        Some(Format::Json) => OutputFormat::Json,
        Some(Format::Table) => OutputFormat::Table,
        Some(Format::Tui) => OutputFormat::Tui,
        None if cli.non_interactive => OutputFormat::Tsv,
        None if is_tty => OutputFormat::Tui,
        None => OutputFormat::Tsv,
    };

    match format {
        OutputFormat::Tsv | OutputFormat::Json | OutputFormat::Table => {
            run_non_interactive(&root, cli.fetch, cli.max_depth, cli.msg_width, jobs, format)
        }
        OutputFormat::Tui => run_tui(&root, cli.fetch, cli.max_depth, jobs),
    }
}

fn run_non_interactive(
    root: &PathBuf,
    fetch: bool,
    max_depth: usize,
    msg_width: usize,
    _jobs: usize,
    format: OutputFormat,
) -> Result<()> {
    use rayon::prelude::*;
    use crate::git::scan_repo;

    let paths = find_repos(root, max_depth);
    let mut repos: Vec<_> = paths.par_iter().map(|p| scan_repo(p, fetch, root)).collect();
    repos.sort_by(|a, b| a.repo.cmp(&b.repo));

    match format {
        OutputFormat::Tsv => output::print_tsv(&repos),
        OutputFormat::Json => output::print_json(&repos),
        OutputFormat::Table => output::print_table(&repos, msg_width),
        _ => unreachable!(),
    }

    let all_good = repos.iter().all(|r| {
        !r.dirty && matches!(r.status, crate::types::RepoStatus::UpToDate | crate::types::RepoStatus::NoUpstream)
    });
    std::process::exit(if all_good { 0 } else { 1 });
}

fn run_tui(root: &PathBuf, fetch: bool, max_depth: usize, jobs: usize) -> Result<()> {
    let aosp = is_aosp(root);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(root.clone(), fetch, max_depth, aosp, jobs);
    app.start_scan();

    let result = run_event_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        app.drain_scan();
        app.drain_aosp();
        app.tick();

        terminal.draw(|f| ui::render(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    handle_key(app, key.code, key.modifiers);
                }
                _ => {}
            }
        }

        if app.should_quit { break; }
    }
    Ok(())
}

// ─── Key handling ─────────────────────────────────────────────────────────────

fn handle_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    match app.input_mode.clone() {
        InputMode::Filter => handle_key_filter(app, code),
        InputMode::AospPrompt(op) => handle_key_aosp_prompt(app, op, code),
        InputMode::AospConfirm(op) => handle_key_aosp_confirm(app, op, code),
        InputMode::Normal => handle_key_normal(app, code, modifiers),
    }
}

fn handle_key_filter(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.filter.clear();
            app.rebuild_filtered();
        }
        KeyCode::Enter => {
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            app.filter.pop();
            app.rebuild_filtered();
        }
        KeyCode::Char(c) => {
            app.filter.push(c);
            app.rebuild_filtered();
        }
        _ => {}
    }
}

fn handle_key_aosp_prompt(app: &mut App, op: AospOp, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.overlay = None;
        }
        KeyCode::Enter => {
            let arg = app.aosp_prompt_buf.trim().to_string();
            app.launch_aosp_op(op, Some(arg));
        }
        KeyCode::Backspace => {
            app.aosp_prompt_buf.pop();
        }
        KeyCode::Char(c) => {
            app.aosp_prompt_buf.push(c);
        }
        _ => {}
    }
}

fn handle_key_aosp_confirm(app: &mut App, op: AospOp, code: KeyCode) {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            // Ops that still need a text argument after confirmation
            match op {
                AospOp::RepoAbandon => {
                    app.input_mode = InputMode::Normal;
                    app.overlay = None;
                    app.start_aosp_prompt(AospOp::RepoAbandon);
                }
                _ => {
                    app.launch_aosp_op(op, None);
                }
            }
        }
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Enter => {
            app.input_mode = InputMode::Normal;
            app.overlay = None;
        }
        _ => {}
    }
}

fn handle_key_normal(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    // Handle open overlays first (scroll only, Esc to close)
    if let Some(overlay) = app.overlay {
        match overlay {
            Overlay::AospCommand | Overlay::AospManifest => {
                match code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        // Keep overlay open while command is running, allow close when done
                        if !app.aosp_running {
                            app.overlay = None;
                        }
                    }
                    KeyCode::Char('x') => app.overlay = None, // force close
                    KeyCode::Char('j') | KeyCode::Down => {
                        let max = app.aosp_output.len().saturating_sub(1);
                        app.aosp_scroll = (app.aosp_scroll + 1).min(max);
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        app.aosp_scroll = app.aosp_scroll.saturating_sub(1);
                    }
                    KeyCode::PageDown => {
                        let max = app.aosp_output.len().saturating_sub(1);
                        app.aosp_scroll = (app.aosp_scroll + 20).min(max);
                    }
                    KeyCode::PageUp => {
                        app.aosp_scroll = app.aosp_scroll.saturating_sub(20);
                    }
                    _ => {}
                }
                return;
            }
            Overlay::Help | Overlay::Detail => {
                // Any key closes these overlays
                app.overlay = None;
                return;
            }
            Overlay::Diff => {
                match code {
                    KeyCode::Esc | KeyCode::Char('q') => app.overlay = None,
                    KeyCode::Char('j') | KeyCode::Down => {
                        let max = app.diff_lines.len().saturating_sub(1);
                        app.diff_scroll = (app.diff_scroll + 1).min(max);
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        app.diff_scroll = app.diff_scroll.saturating_sub(1);
                    }
                    KeyCode::PageDown => {
                        let max = app.diff_lines.len().saturating_sub(1);
                        app.diff_scroll = (app.diff_scroll + 20).min(max);
                    }
                    KeyCode::PageUp => {
                        app.diff_scroll = app.diff_scroll.saturating_sub(20);
                    }
                    _ => {}
                }
                return;
            }
            // AospConfirm and AospPrompt are handled by their respective input modes
            _ => {}
        }
    }

    // Normal navigation and commands
    match code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => app.should_quit = true,

        KeyCode::Char('j') | KeyCode::Down => app.move_down(),
        KeyCode::Char('k') | KeyCode::Up => app.move_up(),
        KeyCode::Char('g') => app.move_top(),
        KeyCode::Char('G') => app.move_bottom(),
        KeyCode::PageDown => app.page_down(10),
        KeyCode::PageUp => app.page_up(10),

        KeyCode::Enter => app.overlay = Some(Overlay::Detail),

        KeyCode::Char('d') => {
            app.load_diff();
            app.overlay = Some(Overlay::Diff);
        }

        KeyCode::Char('?') => app.overlay = Some(Overlay::Help),

        KeyCode::Char('s') => {
            app.sort_mode = app.sort_mode.next();
            app.sort();
        }

        KeyCode::Char('f') => {
            app.fetch = !app.fetch;
            app.start_scan();
        }

        KeyCode::Char('r') => app.start_scan(),

        KeyCode::Char('/') => {
            app.input_mode = InputMode::Filter;
        }

        KeyCode::Esc => {
            if !app.filter.is_empty() {
                app.filter.clear();
                app.rebuild_filtered();
            }
        }

        KeyCode::Char('c') => {
            app.show_header = !app.show_header;
        }

        // ── AOSP-only keys ──────────────────────────────────────────────────
        KeyCode::Char('F') if app.is_aosp => {
            app.launch_aosp_op(AospOp::RepoSync, None);
        }
        KeyCode::Char('n') if app.is_aosp => {
            app.launch_aosp_op(AospOp::RepoSyncN, None);
        }
        KeyCode::Char('T') if app.is_aosp => {
            app.launch_aosp_op(AospOp::RepoStatus, None);
        }
        KeyCode::Char('b') if app.is_aosp => {
            app.launch_aosp_op(AospOp::RepoBranches, None);
        }
        KeyCode::Char('o') if app.is_aosp => {
            app.launch_aosp_op(AospOp::RepoOverview, None);
        }
        KeyCode::Char('m') if app.is_aosp => {
            app.launch_aosp_op(AospOp::RepoManifest, None);
        }
        KeyCode::Char('D') if app.is_aosp => {
            app.start_aosp_prompt(AospOp::RepoDownload);
        }
        KeyCode::Char('M') if app.is_aosp => {
            app.launch_aosp_op(AospOp::MakeBuild, None);
        }
        KeyCode::Char('C') if app.is_aosp => {
            app.aosp_op = Some(AospOp::MakeClean);
            app.start_aosp_confirm(AospOp::MakeClean);
        }
        KeyCode::Char('B') if app.is_aosp => {
            app.start_aosp_prompt(AospOp::RepoStart);
        }
        KeyCode::Char('A') if app.is_aosp => {
            app.aosp_op = Some(AospOp::RepoAbandon);
            app.start_aosp_confirm(AospOp::RepoAbandon);
        }
        KeyCode::Char(':') if app.is_aosp => {
            app.start_aosp_prompt(AospOp::RepoForall);
        }

        _ => {}
    }
}

fn atty_check() -> bool {
    use std::os::unix::io::AsRawFd;
    extern "C" { fn isatty(fd: i32) -> i32; }
    unsafe { isatty(io::stdout().as_raw_fd()) != 0 }
}
