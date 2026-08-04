//! Fetch-freshness cache. The multi-repo verbs' slow part is the network fetch;
//! after one runs, it stamps the folder, and a follow-up on the same folder
//! within the TTL skips its own fetch — the remote-tracking refs are already
//! fresh, and the cheap local ahead/behind + dirty reads are always recomputed,
//! so decisions never go stale on your own edits.
//!
//! It lives in the binary (like `config`/`settings`, it's an env/XDG concern):
//! `grove-core` stays free of the environment and just receives a `fetch: bool`.
//! The stamp is a zero-byte file under `~/.cache/grove/` whose **modification
//! time** is the timestamp — no parsing, and `write` refreshes mtime by truncating.
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

/// The stamp file for `folder`: `fetch-<hash>` keyed by the folder's canonical
/// path, so `.`, `~/src`, and the absolute path all resolve to one stamp. `None`
/// when the folder can't be canonicalized (doesn't exist) or `$HOME` is unset.
fn stamp_file(folder: &Path) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let canon = std::fs::canonicalize(folder).ok()?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    canon.hash(&mut h);
    Some(cache_dir()?.join(format!("fetch-{:016x}", h.finish())))
}

/// Whether `folder` was fetched no more than `ttl` ago — a cache hit means the
/// caller can skip fetching. Any uncertainty (no stamp, unreadable mtime, clock
/// skew) reports false, so we fetch rather than risk acting on stale remotes.
pub fn is_fresh(folder: &Path, ttl: Duration) -> bool {
    let Some(file) = stamp_file(folder) else { return false };
    let Ok(modified) = std::fs::metadata(&file).and_then(|m| m.modified()) else { return false };
    SystemTime::now().duration_since(modified).map(|age| age <= ttl).unwrap_or(false)
}

/// Record that `folder` was just fetched (best-effort — a cache we can't write is
/// simply a cache miss next time, never an error).
pub fn stamp(folder: &Path) {
    let Some(file) = stamp_file(folder) else { return };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&file, []);
}
