//! The settings file: `~/.config/grove/config`, the same shell-agnostic
//! `key = value` shape as the grove file (parsed by [`config::parse`]), so it
//! needs no new dependency. Three knobs, all optional — a missing file (or a
//! missing key) just means the default:
//!
//! - `cache`       — fetch-freshness cache on/off (default **on**)
//! - `cache_ttl`   — how many seconds a fetch stays fresh (default **5**)
//! - `default_dir` — folder the multi-repo verbs fall back to when the current
//!   directory is unrelated to git (default **unset** — no fallback)
//!
//! [`load`] is the typed view the binary runs on; [`configure`] backs
//! `grove configure` (list / get / set).
use crate::config;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Every recognized key, with its human blurb — the source of truth for
/// validation and the `grove configure` listing.
const KEYS: &[(&str, &str)] = &[
    ("cache", "reuse a recent fetch instead of re-fetching (on/off)"),
    ("cache_ttl", "seconds a fetch stays fresh for the cache"),
    ("default_dir", "folder to use when the current one has no git repos"),
];

const DEFAULT_TTL: u64 = 5;

/// Folder names people commonly give the directory that holds their repos — used
/// to prefer a deliberately-named dev folder over an incidental one when `grove
/// setup` autodetects a `default_dir`. Matched case-insensitively, so `Developer`
/// and `developer` both count.
const DEV_DIR_NAMES: &[&str] =
    &["developer", "dev", "src", "code", "projects", "project", "work", "repos", "repo", "git", "sources", "workspace"];

/// An *unconventionally*-named folder needs at least this many repos to make the
/// suggestion list — one stray clone in `~/Downloads` shouldn't show up, but a
/// small collection is fair game for the user to pick.
const COLLECTION_MIN: usize = 2;

/// Cap the suggestion list to the top-ranked few, so the `grove setup` menu stays
/// scannable.
const MAX_SUGGESTIONS: usize = 5;

/// The resolved, typed settings the binary acts on.
pub struct Settings {
    pub cache: bool,
    pub cache_ttl: u64,
    /// Already `~`-expanded to an absolute path, ready to use.
    pub default_dir: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { cache: true, cache_ttl: DEFAULT_TTL, default_dir: None }
    }
}

impl Settings {
    /// The TTL as a `Duration` for the cache check.
    pub fn ttl(&self) -> Duration {
        Duration::from_secs(self.cache_ttl)
    }
}

fn settings_path() -> PathBuf {
    config::config_dir().join("config")
}

/// The file's raw `key = value` pairs (empty when there's no file).
fn raw() -> Vec<(String, String)> {
    std::fs::read_to_string(settings_path()).map(|t| config::parse(&t)).unwrap_or_default()
}

fn lookup(pairs: &[(String, String)], key: &str) -> Option<String> {
    pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
}

/// Render `path` with a leading `~` when it sits under `$HOME`, else its plain
/// display form — so the setup menu and the settings file read `~/Developer`, not
/// the noisy full `/Users/you/Developer`. The inverse of [`expand_tilde`].
pub(crate) fn tildify(path: &Path) -> String {
    match config::env_path("HOME") {
        Some(home) => tildify_under(&home, path),
        None => path.display().to_string(),
    }
}

fn tildify_under(home: &Path, path: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// Expand a leading `~` to `$HOME`; other paths pass through unchanged. Shared
/// with the setup picker, which lets you type a folder that wasn't autodetected.
pub(crate) fn expand_tilde(s: &str) -> PathBuf {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = config::env_path("HOME") {
            return home.join(rest);
        }
    }
    if s == "~" {
        if let Some(home) = config::env_path("HOME") {
            return home;
        }
    }
    PathBuf::from(s)
}

/// Parse the various truthy/falsy spellings we accept for `cache`.
fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "on" | "true" | "yes" | "1" => Some(true),
        "off" | "false" | "no" | "0" => Some(false),
        _ => None,
    }
}

