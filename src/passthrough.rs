//! Run git so it owns the terminal. For pure passthroughs we exec-replace this
//! process with git on Unix, so color, pager, signals, and the exit code are all
//! git's — grove leaves no wrapper process behind. `commit` is the exception:
//! it may chain a push, so it spawns and waits and propagates the exit code.
use crate::git;
use anyhow::{bail, Result};
use std::process::Command;

/// Fail early with a friendly message when we're not in a repo, instead of
/// letting git print its "fatal: not a git repository …" wall of noise.
fn ensure_repo() -> Result<()> {
    if !git::inside_repo() {
        bail!("not a git repository — run this inside a repo, or `git init` to start one here");
    }
    Ok(())
}

/// Replace this process with `git <prefix...> <extra...>`.
/// On success (Unix) this never returns; it only returns `Err` if exec fails.
pub fn exec(prefix: &[&str], extra: &[String]) -> Result<()> {
    ensure_repo()?;
    let mut cmd = Command::new("git");
    cmd.args(prefix).args(extra);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec(); // returns only on failure
        bail!("failed to exec git: {err}");
    }
    #[cfg(not(unix))]
    {
        let status = cmd.status()?;
        std::process::exit(status.code().unwrap_or(1));
    }
}

/// `git commit -m <msg>` (with `-a` if `all`), then `git push` if `push`.
/// Exits non-zero if the commit or push fails.
pub fn commit(all: bool, push: bool, message: &[String]) -> Result<()> {
    ensure_repo()?;
    let msg = message.join(" ");
    if msg.trim().is_empty() {
        bail!("commit message required — e.g. `gc fixed the parser`");
    }
    let mut args: Vec<&str> = vec!["commit"];
    if all {
        args.push("-a");
    }
    args.push("-m");
    args.push(&msg);
    if !Command::new("git").args(&args).status()?.success() {
        std::process::exit(1);
    }
    if push && !Command::new("git").arg("push").status()?.success() {
        std::process::exit(1);
    }
    Ok(())
}
