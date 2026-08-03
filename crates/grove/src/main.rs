//! grove — the git-shortcut command suite. `grove` bundles the everyday git
//! verbs (status/add/commit/pull/push, each exec-ing git), the multi-repo tools
//! (overview/sync/push-all, each over a folder of repos), a self-contained tree
//! view, and the shell-alias setup (`grove setup`). It is a single binary: the
//! short names (gs/ga/gc/…, and lg/lgp/lgpp/lt for the multi-repo tools) are
//! opt-in shell aliases emitted by `grove setup`, so nothing short lands on
//! PATH to collide with other tools (notably `lg` vs lazygit): rename any that
//! clash in your grove file.
mod completions;
mod config;

use clap::{CommandFactory, Parser, Subcommand};
use std::path::PathBuf;

const REPO_URL: &str = "https://github.com/jakobhviid/grove";
const AFTER_HELP: &str = concat!(
    "Short aliases: `grove setup` installs gs/ga/gc/gcp/gp/gpp (git verbs) and lg/lgp/lgpp/lt (multi-repo tools).\n",
    "Repository: https://github.com/jakobhviid/grove (inspect the source there if needed)\n",
    "LLM guide: pass `--llm` for a full machine-readable reference (the whole command suite)."
);

#[derive(Parser)]
#[command(name = "grove", version, about = "Git shortcuts (status/add/commit/pull/push) + multi-repo tools + shell-alias setup.", after_help = AFTER_HELP, after_long_help = AFTER_HELP)]
struct Cli {
    /// Print the full LLM-readable guide (the whole command suite + repo link) and exit.
    // Deliberately NOT `global`: a global flag would let `grove commit fix the
    // --llm bug` swallow the message and print the guide instead of committing.
    // (The multi-repo verbs take a `--json` flag directly; the passthrough git
    // verbs can't, since they exec git and it owns stdout.)
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
    /// Switch the HTTPS remotes of every repo in a folder to SSH (so overview/sync/push-all can fetch them). Previews and asks before changing anything.
    Ssh {
        /// Folder of repositories (default: current directory).
        dir: Option<PathBuf>,
        /// Apply without the confirmation prompt (required for non-interactive use).
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },
    /// Multi-repo dashboard: branch, ahead/behind, dirty state, and a forge link per repo in a folder (alias: lg).
    Overview {
        /// Folder of repositories (default: current directory).
        dir: Option<PathBuf>,
        /// Emit the dashboard as JSON instead of the human table.
        #[arg(long)]
        json: bool,
    },
    /// Fast-forward-pull the behind repos and push the ahead ones (clean, in-sync only), then show the overview (alias: lgp).
    Sync {
        /// Folder of repositories (default: current directory).
        dir: Option<PathBuf>,
        /// Emit what was synced (and the overview) as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Push every repo in a folder that has unpushed commits (no pull), then show the overview (alias: lgpp).
    PushAll {
        /// Folder of repositories (default: current directory).
        dir: Option<PathBuf>,
        /// Emit what was pushed (and the overview) as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Tree view (dirs first, Nerd-Font icons); git repos get a git icon (alias: lt).
    Tree {
        /// Directory to list (default: current directory).
        dir: Option<PathBuf>,
        /// How many levels deep to descend.
        #[arg(short, long, default_value_t = 2)]
        level: usize,
        /// Show hidden entries (dotfiles) too.
        #[arg(short, long)]
        all: bool,
        /// Emit the tree as JSON instead of the human view.
        #[arg(long)]
        json: bool,
    },
    /// Provision your shell: write the grove file and add the load line to your rc (idempotent). Auto-detects the shell if omitted.
    Setup {
        shell: Option<config::Shell>,
        /// Reconcile aliases that differ from grove's defaults without prompting (for scripts/non-interactive use).
        #[arg(long)]
        force: bool,
    },
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
        None => print_suite_overview(),
        Some(Cmd::Status { args }) => run(grove_core::passthrough::status(&args)),
        Some(Cmd::Add { paths }) => run(grove_core::passthrough::add(&paths)),
        Some(Cmd::Commit { all, push, message }) => run(grove_core::passthrough::commit(all, push, &message)),
        Some(Cmd::Pull { args }) => run(grove_core::passthrough::pull(&args)),
        Some(Cmd::Push { args }) => run(grove_core::passthrough::push(&args)),
        Some(Cmd::Ssh { dir, yes }) => run(grove_core::remote::run(dir.as_deref(), yes)),
        Some(Cmd::Overview { dir, json }) => run(cmd_overview(dir, json)),
        Some(Cmd::Sync { dir, json }) => run(cmd_sync(dir, json)),
        Some(Cmd::PushAll { dir, json }) => run(cmd_push_all(dir, json)),
        Some(Cmd::Tree { dir, level, all, json }) => run(cmd_tree(dir, level, all, json)),
        Some(Cmd::Setup { shell, force }) => run(config::setup(shell, force)),
        Some(Cmd::Init { shell }) => config::init(shell),
        Some(Cmd::Example) => config::print_example(),
        Some(Cmd::Completions { shell }) => completions::emit(shell),
        Some(Cmd::Man) => {
            let _ = clap_mangen::Man::new(Cli::command()).render(&mut std::io::stdout());
        }
    }
}

/// The multi-repo dashboard (`grove overview`, alias `lg`): resolve the folder,
/// collect every repo's state, then render the human table or the JSON document.
fn cmd_overview(dir: Option<PathBuf>, json: bool) -> anyhow::Result<()> {
    let report = grove_core::overview::collect(dir.as_deref(), true)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        grove_core::overview::render_human(&report);
    }
    Ok(())
}

