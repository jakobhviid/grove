//! The grove file: `~/.config/grove/aliases`, shell-agnostic `name = command`
//! lines. `grove init <shell>` translates it into that shell's alias syntax. If
//! no file exists, a built-in default set (gs/ga/gc/gcp/gp/gpp → the git verbs)
//! is emitted, so it works out of the box.
use clap::ValueEnum;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, ValueEnum)]
pub enum Shell {
    Zsh,
    Bash,
    Fish,
}

/// Built-in defaults emitted when there's no grove file yet: short names for the
/// git verbs and the multi-repo/tree tools, mapped to `grove` subcommands. These
/// are aliases, not binaries, so nothing short lands on PATH to collide with
/// other tools (notably `lg` vs lazygit) — and each only shadows at your
/// interactive prompt, never in scripts. Rename any that clash on your system
/// (e.g. `gc` or `lg`) by editing the grove file; that's the whole point.
const DEFAULTS: &[(&str, &str)] = &[
    ("gs", "grove status"),
    ("ga", "grove add"),
    ("gc", "grove commit"),
    ("gcp", "grove commit --all --push"),
    ("gp", "grove pull"),
    ("gpp", "grove push"),
    ("lg", "grove overview"),
    ("lgs", "grove sync"),
    ("lgp", "grove pull-all"),
    ("lgpp", "grove push-all"),
    ("lt", "grove tree"),
];

/// Read an environment variable as a path, treating unset **and empty** the
/// same (an empty `XDG_CONFIG_HOME`/`ZDOTDIR` must not become a relative path).
/// Shared with `settings`/`cache`, which key off the same env conventions.
pub(crate) fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// `~/.config/grove` (honoring `XDG_CONFIG_HOME`) — the directory holding both the
/// grove file (`aliases`) and the settings file (`config`).
pub(crate) fn config_dir() -> PathBuf {
    let base = env_path("XDG_CONFIG_HOME")
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"));
    base.join("grove")
}

fn config_path() -> PathBuf {
    config_dir().join("aliases")
}

fn aliases() -> Vec<(String, String)> {
    match std::fs::read_to_string(config_path()) {
        Ok(text) => parse(&text),
        Err(_) => DEFAULTS.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
    }
}

/// The aliases the user has actually configured, or `None` when there's no grove
/// file yet. Unlike [`aliases`], this does **not** fall back to the built-in
/// defaults: a missing file means the short aliases aren't active in any shell,
/// which is exactly what the listings/hints need to know to nudge `grove setup`.
fn configured_aliases() -> Option<Vec<(String, String)>> {
    std::fs::read_to_string(config_path()).ok().map(|t| parse(&t))
}

/// Whether a grove file exists — i.e. the short aliases have been provisioned.
pub(crate) fn is_configured() -> bool {
    config_path().exists()
}

/// The alias name the user bound to `command` (e.g. `grove push-all`) in their
/// grove file, or `None` if there's no file or nothing maps to it. Honors renames
/// (`gv = grove overview` resolves `grove overview` to `gv`), so the listings and
/// hints always show the name the user actually types.
pub(crate) fn alias_for(command: &str) -> Option<String> {
    configured_aliases()?.into_iter().find(|(_, c)| c == command).map(|(n, _)| n)
}

/// Default aliases whose *name* is absent from an existing grove file — the set
/// `setup` appends when topping up. A name already present (even remapped to a
/// different command) is left alone, so we never rewrite a line you've edited.
fn missing_defaults(text: &str) -> Vec<(&'static str, &'static str)> {
    let have: std::collections::HashSet<String> = parse(text).into_iter().map(|(n, _)| n).collect();
    DEFAULTS.iter().copied().filter(|(n, _)| !have.contains(*n)).collect()
}

/// Default aliases whose name is present in the file but bound to a *different*
/// command than grove's default — candidates for `setup` to reconcile. Returns
/// `(name, current command, default command)`.
fn divergent_defaults(text: &str) -> Vec<(&'static str, String, &'static str)> {
    let have = parse(text);
    DEFAULTS
        .iter()
        .filter_map(|(name, default)| {
            let cur = have.iter().find(|(n, _)| n == name)?;
            (cur.1 != *default).then(|| (*name, cur.1.clone(), *default))
        })
        .collect()
}

/// Rewrite the single `name = …` line to grove's default command, preserving the
/// original left-hand side (name + spacing) so the file's formatting survives.
fn override_alias(text: &str, name: &str, default: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        match line.split_once('=') {
            Some((lhs, _)) if lhs.trim() == name => out.push(format!("{lhs}= {default}")),
            _ => out.push(line.to_string()),
        }
    }
    let mut joined = out.join("\n");
    if text.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

