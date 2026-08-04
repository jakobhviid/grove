//! grove — the git-shortcut command suite. `grove` bundles the everyday git
//! verbs (status/add/commit/pull/push, each exec-ing git), the multi-repo tools
//! (overview/sync/pull-all/push-all, each over a folder of repos), a
//! self-contained tree view, the shell-alias setup (`grove setup`), and its own
//! settings (`grove configure`). It is a single binary: the short names
//! (gs/ga/gc/…, and lg/lgs/lgp/lgpp/lt for the multi-repo tools) are opt-in shell
//! aliases emitted by `grove setup`, so nothing short lands on PATH to collide
//! with other tools (notably `lg` vs lazygit): rename any that clash in your
//! grove file.
mod cache;
mod completions;
mod config;
mod settings;

use clap::{CommandFactory, Parser, Subcommand};
use std::path::{Path, PathBuf};

const REPO_URL: &str = "https://github.com/jakobhviid/grove";
const AFTER_HELP: &str = concat!(
    "Short aliases: `grove setup` installs gs/ga/gc/gcp/gp/gpp (git verbs) and lg/lgs/lgp/lgpp/lt (multi-repo tools).\n",
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
    /// Switch the HTTPS remotes of every repo in a folder to SSH (so overview/sync/pull-all/push-all can fetch them). Previews and asks before changing anything.
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
        /// Re-fetch every repo, bypassing the per-repo cache.
        #[arg(short, long)]
        force: bool,
    },
    /// Fast-forward-pull the behind repos and push the ahead ones (clean, in-sync only), then show the overview (alias: lgs).
    Sync {
        /// Folder of repositories (default: current directory).
        dir: Option<PathBuf>,
        /// Emit what was synced (and the overview) as JSON.
        #[arg(long)]
        json: bool,
        /// Re-fetch every repo, bypassing the per-repo cache.
        #[arg(short, long)]
        force: bool,
    },
    /// Fast-forward every repo in a folder that is behind its upstream (no push), then show the overview (alias: lgp).
    PullAll {
        /// Folder of repositories (default: current directory).
        dir: Option<PathBuf>,
        /// Emit what was pulled (and the overview) as JSON.
        #[arg(long)]
        json: bool,
        /// Re-fetch every repo, bypassing the per-repo cache.
        #[arg(short, long)]
        force: bool,
    },
    /// Push every repo in a folder that has unpushed commits (no pull), then show the overview (alias: lgpp).
    PushAll {
        /// Folder of repositories (default: current directory).
        dir: Option<PathBuf>,
        /// Emit what was pushed (and the overview) as JSON.
        #[arg(long)]
        json: bool,
        /// Re-fetch every repo, bypassing the per-repo cache.
        #[arg(short, long)]
        force: bool,
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
    /// Get or set grove's settings in ~/.config/grove/config (cache, cache_ttl, default_dir). No args lists them all.
    Configure {
        /// Setting name (omit to list every setting and its value).
        key: Option<String>,
        /// New value (omit to just read the current value).
        value: Option<String>,
    },
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
        Some(Cmd::Ssh { dir, yes }) => run(grove_core::remote::run(dir.as_deref(), yes, &hints())),
        Some(Cmd::Overview { dir, json, force }) => run(cmd_overview(dir, json, force)),
        Some(Cmd::Sync { dir, json, force }) => run(cmd_sync(dir, json, force)),
        Some(Cmd::PullAll { dir, json, force }) => run(cmd_pull_all(dir, json, force)),
        Some(Cmd::PushAll { dir, json, force }) => run(cmd_push_all(dir, json, force)),
        Some(Cmd::Tree { dir, level, all, json }) => run(cmd_tree(dir, level, all, json)),
        Some(Cmd::Setup { shell, force }) => run(config::setup(shell, force)),
        Some(Cmd::Init { shell }) => config::init(shell),
        Some(Cmd::Example) => config::print_example(),
        Some(Cmd::Configure { key, value }) => run(settings::configure(key, value)),
        Some(Cmd::Completions { shell }) => completions::emit(shell),
        Some(Cmd::Man) => {
            let _ = clap_mangen::Man::new(Cli::command()).render(&mut std::io::stdout());
        }
    }
}

