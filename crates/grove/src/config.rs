//! The grove file: `~/.config/grove/aliases`, shell-agnostic `name = command`
//! lines. `grove init <shell>` translates it into that shell's alias syntax. If
//! no file exists, a built-in default (gs, gcp) is emitted, so it still works out
//! of the box.
use clap::ValueEnum;
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

pub fn init(shell: Shell) {
    for (name, cmd) in aliases() {
        match shell {
            Shell::Fish => println!("alias {name} '{cmd}'"),
            Shell::Zsh | Shell::Bash => println!("alias {name}='{cmd}'"),
        }
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
# These two are the collision-prone names grove doesn't ship as binaries
# (gs clashes with Ghostscript, gcp with coreutils' cp), so they live here:
gs  = gst
gcp = gc --all --push

# Add your own, e.g.:
# gl = lg
";
