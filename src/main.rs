//! grove — your git shortcuts as one portable binary. Thin passthroughs (status,
//! add, commit, pull, push) plus two commands with real logic: `overview` (a
//! multi-repo dashboard) and `sync` (auto pull/push across a folder of repos).
//! `grove init <shell>` prints the short aliases (gs, ga, gc, gcp, gp, gpp, lg,
//! lgp) for zsh/bash/fish — so one `brew install` gives you the same shortcuts in
//! any shell, and `brew upgrade` keeps every shell in sync from one source.
mod git;
mod init;
mod overview;
mod passthrough;
mod sync;
mod tree;
mod ui;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "grove",
    version,
    about = "Portable git shortcuts + a multi-repo overview & sync.",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// `git status` (passthrough, forwards extra args). Alias: gs
    Status {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// `git add` — stages `.` when no paths are given, else the paths you pass. Alias: ga
    Add {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        paths: Vec<String>,
    },
    /// `git commit -m <msg>`; --all stages tracked changes first, --push pushes after. Aliases: gc, gcp
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
    /// `git pull` (passthrough, forwards extra args). Alias: gp
    Pull {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// `git push` (passthrough, forwards extra args). Alias: gpp
    Push {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Multi-repo dashboard: fetch + branch/ahead-behind/status for every repo in a folder. Alias: lg
    Overview { dir: Option<PathBuf> },
    /// Auto pull/push the clean, in-sync repos in a folder, then show the overview. Alias: lgp
    Sync { dir: Option<PathBuf> },
    /// Tree view (dirs first, icons), 2 levels deep; git repos get a git icon. No external deps. Alias: lt
    Tree {
        dir: Option<PathBuf>,
        /// How many levels deep to descend.
        #[arg(short, long, default_value_t = 2)]
        level: usize,
        /// Show hidden entries (dotfiles) too.
        #[arg(short, long)]
        all: bool,
    },
    /// Print shell aliases (gs, ga, …). Add `eval "$(grove init zsh)"` (or bash) / `grove init fish | source`.
    Init { shell: init::Shell },
    /// Print a shell completion script (bash|zsh|fish|…) to stdout.
    #[command(hide = true)]
    Completions { shell: clap_complete::Shell },
    /// Print a man page (roff) to stdout.
    #[command(hide = true)]
    Man,
}

fn main() {
    if let Err(e) = run() {
        ui::err(&format!("{e}"));
        std::process::exit(1);
    }
}

/// Multi-call dispatch: when grove is invoked through a short-name symlink
/// (gst, ga, …), splice the matching subcommand in front of the args and let
/// clap parse the rest — so every entry point keeps full flag/help handling.
/// Invoked as `grove`, parse normally.
fn parse_cli() -> Cli {
    let mut args = std::env::args_os();
    let arg0 = args.next().unwrap_or_default();
    let rest: Vec<OsString> = args.collect();
    let prog = std::path::Path::new(&arg0)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let inject: &[&str] = match prog {
        "gst" => &["status"],
        "ga" => &["add"],
        "gc" => &["commit"],
        "gcp" => &["commit", "--all", "--push"],
        "gp" => &["pull"],
        "gpp" => &["push"],
        "lg" => &["overview"],
        "lgp" => &["sync"],
        "lt" => &["tree"],
        _ => &[],
    };
    if inject.is_empty() {
        Cli::parse()
    } else {
        let mut v: Vec<OsString> = vec![arg0];
        v.extend(inject.iter().map(OsString::from));
        v.extend(rest);
        Cli::parse_from(v)
    }
}

fn run() -> Result<()> {
    let cli = parse_cli();
    match cli.cmd {
        Cmd::Status { args } => passthrough::exec(&["status"], &args),
        Cmd::Add { paths } => {
            if paths.is_empty() {
                passthrough::exec(&["add", "."], &[])
            } else {
                passthrough::exec(&["add"], &paths)
            }
        }
        Cmd::Commit { all, push, message } => passthrough::commit(all, push, &message),
        Cmd::Pull { args } => passthrough::exec(&["pull"], &args),
        Cmd::Push { args } => passthrough::exec(&["push"], &args),
        Cmd::Overview { dir } => overview::run(dir.as_deref()),
        Cmd::Sync { dir } => sync::run(dir.as_deref()),
        Cmd::Tree { dir, level, all } => tree::run(dir.as_deref(), level, all),
        Cmd::Init { shell } => {
            init::print(shell);
            Ok(())
        }
        Cmd::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "grove", &mut std::io::stdout());
            Ok(())
        }
        Cmd::Man => {
            clap_mangen::Man::new(Cli::command()).render(&mut std::io::stdout())?;
            Ok(())
        }
    }
}
