//! Tiny color helper, a friendly error line, and a progress bar. All respect
//! NO_COLOR and non-TTY output, so piping stays clean and scriptable.
//!
//! The color decision is made **once per stream** via `OnceLock` (env + TTY read
//! exactly once, so every call agrees) and keyed to the stream the helper writes
//! to: `paint` colors stdout content, so it gates on stdout; `err` writes to
//! stderr, so it gates on stderr.
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{self, IsTerminal};
use std::sync::OnceLock;

/// Whether to emit ANSI on stdout — `NO_COLOR` unset AND stdout is a terminal.
fn stdout_color() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("NO_COLOR").is_none() && io::stdout().is_terminal())
}

/// Whether to emit ANSI on stderr — `NO_COLOR` unset AND stderr is a terminal.
fn stderr_color() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("NO_COLOR").is_none() && io::stderr().is_terminal())
}

/// Wrap `s` in an ANSI SGR code (e.g. "1;34"), unless color is off / stdout isn't a TTY.
pub fn paint(code: &str, s: &str) -> String {
    if stdout_color() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

/// True when stdout is a terminal we believe renders OSC 8 hyperlinks (iTerm2,
/// WezTerm, kitty, Ghostty, VTE ≥ 0.50, Windows Terminal, …). Piped/redirected
/// output and unrecognized terminals report false, so we never emit a link that
/// can't be clicked — set `FORCE_HYPERLINK=1` to override the detection. Note
/// macOS Terminal.app does NOT support OSC 8, so it correctly reports false.
pub fn hyperlinks() -> bool {
    supports_hyperlinks::on(supports_hyperlinks::Stream::Stdout)
}

/// Wrap `text` in an OSC 8 hyperlink to `url`. Only worth emitting when
/// [`hyperlinks`] is true; a terminal that doesn't understand OSC 8 silently
/// drops the escape and shows just `text` (no visible garbage).
pub fn link(url: &str, text: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
}

/// A friendly error line to stderr (red ✗), used instead of leaking raw git noise.
pub fn err(m: &str) {
    let line = format!("✗ {m}");
    if stderr_color() {
        eprintln!("\x1b[1;31m{line}\x1b[0m");
    } else {
        eprintln!("{line}");
    }
}

/// A determinate progress bar that animates via a steady tick (spinner +
/// elapsed), so it reads as live even while parallel work is in flight rather
/// than looking frozen between updates. Leads with the count (`Fetching 3/12`).
/// Draws to stderr; auto-hidden when not a TTY.
pub fn bar(len: u64, msg: &str) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::with_template("  {spinner:.cyan} {msg} {pos}/{len} [{bar:24.cyan/blue}] {elapsed}")
            .unwrap()
            .progress_chars("=>-")
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(90));
    pb
}
