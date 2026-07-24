//! The grove file: `~/.config/grove/aliases`, shell-agnostic `name = command`
//! lines. `grove init <shell>` translates it into that shell's alias syntax. If
//! no file exists, a built-in default (gs, gcp) is emitted, so it still works out
//! of the box.
use clap::ValueEnum;
use std::io::{self, IsTerminal};
use std::path::PathBuf;

#[derive(Clone, Copy, ValueEnum)]
pub enum Shell {
    Zsh,
    Bash,
    Fish,
}

/// Built-in defaults: the collision-prone short names grove deliberately does
/// NOT ship as binaries (gs → Ghostscript, gcp → coreutils' cp) — safe here
/// because a shell alias only shadows at your prompt, never in scripts.
const DEFAULTS: &[(&str, &str)] = &[("gs", "gst"), ("gcp", "gc --all --push")];

fn config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
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
    println!("{}", paint("90", "(when run non-interactively, e.g. via eval, it prints only the alias lines.)"));
}

pub fn print_example() {
    print!("{EXAMPLE}");
}

const EXAMPLE: &str = "\
# ~/.config/grove/aliases
# Shell-agnostic aliases. `grove init <shell>` turns these into aliases for your
# shell. Left = the name you type, right = the command it runs.
#
# These two are the collision-prone names grove doesn't ship as binaries
# (gs clashes with Ghostscript, gcp with coreutils' cp), so they live here:
gs  = gst
gcp = gc --all --push

# Add your own, e.g.:
# gl = lg
";
