//! Shared logic behind the grove tools. `grove` calls in here for the git verbs
//! (status/add/commit/pull/push) and the multi-repo tools lg/lgp/lgpp/lt are
//! their own thin binaries that do too. Keeping the logic in one lib means the
//! tools share the same git handling, colors, and error style.
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