/// The next-step hints for the human dashboard, resolved to the aliases the user
/// actually bound in their grove file (or the long `grove …` forms, plus a
/// `grove setup` nudge, when they haven't provisioned any).
fn hints() -> grove_core::overview::Hints {
    grove_core::overview::Hints {
        pull_all: config::alias_for("grove pull-all"),
        push_all: config::alias_for("grove push-all"),
        ssh: config::alias_for("grove ssh"),
        configured: config::is_configured(),
    }
}

/// Collect the dashboard, letting the per-repo cache decide which repos to fetch.
/// With the cache on (and no `--force`), a repo left settled by a recent fetch is
/// skipped; everything with pending work still fetches. After collecting we record
/// each *fetched* repo's settled state (skipped ones keep their earlier stamp, so
/// staleness stays bounded to the TTL). `--force` / cache-off fetch everything.
fn fetch_collect(dir: Option<&Path>, force: bool, s: &settings::Settings) -> anyhow::Result<grove_core::overview::Report> {
    use grove_core::overview::Fetch;
    let ttl = s.ttl();
    let want = move |repo: &Path| !cache::settled_within(repo, ttl);
    let mode = if force || !s.cache { Fetch::All } else { Fetch::Cache(&want) };
    let report = grove_core::overview::collect(dir, mode)?;
    if s.cache {
        mark_cache(&report);
    }
    Ok(report)
}

/// Update the per-repo cache from a freshly-collected report: a repo we fetched
/// (not `cached`, not https) is stamped settled when calm, else its stamp is
/// dropped so it keeps re-fetching. Repos served from cache are left untouched.
fn mark_cache(report: &grove_core::overview::Report) {
    for r in &report.repos {
        if r.https || r.cached {
            continue;
        }
        if r.calm() {
            cache::mark_settled(Path::new(&r.path));
        } else {
            cache::mark_unsettled(Path::new(&r.path));
        }
    }
}

/// When no folder is given and the current directory has nothing to do with git
/// (not inside a repo, and no immediate sub-repo to list), fall back to the
/// configured `default_dir` — with a dim note so it's never a silent surprise. An
/// explicit folder argument always wins, and with no `default_dir` set nothing
/// changes (core defaults to `.`).
fn resolve_dir(dir: Option<PathBuf>, settings: &settings::Settings) -> Option<PathBuf> {
    if dir.is_some() {
        return dir;
    }
    let default = settings.default_dir.as_ref()?;
    if grove_core::git::inside_repo() || !grove_core::git::discover(Path::new(".")).is_empty() {
        return None;
    }
    grove_core::ui::note(&format!("no git repos here — showing {} (default_dir)", settings::tildify(default)));
    Some(default.clone())
}

/// The multi-repo dashboard (`grove overview`, alias `lg`): resolve the folder,
/// collect every repo's state, then render the human table or the JSON document.
fn cmd_overview(dir: Option<PathBuf>, json: bool, force: bool) -> anyhow::Result<()> {
    let s = settings::load();
    let dir = resolve_dir(dir, &s);
    let report = fetch_collect(dir.as_deref(), force, &s)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        grove_core::overview::render_human(&report, &hints());
    }
    Ok(())
}

/// `grove sync` (alias `lgs`): pull/push the clean, in-sync repos, then re-read
/// (no fetch) and render the post-action dashboard.
fn cmd_sync(dir: Option<PathBuf>, json: bool, force: bool) -> anyhow::Result<()> {
    use grove_core::overview::Fetch;
    let s = settings::load();
    let dir = resolve_dir(dir, &s);
    let report = fetch_collect(dir.as_deref(), force, &s)?;
    let synced = grove_core::sync::act_sync(&report);
    let overview = grove_core::overview::collect(dir.as_deref(), Fetch::None)?;
    let out = grove_core::sync::SyncReport { synced, overview };
    if json {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        grove_core::sync::render_human(&out, &hints());
    }
    Ok(())
}