/// Ask a yes/no question on the terminal, defaulting to no. Returns `false`
/// without prompting when stdin/stdout isn't a TTY (e.g. piped in a script) —
/// there `--force` is the way to apply changes non-interactively.
fn confirm(question: &str) -> bool {
    use std::io::Write;
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return false;
    }
    print!("  {question} [y/N] ");
    let _ = io::stdout().flush();
    let mut answer = String::new();
    if io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim(), "y" | "Y" | "yes" | "Yes")
}

/// Parse `name = command` lines; ignore blanks and `#` comments. Shared with the
/// settings file (`settings.rs`), which uses the same `key = value` shape.
pub(crate) fn parse(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (name, cmd) = line.split_once('=')?;
            let (name, cmd) = (name.trim(), cmd.trim());
            (!name.is_empty() && !cmd.is_empty()).then(|| (name.to_string(), cmd.to_string()))
        })
        .collect()
}

fn name_of(shell: Shell) -> &'static str {
    match shell {
        Shell::Zsh => "zsh",
        Shell::Bash => "bash",
        Shell::Fish => "fish",
    }
}

fn alias_line(shell: Shell, name: &str, cmd: &str) -> String {
    match shell {
        Shell::Fish => format!("alias {name} '{cmd}'"),
        Shell::Zsh | Shell::Bash => format!("alias {name}='{cmd}'"),
    }
}

fn activate_hint(shell: Shell) -> String {
    match shell {
        Shell::Fish => "grove init fish | source".to_string(),
        s => format!("eval \"$(grove init {})\"", name_of(s)),
    }
}

pub fn init(shell: Shell) {
    let items = aliases();
    let lines: Vec<String> = items.iter().map(|(n, c)| alias_line(shell, n, c)).collect();

    // Being eval'd / piped / redirected (not a TTY): emit ONLY shell code, and
    // nothing else — this runs on every shell startup, so it must stay silent
    // and pure. This is the path `eval "$(grove init zsh)"` takes.
    if !io::stdout().is_terminal() {
        for l in &lines {
            println!("{l}");
        }
        return;
    }

    // A human ran it in a terminal: explain what it does rather than dumping raw
    // `alias` lines with no context.
    use grove_core::ui::paint;
    let path = config_path();
    let source = if path.exists() {
        path.display().to_string()
    } else {
        "built-in defaults — no grove file yet (run `grove example > ~/.config/grove/aliases`)".to_string()
    };

    println!("{} would set up {} {} for {}:", paint("1;32", "grove init"), paint("1", &items.len().to_string()), if items.len() == 1 { "alias" } else { "aliases" }, paint("1", name_of(shell)));
    println!("{}", paint("90", &format!("  source: {source}")));
    println!();
    if items.is_empty() {
        println!("  (none defined)");
    } else {
        let w = items.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
        for (n, c) in &items {
            println!("  {}  →  {}", paint("36", &format!("{n:<w$}")), c);
        }
    }
    println!();
    println!("It doesn't change anything on its own — it prints shell code to evaluate.");
    println!("To activate, add this to your shell startup file:");
    println!("  {}", paint("1", &activate_hint(shell)));
    println!("{}", paint("90", &format!("Or let grove wire it up for you (writes the grove file + this line): grove setup {}", name_of(shell))));
    println!("{}", paint("90", "(when run non-interactively, e.g. via eval, it prints only the alias lines.)"));
}

/// Marker that identifies grove's managed block in a shell rc file. `setup`
/// checks for it to stay idempotent — re-running never adds a second block.
const MARKER: &str = "# grove — shell integration";

