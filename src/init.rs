//! `grove init <shell>` prints the short aliases so they work in any shell from a
//! single install: `eval "$(grove init zsh)"` (bash is identical) or
//! `grove init fish | source`. The aliases route through grove's subcommands, so
//! there is one source of truth and `brew upgrade grove` updates every shell.
use clap::ValueEnum;

#[derive(Clone, Copy, ValueEnum)]
pub enum Shell {
    Zsh,
    Bash,
    Fish,
}

/// (alias, grove subcommand) — the shortcuts this kit ships.
const ALIASES: &[(&str, &str)] = &[
    ("gs", "grove status"),
    ("ga", "grove add"),
    ("gc", "grove commit"),
    ("gcp", "grove commit --all --push"),
    ("gp", "grove pull"),
    ("gpp", "grove push"),
    ("lg", "grove overview"),
    ("lgp", "grove sync"),
    ("lt", "grove tree"),
];

pub fn print(shell: Shell) {
    match shell {
        // POSIX shells share alias syntax.
        Shell::Zsh | Shell::Bash => {
            for (name, cmd) in ALIASES {
                println!("alias {name}='{cmd}'");
            }
        }
        // fish takes the command unquoted-per-word: `alias name 'cmd'`.
        Shell::Fish => {
            for (name, cmd) in ALIASES {
                println!("alias {name} '{cmd}'");
            }
        }
    }
}
