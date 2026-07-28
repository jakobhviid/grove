//! grove — the git-shortcut command suite. `grove` bundles the everyday git
//! verbs as subcommands (status/add/commit/pull/push, each exec-ing git) and the
//! shell-alias setup (`grove init`). The multi-repo/tree tools — lg, lgp, lgpp,
//! lt — are separate binaries. Short names for the git verbs (gs, ga, gc, …) are
//! opt-in aliases emitted by `grove init`, so nothing short lands on PATH to
//! collide with other tools: rename any that clash in your grove file.
mod completions;
mod config;

use clap::{CommandFactory, Parser, Subcommand};
use std::path::PathBuf;

const REPO_URL: &str = "https://github.com/jakobhviid/grove";
const AFTER_HELP: &str = concat!(
    "Short aliases: `grove init <shell>` emits gs/ga/gc/gcp/gp/gpp for the git verbs.\n",
    "Repository: https://github.com/jakobhviid/grove (inspect the source there if needed)\n",
    "LLM guide: pass `--llm` for a full machine-readable reference (the whole command suite)."
);

#[derive(Parser)]
#[command(name = "grove", version, about = "Git shortcuts (status/add/commit/pull/push) + shell-alias setup.", after_help = AFTER_HELP, after_long_help = AFTER_HELP)]
struct Cli {
    /// Print the full LLM-readable guide (the whole command suite + repo link) and exit.
    // Deliberately NOT `global`: a global flag would let `grove commit fix the
    // --llm bug` swallow the message and print the guide instead of committing.
    #[arg(long)]
    llm: bool,
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// git status — forwards any extra args to git.
    Status {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// git add — stages "." when given no paths, else the paths you pass.
    Add {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        paths: Vec<String>,
    },
    /// git commit -m <msg>; -a stages tracked changes first, -p pushes after.
    Commit {
        /// Stage all tracked changes first (git commit -a).
        #[arg(short, long)]
        all: bool,
        /// Push after a successful commit.
        #[arg(short, long)]
        push: bool,
        /// Commit message (all words joined into one message).
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        message: Vec<String>,
    },
    /// git pull — forwards any extra args to git.
    Pull {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// git push — forwards any extra args to git.
    Push {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Switch the HTTPS remotes of every repo in a folder to SSH (so lg/lgp/lgpp can fetch them). Previews and asks before changing anything.
    Ssh {
        /// Folder of repositories (default: current directory).
        dir: Option<PathBuf>,
        /// Apply without the confirmation prompt (required for non-interactive use).
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },
    /// Provision your shell: write the grove file and add the load line to your rc (idempotent). Auto-detects the shell if omitted.
    Setup { shell: Option<config::Shell> },
    /// Print shell aliases from your grove file (~/.config/grove/aliases) for eval. Add `eval "$(grove init zsh)"`.
    Init { shell: config::Shell },
    /// Print a starter grove file you can save to ~/.config/grove/aliases.
    Example,
    /// Print a shell completion script (bash|zsh|fish|…) for the grove suite to stdout.
    #[command(hide = true)]
    Completions { shell: clap_complete::Shell },
    /// Print grove's man page (roff) to stdout.
    #[command(hide = true)]
    Man,
}

fn main() {
    grove_core::reset_sigpipe();
    let cli = Cli::parse();
    if cli.llm {
        print!("{}", llm_guide());
        return;
    }
    match cli.cmd {
        None => overview(),
        Some(Cmd::Status { args }) => run(grove_core::passthrough::status(&args)),
        Some(Cmd::Add { paths }) => run(grove_core::passthrough::add(&paths)),
        Some(Cmd::Commit { all, push, message }) => run(grove_core::passthrough::commit(all, push, &message)),
        Some(Cmd::Pull { args }) => run(grove_core::passthrough::pull(&args)),
        Some(Cmd::Push { args }) => run(grove_core::passthrough::push(&args)),
        Some(Cmd::Ssh { dir, yes }) => run(grove_core::remote::run(dir.as_deref(), yes)),
        Some(Cmd::Setup { shell }) => run(config::setup(shell)),
        Some(Cmd::Init { shell }) => config::init(shell),
        Some(Cmd::Example) => config::print_example(),
        Some(Cmd::Completions { shell }) => completions::emit(shell),
        Some(Cmd::Man) => {
            let _ = clap_mangen::Man::new(Cli::command()).render(&mut std::io::stdout());
        }
    }
}

/// Report an error through the shared red-✗ formatter and exit non-zero. The
/// passthrough git verbs normally exec-replace this process, so a returned `Err`
/// means git never took over (e.g. not a repo) — surface it and stop.
fn run(r: anyhow::Result<()>) {
    if let Err(e) = r {
        grove_core::ui::err(&e.to_string());
        std::process::exit(1);
    }
}

/// A single self-contained, plain-text guide an LLM/agent can read to drive the
/// whole grove suite from zero: grove's own command reference (rendered from clap,
/// including each git-verb subcommand) followed by the README, plus the repo link.
fn llm_guide() -> String {
    let mut cmd = Cli::command();
    let mut out = String::new();
    out.push_str(&format!("grove {} — LLM guide\n", env!("CARGO_PKG_VERSION")));
    out.push_str(&format!("Repository: {REPO_URL}  (read the source there if you need behavior details)\n"));
    out.push_str("This is the same reference as `man grove`, laid out plainly for LLM reading.\n");
    out.push_str("grove bundles the git verbs (status/add/commit/pull/push); the multi-repo/tree\n");
    out.push_str("tools (lg, lgp, lgpp, lt) are separate binaries — run `<cmd> --help` for any one.\n");
    out.push_str("Full documentation follows.\n\n");

    out.push_str("================================ grove COMMAND REFERENCE ================================\n\n");
    out.push_str(&cmd.render_long_help().to_string());
    for sub in cmd.get_subcommands_mut() {
        if sub.is_hide_set() {
            continue;
        }
        out.push_str(&format!("\n\n-------------------------------- grove {} --------------------------------\n\n", sub.get_name()));
        out.push_str(&sub.render_long_help().to_string());
    }

    out.push_str("\n\n================================ WORKFLOWS ================================\n\n");
    out.push_str(include_str!("../../../WORKFLOWS.md"));
    out.push_str("\n\n================================ GUIDE (README) ================================\n\n");
    out.push_str(include_str!("../../../README.md"));
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// The friendly listing shown when `grove` is run with no arguments.
fn overview() {
    use grove_core::ui::paint;
    let hdr = |s: &str| println!("\n{}", paint("1", s));
    // Pad the name to width *before* coloring, so ANSI codes don't skew columns.
    let row = |name: &str, desc: &str| println!("  {} {}", paint("36", &format!("{name:<22}")), desc);

    println!("{} — git shortcuts + multi-repo tools", paint("1;32", "grove"));

    hdr("EVERYDAY GIT  (grove subcommands — alias them short via `grove init`)");
    row("grove status", "git status");
    row("grove add [paths]", "git add (defaults to \".\")");
    row("grove commit <msg>", "git commit -m   (-a stage all tracked, -p push after)");
    row("grove pull", "git pull");
    row("grove push", "git push");

    hdr("MULTI-REPO  (standalone commands — run in a folder of repos)");
    row("lg [dir]", "dashboard: branch, ahead/behind, dirty state per repo");
    row("lgp [dir]", "auto pull/push the clean, in-sync repos, then show lg");
    row("lgpp [dir]", "push every repo with unpushed commits (no pull)");
    row("grove ssh [dir]", "switch HTTPS remotes to SSH (previews & asks first)");

    hdr("FILES");
    row("lt [dir] [-a]", "tree view; git repos get a git icon");

    hdr("SHELL ALIASES  (short names for the git verbs — gs ga gc gcp gp gpp)");
    row("grove setup [sh]", "provision your shell: writes the grove file + rc line (one-stop)");
    row("grove init <sh>", "just print the alias lines (for eval / manual or scripted setup)");
    row("grove example", "print a starter grove file");

    println!("\n{}", paint("90", "Rename any alias that clashes on your system (e.g. gc) — they're yours to edit."));
}