/// The typed settings the binary runs on: file values where present, defaults
/// otherwise. Malformed values fall back to the default rather than erroring —
/// `configure` validates on write, so a bad value only lands here if hand-edited.
pub fn load() -> Settings {
    let pairs = raw();
    let mut s = Settings::default();
    if let Some(v) = lookup(&pairs, "cache") {
        s.cache = parse_bool(&v).unwrap_or(s.cache);
    }
    if let Some(v) = lookup(&pairs, "cache_ttl") {
        s.cache_ttl = v.parse().unwrap_or(s.cache_ttl);
    }
    s.default_dir = lookup(&pairs, "default_dir").filter(|v| !v.is_empty()).map(|v| expand_tilde(&v));
    s
}

/// `grove configure` — list all settings (no args), print one (key only), or set
/// one (key + value). Only `grove configure` exposes these; there is no short
/// alias for it.
pub fn configure(key: Option<String>, value: Option<String>) -> Result<()> {
    match (key, value) {
        (None, _) => list(),
        (Some(k), None) => get(&k),
        (Some(k), Some(v)) => set(&k, &v),
    }
}

fn known(key: &str) -> bool {
    KEYS.iter().any(|(k, _)| *k == key)
}

fn unknown_key_error(key: &str) -> anyhow::Error {
    let names: Vec<&str> = KEYS.iter().map(|(k, _)| *k).collect();
    anyhow::anyhow!("unknown setting `{key}` — known settings: {}", names.join(", "))
}

/// The effective value string for display: the file value, or `default: …`.
fn effective(pairs: &[(String, String)], key: &str) -> String {
    if let Some(v) = lookup(pairs, key) {
        return v;
    }
    match key {
        "cache" => "on".into(),
        "cache_ttl" => DEFAULT_TTL.to_string(),
        "default_dir" => "unset".into(),
        _ => String::new(),
    }
}

fn list() -> Result<()> {
    use grove_core::ui::paint;
    let pairs = raw();
    let path = settings_path();
    let source = if path.exists() { path.display().to_string() } else { format!("{} (not created yet — all defaults)", path.display()) };
    println!("{} — {}", paint("1;32", "grove settings"), paint("90", &source));
    println!();
    let w = KEYS.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (k, blurb) in KEYS {
        let set_here = lookup(&pairs, k).is_some();
        let val = effective(&pairs, k);
        let shown = if set_here { paint("1", &val) } else { paint("90", &val) };
        println!("  {}  {}   {}", paint("36", &format!("{k:<w$}")), shown, paint("90", blurb));
    }
    println!();
    println!("{}", paint("90", "Set one with:  grove configure <key> <value>   (e.g. grove configure default_dir ~/Developer)"));
    Ok(())
}

fn get(key: &str) -> Result<()> {
    if !known(key) {
        return Err(unknown_key_error(key));
    }
    println!("{}", effective(&raw(), key));
    Ok(())
}

fn set(key: &str, value: &str) -> Result<()> {
    use grove_core::ui::paint;
    if !known(key) {
        return Err(unknown_key_error(key));
    }
    // Validate + canonicalize per key; an empty/"unset" value clears the key.
    let cleared = matches!(value.trim().to_ascii_lowercase().as_str(), "" | "unset" | "none");
    let canonical: Option<String> = if cleared {
        None
    } else {
        Some(match key {
            "cache" => {
                let on = parse_bool(value).ok_or_else(|| anyhow::anyhow!("`cache` must be on or off (got `{value}`)"))?;
                if on { "on" } else { "off" }.to_string()
            }
            "cache_ttl" => {
                let n: u64 = value.trim().parse().map_err(|_| anyhow::anyhow!("`cache_ttl` must be a whole number of seconds (got `{value}`)"))?;
                n.to_string()
            }
            _ => value.trim().to_string(), // default_dir: store the path as typed (~ kept for readability)
        })
    };

    write_key(key, canonical.as_deref())?;

    match &canonical {
        Some(v) => println!("{} {} = {}", paint("1;32", "set"), paint("36", key), paint("1", v)),
        None => println!("{} {}", paint("1;32", "cleared"), paint("36", key)),
    }
    Ok(())
}

