//! Tiny color helper, a friendly error line, and a progress bar. All respect
//! NO_COLOR and non-TTY output, so piping stays clean and scriptable.
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{self, IsTerminal};

fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some()
}

/// Wrap `s` in an ANSI SGR code (e.g. "1;34"), unless color is off / stdout isn't a TTY.
pub fn paint(code: &str, s: &str) -> String {
    if !no_color() && io::stdout().is_terminal() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

/// A friendly error line to stderr (red ✗), used instead of leaking raw git noise.
pub fn err(m: &str) {
    let line = format!("✗ {m}");
    if !no_color() && io::stderr().is_terminal() {
        eprintln!("\x1b[1;31m{line}\x1b[0m");
    } else {
        eprintln!("{line}");
    }
}

/// A determinate progress bar (draws to stderr; auto-hidden when not a TTY).
pub fn bar(len: u64, msg: &str) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::with_template("  {msg} [{bar:30.cyan/blue}] {pos}/{len} ({elapsed})")
            .unwrap()
            .progress_chars("=>-"),
    );
    pb.set_message(msg.to_string());
    pb
}
