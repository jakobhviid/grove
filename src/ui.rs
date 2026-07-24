//! Tiny color helper. Respects NO_COLOR and non-TTY output (same policy as
//! amdl/pwtune), so piping `grove overview | cat` stays clean and scriptable.
use std::io::{self, IsTerminal};

fn color() -> bool {
    std::env::var_os("NO_COLOR").is_none() && io::stdout().is_terminal()
}

/// Wrap `s` in an ANSI SGR code (e.g. "1;34"), unless color is disabled.
pub fn paint(code: &str, s: &str) -> String {
    if color() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn info(m: &str) {
    println!("{}", paint("1;34", &format!("▸ {m}")));
}