/// `grove setup [shell]`: the one-stop provisioner. Writes the grove file if
/// it's missing and appends an idempotent, marker-delimited block to the shell's
/// rc that loads the aliases via `grove init` on every startup. Re-running is a
/// no-op once the block is present. `init` stays the pure emitter this block
/// calls; `setup` is the only thing that edits your files.
pub fn setup(shell: Option<Shell>, force: bool) -> anyhow::Result<()> {
    use grove_core::ui::paint;
    let shell = shell
        .or_else(detect_shell)
        .ok_or_else(|| anyhow::anyhow!("couldn't detect your shell from $SHELL — run `grove setup zsh` (or bash/fish)"))?;
    let sh = name_of(shell);

    // 1) Materialize the editable grove file, then reconcile it. A brand-new file gets
    //    the full annotated template. An existing one is (a) topped up with any default
    //    whose *name* is missing (so a file predating a default — e.g. `gp` — self-heals),
    //    and (b) offered a fix for any default whose name is present but bound to a
    //    different command. We never silently rewrite an edited line: each divergence is
    //    confirmed interactively, or applied wholesale with `--force` for scripts.
    let cfg = config_path();
    let mut added: Vec<&'static str> = Vec::new();
    let mut overridden: Vec<&'static str> = Vec::new();
    let mut kept: Vec<&'static str> = Vec::new();
    let file_status = if cfg.exists() {
        let mut text = std::fs::read_to_string(&cfg)?;
        let mut changed = false;

        // (b) Reconcile divergent aliases first, so top-up sees the final name set.
        for (name, current, default) in divergent_defaults(&text) {
            let apply = force
                || confirm(&format!(
                    "{} is `{}` — reset to grove's default `{}`?",
                    paint("36", name),
                    paint("1", &current),
                    paint("1", default),
                ));
            if apply {
                text = override_alias(&text, name, default);
                overridden.push(name);
                changed = true;
            } else {
                kept.push(name);
            }
        }

        // (a) Top up the aliases this file never defined.
        let missing = missing_defaults(&text);
        if !missing.is_empty() {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str("\n# Added by `grove setup` — default git-verb aliases this file was missing:\n");
            let w = missing.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
            for (n, c) in &missing {
                text.push_str(&format!("{n:<w$} = {c}\n"));
                added.push(n);
            }
            changed = true;
        }

        if changed {
            std::fs::write(&cfg, text)?;
            "updated"
        } else {
            "exists"
        }
    } else {
        if let Some(dir) = cfg.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&cfg, EXAMPLE)?;
        "created"
    };

    // 2) Append the managed block to the rc file, unless our marker is already there.
    let rc = rc_path(shell)?;
    let existing = std::fs::read_to_string(&rc).unwrap_or_default();
    let rc_status = if existing.contains(MARKER) {
        "present"
    } else {
        if let Some(dir) = rc.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut content = existing;
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push('\n');
        content.push_str(&rc_block(shell));
        std::fs::write(&rc, content)?;
        "added"
    };

    println!("{} — {}", paint("1;32", "grove setup"), paint("1", sh));
    println!();
    println!("  {} {} {}", paint("36", "grove file"), paint("1", &format!("{file_status:<8}")), cfg.display());
    let note = |label: &str, names: &[&str]| {
        if !names.is_empty() {
            println!("  {} {} {label}: {}", paint("36", &format!("{:<10}", "")), paint("1", &format!("{:<8}", "")), paint("1", &names.join(" ")));
        }
    };
    note("topped up", &added);
    note("reset to default", &overridden);
    note("left as-is", &kept);
    let rc_desc = if rc_status == "present" {
        "already configured — no change".to_string()
    } else {
        format!("added the `grove init {sh}` line")
    };
    println!("  {} {} {}", paint("36", &format!("{:<10}", rc_name(shell))), paint("1", &format!("{rc_status:<8}")), rc_desc);
    println!();
    if file_status != "exists" || rc_status == "added" {
        println!("Reload your shell to activate:  {}", paint("1", &reload_hint(shell, &rc)));
        let names: Vec<String> = aliases().into_iter().map(|(n, _)| n).collect();
        println!("{}", paint("90", &format!("Aliases: {}", names.join(" "))));
    } else {
        println!("{}", paint("90", "Already set up — open a new shell if you haven't reloaded."));
    }
    Ok(())
}

/// Guess the shell from `$SHELL` (used when `grove setup` is run without an arg).
fn detect_shell() -> Option<Shell> {
    let sh = std::env::var("SHELL").ok()?;
    let base = Path::new(&sh).file_name()?.to_string_lossy().into_owned();
    if base.contains("zsh") {
        Some(Shell::Zsh)
    } else if base.contains("fish") {
        Some(Shell::Fish)
    } else if base.contains("bash") {
        Some(Shell::Bash)
    } else {
        None
    }
}

/// The startup file `setup` writes to for each shell.
fn rc_path(shell: Shell) -> anyhow::Result<PathBuf> {
    let home = env_path("HOME").ok_or_else(|| anyhow::anyhow!("HOME is not set"))?;
    Ok(match shell {
        Shell::Zsh => env_path("ZDOTDIR").unwrap_or(home).join(".zshrc"),
        Shell::Bash => home.join(".bashrc"),
        Shell::Fish => env_path("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config")).join("fish").join("config.fish"),
    })
}

