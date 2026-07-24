//! Shared logic behind the grove tools. Each command (gst, ga, gc, gp, gpp, lg,
//! lgp, lt) is its own thin binary that calls into here; `grove` itself is the
//! init/config tool. Keeping the logic in one lib means the tools share the same
//! git handling, colors, and error style without folding into a single binary.
pub mod git;
pub mod overview;
pub mod passthrough;
pub mod sync;
pub mod tree;
pub mod ui;

/// Restore default SIGPIPE handling so piping into `head`/`less` terminates
/// quietly instead of panicking on a broken pipe (Rust sets SIGPIPE to ignore
/// by default, and git would inherit that across exec). Call once at the top of
/// each binary's `main`.
pub fn reset_sigpipe() {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}