/// `grove sync` (alias `lgp`): pull/push the clean, in-sync repos, then render.
fn cmd_sync(dir: Option<PathBuf>, json: bool) -> anyhow::Result<()> {
    let report = grove_core::sync::run(dir.as_deref())?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        grove_core::sync::render_human(&report);
    }
    Ok(())
}

/// `grove push-all` (alias `lgpp`): push every ahead repo, then render.
fn cmd_push_all(dir: Option<PathBuf>, json: bool) -> anyhow::Result<()> {
    let report = grove_core::sync::push_all(dir.as_deref())?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        grove_core::sync::render_push(&report);
    }
    Ok(())
}

/// `grove tree` (alias `lt`): build the tree, then render the human view or JSON.
fn cmd_tree(dir: Option<PathBuf>, level: usize, all: bool, json: bool) -> anyhow::Result<()> {
    let report = grove_core::tree::collect(dir.as_deref(), level, all)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        grove_core::tree::render_human(&report);
    }
    Ok(())
}

/// Report an error through the shared red-✗ formatter and exit non-zero. The
/// passthrough git verbs normally exec-replace this process, so a returned `Err`
/// means git never took over (e.g. not a repo) — surface it and stop. `{e:#}`
/// prints the whole anyhow context chain, not just the outermost message.
fn run(r: anyhow::Result<()>) {
    if let Err(e) = r {
        grove_core::ui::err(&format!("{e:#}"));
        std::process::exit(1);
    }
}

/// A single self-contained, plain-text guide an LLM/agent can read to drive the
/// whole grove suite from zero: grove's own command reference (rendered from
/// clap, including every subcommand) followed by WORKFLOWS and the README, plus
/// the repo link.
fn llm_guide() -> String {
    let mut cmd = Cli::command();
    let mut out = String::new();
    out.push_str(&format!("grove {} — LLM guide\n", env!("CARGO_PKG_VERSION")));
    out.push_str(&format!("Repository: {REPO_URL}  (read the source there if you need behavior details)\n"));
    out.push_str("This is the same reference as `man grove`, laid out plainly for LLM reading.\n");
    out.push_str("grove is one binary: the git verbs (status/add/commit/pull/push) exec git; the\n");
    out.push_str("multi-repo tools (overview/sync/push-all) and the tree view work over a folder.\n");
    out.push_str("The short names (gs/ga/… and lg/lgp/lgpp/lt) are shell aliases from `grove setup`.\n");
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

    out.push_str("\n\n================================ ARCHITECTURE ================================\n\n");
    out.push_str(include_str!("../../../ARCHITECTURE.md"));
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
fn print_suite_overview() {
    use grove_core::ui::paint;
    let hdr = |s: &str| println!("\n{}", paint("1", s));
    // Pad the name to width *before* coloring, so ANSI codes don't skew columns.
    let row = |name: &str, desc: &str| println!("  {} {}", paint("36", &format!("{name:<22}")), desc);

    println!("{} — git shortcuts + multi-repo tools", paint("1;32", "grove"));

    hdr("EVERYDAY GIT  (grove subcommands — alias them short via `grove setup`)");
    row("grove status", "git status");
    row("grove add [paths]", "git add (defaults to \".\")");
    row("grove commit <msg>", "git commit -m   (-a stage all tracked, -p push after)");
    row("grove pull", "git pull");
    row("grove push", "git push");

    hdr("MULTI-REPO  (subcommands — run in a folder of repos; alias in parens)");
    row("grove overview [dir]", "dashboard: branch, ahead/behind, dirty state per repo  (lg)");
    row("grove sync [dir]", "auto pull/push the clean, in-sync repos, then overview  (lgp)");
    row("grove push-all [dir]", "push every repo with unpushed commits (no pull)  (lgpp)");
    row("grove ssh [dir]", "switch HTTPS remotes to SSH (previews & asks first)");

    hdr("FILES");
    row("grove tree [dir] [-a]", "tree view; git repos get a git icon  (lt)");

    hdr("SHELL ALIASES  (short names — gs ga gc gcp gp gpp, and lg lgp lgpp lt)");
    row("grove setup [sh]", "provision your shell: writes the grove file + rc line (one-stop)");
    row("grove init <sh>", "just print the alias lines (for eval / manual or scripted setup)");
    row("grove example", "print a starter grove file");

    println!("\n{}", paint("90", "Aliases are yours to edit — rename any that clash on your system (e.g. `lg` if you use lazygit)."));
    println!("{}", paint("90", "Machine-readable: `grove overview|sync|push-all|tree --json`, or `grove --llm` for the full guide."));
}
