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

/// If `args` is exactly `--version` or `-V`, print "<name> <version>" and return
/// true so the caller can exit. Lets the thin passthrough bins (which otherwise
/// forward everything to git) answer `--version` themselves.
pub fn maybe_version(name: &str, args: &[String]) -> bool {
    if args.len() == 1 && matches!(args[0].as_str(), "--version" | "-V") {
        println!("{name} {}", env!("CARGO_PKG_VERSION"));
        return true;
    }
    false
}
