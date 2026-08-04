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
use std::path::PathBuf;
use std::time::Duration;

/// Every recognized key, with its human blurb — the source of truth for
/// validation and the `grove configure` listing.
const KEYS: &[(&str, &str)] = &[
    ("cache", "reuse a recent fetch instead of re-fetching (on/off)"),
    ("cache_ttl", "seconds a fetch stays fresh for the cache"),
    ("default_dir", "folder to use when the current one has no git repos"),
];

const DEFAULT_TTL: u64 = 5;

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

/// Expand a leading `~` to `$HOME`; other paths pass through unchanged.
fn expand_tilde(s: &str) -> PathBuf {
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

    let path = settings_path();
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let updated = upsert(&existing, key, canonical.as_deref());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, updated)?;

    match &canonical {
        Some(v) => println!("{} {} = {}", paint("1;32", "set"), paint("36", key), paint("1", v)),
        None => println!("{} {}", paint("1;32", "cleared"), paint("36", key)),
    }
    Ok(())
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
}