/// `grove pull-all` (alias `lgp`): fast-forward every behind repo, then render.
fn cmd_pull_all(dir: Option<PathBuf>, json: bool, force: bool) -> anyhow::Result<()> {
    use grove_core::overview::Fetch;
    let s = settings::load();
    let dir = resolve_dir(dir, &s);
    let report = fetch_collect(dir.as_deref(), force, &s)?;
    let pulled = grove_core::sync::act_pull_all(&report);
    let overview = grove_core::overview::collect(dir.as_deref(), Fetch::None)?;
    let out = grove_core::sync::PullReport { pulled, overview };
    if json {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        grove_core::sync::render_pull(&out, &hints());
    }
    Ok(())
}

/// `grove push-all` (alias `lgpp`): push every ahead repo, then render.
fn cmd_push_all(dir: Option<PathBuf>, json: bool, force: bool) -> anyhow::Result<()> {
    use grove_core::overview::Fetch;
    let s = settings::load();
    let dir = resolve_dir(dir, &s);
    let report = fetch_collect(dir.as_deref(), force, &s)?;
    let pushed = grove_core::sync::act_push_all(&report);
    let overview = grove_core::overview::collect(dir.as_deref(), Fetch::None)?;
    let out = grove_core::sync::PushReport { pushed, overview };
    if json {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        grove_core::sync::render_push(&out, &hints());
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
    out.push_str("multi-repo tools (overview/sync/pull-all/push-all) and the tree view work over a folder.\n");
    out.push_str("The short names (gs/ga/… and lg/lgs/lgp/lgpp/lt) are shell aliases from `grove setup`.\n");
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
    // The alias the user bound to a verb (e.g. `lg`), else grove's default name —
    // so the parens name what you'd actually type on this machine.
    let alias = |cmd: &str, default: &str| config::alias_for(cmd).unwrap_or_else(|| default.to_string());
    let configured = config::is_configured();

    println!("{} — git shortcuts + multi-repo tools", paint("1;32", "grove"));

    hdr("EVERYDAY GIT  (grove subcommands — alias them short via `grove setup`)");
    row("grove status", "git status");
    row("grove add [paths]", "git add (defaults to \".\")");
    row("grove commit <msg>", "git commit -m   (-a stage all tracked, -p push after)");
    row("grove pull", "git pull");
    row("grove push", "git push");

    hdr("MULTI-REPO  (subcommands — run in a folder of repos; alias in parens)");
    row("grove overview [dir]", &format!("dashboard: branch, ahead/behind, dirty state per repo  ({})", alias("grove overview", "lg")));
    row("grove sync [dir]", &format!("pull the behind + push the ahead clean repos, then overview  ({})", alias("grove sync", "lgs")));
    row("grove pull-all [dir]", &format!("fast-forward every behind repo (no push)  ({})", alias("grove pull-all", "lgp")));
    row("grove push-all [dir]", &format!("push every repo with unpushed commits (no pull)  ({})", alias("grove push-all", "lgpp")));
    row("grove ssh [dir]", "switch HTTPS remotes to SSH (previews & asks first)");

    hdr("FILES");
    row("grove tree [dir] [-a]", &format!("tree view; git repos get a git icon  ({})", alias("grove tree", "lt")));

    hdr("SHELL ALIASES  (short names — gs ga gc gcp gp gpp, and lg lgs lgp lgpp lt)");
    row("grove setup [sh]", "provision your shell: writes the grove file + rc line (one-stop)");
    row("grove init <sh>", "just print the alias lines (for eval / manual or scripted setup)");
    row("grove example", "print a starter grove file");

    hdr("SETTINGS");
    row("grove configure", "get/set settings: cache, cache_ttl, default_dir (no args lists them)");

    println!();
    if configured {
        println!("{}", paint("90", "Aliases are yours to edit — rename any that clash on your system (e.g. `lg` if you use lazygit)."));
    } else {
        println!("{}", paint("90", "Short aliases aren't installed yet — run `grove setup` to enable lg lgs lgp lgpp lt (and gs ga gc …)."));
    }
    println!("{}", paint("90", "Machine-readable: `grove overview|sync|pull-all|push-all|tree --json`, or `grove --llm` for the full guide."));
}