/// Upsert `key` in the settings file (creating it if needed), or drop the line
/// when `value` is None. The bare I/O behind both `set` (which validates + prints)
/// and [`put`] (a quiet programmatic setter for the setup flow).
fn write_key(key: &str, value: Option<&str>) -> Result<()> {
    let path = settings_path();
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let updated = upsert(&existing, key, value);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, updated)?;
    Ok(())
}

/// Programmatically set a key (no validation, no output) — used by `grove setup`
/// to record an autodetected `default_dir` the user accepted.
pub(crate) fn put(key: &str, value: &str) -> Result<()> {
    write_key(key, Some(value))
}

/// Whether `default_dir` is already set to a non-empty value — `grove setup` uses
/// this to skip the autodetect offer once the user has a default.
pub(crate) fn default_dir_configured() -> bool {
    lookup(&raw(), "default_dir").filter(|v| !v.is_empty()).is_some()
}

/// Autodetect the folders full of git repos worth suggesting as `default_dir`,
/// ranked best-first. Empty when nothing convincing turns up. See
/// [`candidates_in`]; this just supplies `$HOME`.
pub(crate) fn detect_candidates() -> Vec<(PathBuf, usize)> {
    match config::env_path("HOME") {
        Some(home) => candidates_in(&home),
        None => Vec::new(),
    }
}

/// The testable core of [`detect_candidates`]: scan `home`'s immediate,
/// non-hidden subdirectories, count the git repos directly inside each, and keep
/// the plausible dev-root folders — a conventionally-named one with any repos, or
/// any folder holding at least [`COLLECTION_MIN`]. Ranked conventionally-named
/// first, then by repo count, capped at [`MAX_SUGGESTIONS`]. Each entry is
/// `(path, repo count)`.
fn candidates_in(home: &Path) -> Vec<(PathBuf, usize)> {
    let Ok(entries) = std::fs::read_dir(home) else {
        return Vec::new();
    };
    let mut found: Vec<(PathBuf, usize, bool)> = Vec::new(); // (path, repos, conventional)
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        if name.starts_with('.') || !path.is_dir() {
            continue;
        }
        let repos = grove_core::git::discover(&path).len();
        let conventional = DEV_DIR_NAMES.contains(&name.to_ascii_lowercase().as_str());
        if (conventional && repos >= 1) || repos >= COLLECTION_MIN {
            found.push((path, repos, conventional));
        }
    }
    // Conventionally-named folders first, then by repo count — the likeliest
    // default floats to the top of the menu.
    found.sort_by_key(|(_, repos, conventional)| (!conventional, std::cmp::Reverse(*repos)));
    found.into_iter().take(MAX_SUGGESTIONS).map(|(path, repos, _)| (path, repos)).collect()
}

