//! Discover the git repos directly under a folder and read their state by
//! shelling out to `git`. Going through the real git binary (rather than a
//! library) means the user's config, credentials, and SSH agent all apply —
//! exactly matching the shell functions grove replaces.
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Repo {
    pub path: PathBuf,
    pub name: String,
}

/// Immediate subdirectories of `dir` that are git worktrees, sorted by name.
pub fn discover(dir: &Path) -> Vec<Repo> {
    let mut repos = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return repos;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.join(".git").exists() {
            let name = p
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            repos.push(Repo { path: p, name });
        }
    }
    repos.sort_by(|a, b| a.name.cmp(&b.name));
    repos
}

fn git_out(repo: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn is_https(repo: &Path) -> bool {
    git_out(repo, &["remote", "get-url", "origin"])
        .map(|u| u.starts_with("https://"))
        .unwrap_or(false)
}

/// True if the current directory is inside a git work tree.
pub fn inside_repo() -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn branch(repo: &Path) -> String {
    match git_out(repo, &["branch", "--show-current"]) {
        Some(b) if !b.is_empty() => b,
        _ => "detached".into(),
    }
}

/// (ahead, behind) vs the configured upstream, or None if there is no upstream.
pub fn ahead_behind(repo: &Path) -> Option<(u32, u32)> {
    git_out(repo, &["rev-parse", "--abbrev-ref", "@{upstream}"])?;
    let counts = git_out(
        repo,
        &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
    )?;
    let mut it = counts.split_whitespace();
    let ahead = it.next()?.parse().ok()?;
    let behind = it.next()?.parse().ok()?;
    Some((ahead, behind))
}

#[derive(Default)]
pub struct Dirty {
    pub staged: u32,
    pub modified: u32,
    pub untracked: u32,
}

impl Dirty {
    pub fn any(&self) -> bool {
        self.staged > 0 || self.modified > 0 || self.untracked > 0
    }
}

/// Parse `git status --porcelain`: X (index) and Y (worktree) per line.
pub fn dirty(repo: &Path) -> Dirty {
    let mut d = Dirty::default();
    let Some(out) = git_out(repo, &["status", "--porcelain"]) else {
        return d;
    };
    for line in out.lines() {
        let b = line.as_bytes();
        if b.len() < 2 {
            continue;
        }
        let (x, y) = (b[0] as char, b[1] as char);
        if x == '?' && y == '?' {
            d.untracked += 1;
            continue;
        }
        if matches!(x, 'M' | 'A' | 'D' | 'R' | 'C') {
            d.staged += 1;
        }
        if matches!(y, 'M' | 'D') {
            d.modified += 1;
        }
    }
    d
}

pub fn fetch(repo: &Path) {
    let _ = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["fetch", "--quiet"])
        .status();
}

pub fn pull(repo: &Path) -> Result<bool> {
    Ok(Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["pull", "--quiet"])
        .status()?
        .success())
}

pub fn push(repo: &Path) -> Result<bool> {
    Ok(Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["push", "--quiet"])
        .status()?
        .success())
}