fn rc_name(shell: Shell) -> &'static str {
    match shell {
        Shell::Zsh => ".zshrc",
        Shell::Bash => ".bashrc",
        Shell::Fish => "config.fish",
    }
}

/// The marker comment + a guarded load line (a no-op if `grove` isn't on PATH,
/// so removing grove later doesn't spam errors at shell startup).
fn rc_block(shell: Shell) -> String {
    let load = match shell {
        Shell::Fish => "command -v grove >/dev/null 2>&1; and grove init fish | source".to_string(),
        s => format!("command -v grove >/dev/null 2>&1 && eval \"$(grove init {})\"", name_of(s)),
    };
    format!("{MARKER} (managed by `grove setup`; safe to delete this block)\n{load}\n")
}

fn reload_hint(shell: Shell, rc: &Path) -> String {
    match shell {
        Shell::Fish => "exec fish".to_string(),
        _ => format!("source {}", rc.display()),
    }
}

pub fn print_example() {
    print!("{EXAMPLE}");
}

const EXAMPLE: &str = "\
# ~/.config/grove/aliases
# Shell-agnostic aliases. `grove init <shell>` turns these into aliases for your
# shell. Left = the name you type, right = the command it runs.
#
# Short names for the everyday git verbs. They're aliases (not binaries), so they
# only apply at your interactive prompt — rename any that clash with another tool
# on your system (e.g. change `gc` if you already use it for something else).
gs  = grove status
ga  = grove add
gc  = grove commit
gcp = grove commit --all --push
gp  = grove pull
gpp = grove push

# The multi-repo / tree tools. `lgs` (sync) is the everyday one — it pulls the
# behind repos and pushes the ahead ones; `lgp`/`lgpp` are the one-direction
# escape hatches. `lg` in particular clashes with lazygit — rename it (or any of
# these) if you already use that name for something else.
lg   = grove overview
lgs  = grove sync
lgp  = grove pull-all
lgpp = grove push-all
lt   = grove tree

# Add your own, too:
# co = grove commit
# st = grove status
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_defaults_reports_absent_names() {
        // An old file that predates most defaults (only gs + gcp present): setup
        // tops up every absent default — including the multi-repo aliases a file
        // written before the 2.0 collapse never had — but leaves the two present
        // names, even remapped ones, alone.
        let old = "gs  = gst\ngcp = gc --all --push\n";
        let missing: Vec<&str> = missing_defaults(old).into_iter().map(|(n, _)| n).collect();
        assert_eq!(missing, vec!["ga", "gc", "gp", "gpp", "lg", "lgs", "lgp", "lgpp", "lt"]);
    }

    #[test]
    fn missing_defaults_empty_when_all_present() {
        assert!(missing_defaults(EXAMPLE).is_empty());
    }

    #[test]
    fn remapped_name_is_not_reported_missing() {
        // `gp` present but remapped to something else is still "present" — we must
        // not rewrite a line the user has edited.
        assert!(!missing_defaults("gp = git pull --rebase\n").iter().any(|(n, _)| *n == "gp"));
    }

    #[test]
    fn divergent_defaults_flags_only_remapped_names() {
        // gs is remapped (divergent); gp matches the default; ga is absent (not divergent).
        let text = "gs = gst\ngp = grove pull\n";
        let div = divergent_defaults(text);
        assert_eq!(div.len(), 1);
        assert_eq!(div[0], ("gs", "gst".to_string(), "grove status"));
    }

    #[test]
    fn override_alias_rewrites_rhs_and_keeps_formatting() {
        let text = "# a comment\ngs  = gst\ngcp = gc --all --push\n";
        let out = override_alias(text, "gs", "grove status");
        assert_eq!(out, "# a comment\ngs  = grove status\ngcp = gc --all --push\n");
        // Untouched names and comments are preserved verbatim.
        assert!(out.contains("# a comment"));
        assert!(out.contains("gcp = gc --all --push"));
    }

    #[test]
    fn override_alias_preserves_trailing_newline_absence() {
        assert_eq!(override_alias("gp = git pull", "gp", "grove pull"), "gp = grove pull");
    }

    #[test]
    fn example_template_defines_every_default() {
        // The starter template and the built-in defaults must not drift apart.
        assert!(missing_defaults(EXAMPLE).is_empty(), "EXAMPLE is missing a default alias");
    }
}