/// Rewrite `key`'s line to `value` (preserving the file's other lines and its
/// trailing newline), append it if absent, or drop the line when `value` is None.
fn upsert(text: &str, key: &str, value: Option<&str>) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut replaced = false;
    for line in text.lines() {
        let is_key = line.split_once('=').map(|(lhs, _)| lhs.trim() == key).unwrap_or(false);
        if is_key {
            if let Some(v) = value {
                out.push(format!("{key} = {v}"));
            }
            replaced = true; // a None value simply omits the line (clears the key)
        } else {
            out.push(line.to_string());
        }
    }
    if !replaced {
        if let Some(v) = value {
            out.push(format!("{key} = {v}"));
        }
    }
    let mut joined = out.join("\n");
    if !joined.is_empty() {
        joined.push('\n');
    }
    joined
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_replaces_an_existing_key_in_place() {
        let out = upsert("cache = on\ncache_ttl = 5\n", "cache_ttl", Some("30"));
        assert_eq!(out, "cache = on\ncache_ttl = 30\n");
    }

    #[test]
    fn upsert_appends_a_missing_key() {
        assert_eq!(upsert("cache = on\n", "cache_ttl", Some("10")), "cache = on\ncache_ttl = 10\n");
        assert_eq!(upsert("", "cache", Some("off")), "cache = off\n");
    }

    #[test]
    fn upsert_none_clears_the_key_line() {
        assert_eq!(upsert("cache = on\ndefault_dir = ~/x\n", "default_dir", None), "cache = on\n");
    }

    #[test]
    fn parse_bool_accepts_common_spellings() {
        assert_eq!(parse_bool("ON"), Some(true));
        assert_eq!(parse_bool("0"), Some(false));
        assert_eq!(parse_bool("maybe"), None);
    }

    /// Make `home/<folder>/<repo>/.git` for each repo name — enough for
    /// `git::discover` to count it as a repo, without needing a real git tree.
    fn seed_repos(home: &Path, folder: &str, repos: &[&str]) {
        for r in repos {
            std::fs::create_dir_all(home.join(folder).join(r).join(".git")).unwrap();
        }
    }

    fn names(candidates: &[(PathBuf, usize)]) -> Vec<String> {
        candidates.iter().map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn candidates_rank_conventional_first_then_by_repo_count() {
        let home = tempfile::tempdir().unwrap();
        seed_repos(home.path(), "Developer", &["a", "b"]); // conventional, 2
        seed_repos(home.path(), "src", &["x"]); // conventional, 1
        seed_repos(home.path(), "stuff", &["p", "q", "r", "s"]); // unconventional, 4
        let c = candidates_in(home.path());
        // Both conventional folders rank ahead of the richer incidental one, and
        // among the conventional ones the higher repo count wins.
        assert_eq!(names(&c), vec!["Developer", "src", "stuff"]);
        assert_eq!(c[0].1, 2);
    }

    #[test]
    fn candidates_exclude_a_lone_repo_in_an_unconventional_folder() {
        let home = tempfile::tempdir().unwrap();
        seed_repos(home.path(), "misc", &["only"]); // 1 repo, not a conventional name
        assert!(candidates_in(home.path()).is_empty());
    }

    #[test]
    fn candidates_include_a_two_repo_unconventional_collection() {
        let home = tempfile::tempdir().unwrap();
        seed_repos(home.path(), "misc", &["a", "b"]); // >= COLLECTION_MIN
        let c = candidates_in(home.path());
        assert_eq!(names(&c), vec!["misc"]);
    }

    #[test]
    fn candidates_empty_when_home_has_no_repo_folders() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join("empty")).unwrap();
        assert!(candidates_in(home.path()).is_empty());
    }

    #[test]
    fn tildify_renders_paths_under_home_with_a_tilde() {
        let home = Path::new("/home/u");
        assert_eq!(tildify_under(home, Path::new("/home/u/Developer")), "~/Developer");
        assert_eq!(tildify_under(home, Path::new("/home/u/dev/projects")), "~/dev/projects");
        assert_eq!(tildify_under(home, Path::new("/home/u")), "~");
        // A sibling that merely shares a name prefix must not false-match.
        assert_eq!(tildify_under(home, Path::new("/home/user2/x")), "/home/user2/x");
        assert_eq!(tildify_under(home, Path::new("/opt/repos")), "/opt/repos");
    }

    #[test]
    fn candidates_are_capped_to_the_top_five_by_rank() {
        let home = tempfile::tempdir().unwrap();
        // Seven collections with descending repo counts — only the richest five surface.
        for count in [8usize, 7, 6, 5, 4, 3, 2] {
            let repos: Vec<String> = (0..count).map(|i| format!("repo{i}")).collect();
            let refs: Vec<&str> = repos.iter().map(String::as_str).collect();
            seed_repos(home.path(), &format!("r{count}"), &refs);
        }
        let c = candidates_in(home.path());
        assert_eq!(c.len(), 5);
        assert_eq!(names(&c), vec!["r8", "r7", "r6", "r5", "r4"]);
    }
}
