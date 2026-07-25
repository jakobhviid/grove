//! grove — the init/config tool for the grove command suite. The commands
//! themselves (gst, ga, gc, gp, gpp, lg, lgp, lt) are separate binaries that
//! work with no setup; grove just (1) prints a friendly overview when run bare,
//! and (2) turns your grove file into shell aliases via `grove init`.
mod config;

use clap::{CommandFactory, Parser, Subcommand};

const REPO_URL: &str = "https://github.com/jakobhviid/grove";
const AFTER_HELP: &str = concat!(
    "Repository: https://github.com/jakobhviid/grove (inspect the source there if needed)\n",
    "LLM guide: pass `--llm` for a full machine-readable reference (the whole command suite)."
);

#[derive(Parser)]
#[command(name = "grove", version, about = "The grove command suite — overview and shell-alias setup.", after_help = AFTER_HELP, after_long_help = AFTER_HELP)]
struct Cli {
    /// Print the full LLM-readable guide (the whole command suite + repo link) and exit.
    #[arg(long, global = true)]
    llm: bool,
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print shell aliases from your grove file (~/.config/grove/aliases). Add `eval "$(grove init zsh)"`.
    Init { shell: config::Shell },
    /// Print a starter grove file you can save to ~/.config/grove/aliases.
    Example,
    /// Print a shell completion script (bash|zsh|fish|…) for `grove` to stdout.
    #[command(hide = true)]
    Completions { shell: clap_complete::Shell },
    /// Print grove's man page (roff) to stdout.
    #[command(hide = true)]
    Man,
}

fn main() {
    grove_core::reset_sigpipe();
    // `--llm` is a documentation flag like `--help`: works from anywhere.
    if std::env::args().skip(1).any(|a| a == "--llm") {
        print!("{}", llm_guide());
        return;
    }
    let cli = Cli::parse();
    match cli.cmd {
        None => overview(),
        Some(Cmd::Init { shell }) => config::init(shell),
        Some(Cmd::Example) => config::print_example(),
        Some(Cmd::Completions { shell }) => {
            clap_complete::generate(shell, &mut Cli::command(), "grove", &mut std::io::stdout());
        }
        Some(Cmd::Man) => {
            let _ = clap_mangen::Man::new(Cli::command()).render(&mut std::io::stdout());
        }
    }
}

/// A single self-contained, plain-text guide an LLM/agent can read to drive the
/// whole grove suite from zero: grove's own command reference (rendered from clap)
/// followed by the README (which documents every sub-binary), plus the repo link.
fn llm_guide() -> String {
    let mut cmd = Cli::command();
    let mut out = String::new();
    out.push_str(&format!("grove {} — LLM guide\n", env!("CARGO_PKG_VERSION")));
    out.push_str(&format!("Repository: {REPO_URL}  (read the source there if you need behavior details)\n"));
    out.push_str("This is the same reference as `man grove`, laid out plainly for LLM reading.\n");
    out.push_str("The suite is separate binaries (gst, ga, gc, gp, gpp, lg, lgp, lgpp, lt);\n");
    out.push_str("run `<cmd> --help` for any one. Full documentation follows.\n\n");

    out.push_str("================================ grove COMMAND REFERENCE ================================\n\n");
    out.push_str(&cmd.render_long_help().to_string());
    for sub in cmd.get_subcommands_mut() {
        if sub.is_hide_set() {
            continue;
        }
        out.push_str(&format!("\n\n-------------------------------- grove {} --------------------------------\n\n", sub.get_name()));
        out.push_str(&sub.render_long_help().to_string());
    }

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
    let row = |name: &str, desc: &str| println!("  {} {}", paint("36", &format!("{name:<16}")), desc);

    println!("{} — git shortcuts + multi-repo tools, as separate binaries for any shell", paint("1;32", "grove"));

    hdr("EVERYDAY GIT");
    row("gst", "git status");
    row("ga [paths]", "git add (defaults to \".\")");
    row("gc <msg>", "git commit -m   (-a stage all tracked, -p push after)");
    row("gp", "git pull");
    row("gpp", "git push");

    hdr("MULTI-REPO  (run in a folder of repos)");
    row("lg [dir]", "dashboard: branch, ahead/behind, dirty state per repo");
    row("lgp [dir]", "auto pull/push the clean, in-sync repos, then show lg");
    row("lgpp [dir]", "push every repo with unpushed commits (no pull)");

    hdr("FILES");
    row("lt [dir] [-a]", "tree view; git repos get a git icon");

    hdr("SHELL ALIASES  (optional)");
    row("grove init <sh>", "emit aliases from ~/.config/grove/aliases  (zsh|bash|fish)");
    row("", &paint("90", "e.g.  eval \"$(grove init zsh)\"   → gs, gcp, and your own"));
    row("grove example", "print a starter grove file");

    println!("\n{}", paint("90", "Every command works on its own — run `<cmd> --help` for details."));
}
