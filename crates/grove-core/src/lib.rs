//! Shared logic behind the grove tools. The single `grove` binary calls in here
//! for the git verbs (status/add/commit/pull/push) and the multi-repo tools
//! (overview/sync/pull-all/push-all — the `lg`/`lgs`/`lgp`/`lgpp` aliases) plus
//! `tree`. Keeping the logic in one lib means every verb shares the same git
//! handling, colors, and error style, and a second frontend could reuse it.
pub mod git;
pub mod overview;
pub mod passthrough;
pub mod remote;
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
