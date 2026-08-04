//! Per-repo fetch cache for fleet operations. Fetching is the slow part of the
//! multi-repo verbs (one SSH handshake per repo), so we skip re-fetching a repo
//! that a recent *real* fetch already left fully **settled** (clean worktree, in
//! sync, ssh) — those are the quiet repos with nothing to act on. Anything dirty,
//! ahead, behind, diverged, or https always re-fetches, so the repos you act on
//! are never stale; only the "nothing to do here" rows can lag, and only for the
//! TTL (default 5s). `--force` bypasses it.
//!
//! Bounded by construction: a stamp records the last *real* fetch time and is
//! never refreshed on a skip, so a repo re-fetches at most TTL seconds after its
//! last actual fetch — skips can't chain indefinitely.
//!
//! Lives in the binary (an env/XDG concern); the stamp is a zero-byte file under
//! `~/.cache/grove/` whose modification time is the timestamp.
use crate::config;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// `~/.cache/grove` (honoring `XDG_CACHE_HOME`), or `None` if `$HOME` is unset.
fn cache_dir() -> Option<PathBuf> {
    let base = match config::env_path("XDG_CACHE_HOME") {
        Some(p) => p,
        None => config::env_path("HOME")?.join(".cache"),
    };
    Some(base.join("grove"))
}

/// The stamp file for one repo, under `dir`, keyed by the repo's canonical path.
fn stamp_path(dir: &Path, repo: &Path) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let canon = std::fs::canonicalize(repo).ok()?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    canon.hash(&mut h);
    Some(dir.join(format!("repo-{:016x}", h.finish())))
}

fn stamp_file(repo: &Path) -> Option<PathBuf> {
    stamp_path(&cache_dir()?, repo)
}

/// Whether `file`'s mtime is no more than `ttl` old. Any uncertainty (missing,
/// unreadable, clock skew) is `false`, so we fetch rather than risk staleness.
fn fresh(file: &Path, ttl: Duration) -> bool {
    let Ok(modified) = std::fs::metadata(file).and_then(|m| m.modified()) else { return false };
    SystemTime::now().duration_since(modified).map(|age| age <= ttl).unwrap_or(false)
}

/// Whether a real fetch left `repo` settled no more than `ttl` ago — a hit means
/// its fetch can be skipped this run.
pub fn settled_within(repo: &Path, ttl: Duration) -> bool {
    stamp_file(repo).map(|f| fresh(&f, ttl)).unwrap_or(false)
}

/// Record that a real fetch just left `repo` settled (mtime = now). Best-effort.
pub fn mark_settled(repo: &Path) {
    let Some(file) = stamp_file(repo) else { return };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&file, []);
}

/// Record that `repo` has pending work — drop any settled stamp so it always
/// re-fetches until a later fetch finds it settled again. Best-effort.
pub fn mark_unsettled(repo: &Path) {
    if let Some(file) = stamp_file(repo) {
        let _ = std::fs::remove_file(file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settled_stamp_round_trips_and_respects_ttl() {
        let cache = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap(); // must exist to canonicalize
        let file = stamp_path(cache.path(), repo.path()).unwrap();

        // Mark settled (write the stamp): fresh within a generous ttl, expired at 0.
        std::fs::write(&file, []).unwrap();
        assert!(fresh(&file, Duration::from_secs(60)), "just-written stamp should be fresh");
        assert!(!fresh(&file, Duration::from_secs(0)), "ttl 0 means nothing is fresh");

        // Unsettle (remove the stamp): never fresh.
        std::fs::remove_file(&file).unwrap();
        assert!(!fresh(&file, Duration::from_secs(60)), "removed stamp is not fresh");
    }

    #[test]
    fn distinct_repos_get_distinct_stamps() {
        let cache = tempfile::tempdir().unwrap();
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        assert_ne!(stamp_path(cache.path(), a.path()), stamp_path(cache.path(), b.path()));
    }
}
