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
/// git verbs, mapped to `grove` subcommands. These are aliases, not binaries, so
/// nothing short lands on PATH to collide with other tools — and each only
/// shadows at your interactive prompt, never in scripts. Rename any that clash on
/// your system (e.g. `gc`) by editing the grove file; that's the whole point.
const DEFAULTS: &[(&str, &str)] = &[
    ("gs", "grove status"),
    ("ga", "grove add"),
    ("gc", "grove commit"),
    ("gcp", "grove commit --all --push"),
    ("gp", "grove pull"),
    ("gpp", "grove push"),
];

/// Read an environment variable as a path, treating unset **and empty** the
/// same (an empty `XDG_CONFIG_HOME`/`ZDOTDIR` must not become a relative path).
fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key).filter(|v| !v.is_empty()).map(PathBuf::from)
}

fn config_path() -> PathBuf {
    let base = env_path("XDG_CONFIG_HOME")
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"));
    base.join("grove").join("aliases")
}

fn aliases() -> Vec<(String, String)> {
    match std::fs::read_to_string(config_path()) {
        Ok(text) => parse(&text),
        Err(_) => DEFAULTS.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
    }
}

/// Parse `name = command` lines; ignore blanks and `#` comments.
fn parse(text: &str) -> Vec<(String, String)> {
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
pub fn setup(shell: Option<Shell>) -> anyhow::Result<()> {
    use grove_core::ui::paint;
    let shell = shell
        .or_else(detect_shell)
        .ok_or_else(|| anyhow::anyhow!("couldn't detect your shell from $SHELL — run `grove setup zsh` (or bash/fish)"))?;
    let sh = name_of(shell);

    // 1) Materialize the editable grove file so aliases have a home to be renamed in.
    let cfg = config_path();
    let file_status = if cfg.exists() {
        "exists"
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
    let rc_desc = if rc_status == "present" {
        "already configured — no change".to_string()
    } else {
        format!("added the `grove init {sh}` line")
    };
    println!("  {} {} {}", paint("36", &format!("{:<10}", rc_name(shell))), paint("1", &format!("{rc_status:<8}")), rc_desc);
    println!();
    if file_status == "created" || rc_status == "added" {
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

# Add your own, e.g. short names for the multi-repo tools:
# gl = lg
# co = grove commit
";
