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

/// Every remote of `repo` as (name, url) pairs, in `git remote -v` order.
/// Fetch/push URLs collapse to one entry per remote (we only rewrite the URL,
/// and `set-url` without `--push` updates both).
pub fn remotes(repo: &Path) -> Vec<(String, String)> {
    let Some(out) = git_out(repo, &["remote"]) else {
        return Vec::new();
    };
    out.lines()
        .filter_map(|name| {
            let url = git_out(repo, &["remote", "get-url", name])?;
            Some((name.to_string(), url))
        })
        .collect()
}

/// Convert an https remote URL to its ssh equivalent, or None if it isn't https.
/// `https://[user[:pw]@]host[:port]/path` → `git@host:path` (scp-like), or
/// `ssh://git@host:port/path` when a port is present (scp syntax can't carry a
/// port). Embedded credentials (a token or `user:pw@` before the host) are
/// dropped — ssh authenticates with your key, not a URL secret.
pub fn https_to_ssh(url: &str) -> Option<String> {
    let (authority, path) = url.strip_prefix("https://")?.split_once('/')?;
    // Strip any userinfo (`token@` / `user:pw@`) that precedes the host.
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    if host.is_empty() || path.is_empty() {
        return None;
    }
    match host.split_once(':') {
        Some((h, port)) => Some(format!("ssh://git@{h}:{port}/{path}")),
        None => Some(format!("git@{host}:{path}")),
    }
}

/// The browser URL for a repo's `origin` (its GitHub/GitLab/Gitea/Forgejo page),
/// or None if there's no origin or it can't be parsed. Whatever transport origin
/// uses — scp-form, `ssh://`, `git://`, http(s) — maps to `https://host/path`.
pub fn web_url(repo: &Path) -> Option<String> {
    remote_to_web(&git_out(repo, &["remote", "get-url", "origin"])?)
}

/// Pure counterpart of [`web_url`]: map any git remote URL to its https web page.
pub fn remote_to_web(url: &str) -> Option<String> {
    let (host, path) = split_host_path(url)?;
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some(format!("https://{host}/{path}"))
}

/// Split a remote URL into (host, path), dropping any userinfo and `:port`.
fn split_host_path(url: &str) -> Option<(String, String)> {
    for scheme in ["https://", "http://", "ssh://", "git://"] {
        if let Some(rest) = url.strip_prefix(scheme) {
            let (authority, path) = rest.split_once('/')?;
            let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
            let host = host.split(':').next().unwrap_or(host); // drop :port
            return Some((host.to_string(), path.to_string()));
        }
    }
    // scp-like: `[user@]host:path` (host carries no port in this form).
    let (authority, path) = url.split_once(':')?;
    if authority.contains('/') {
        return None; // a local path like `../foo`, not a remote
    }
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    Some((host.to_string(), path.to_string()))
}

/// Point `remote` at `url` (`git remote set-url`). Returns whether it succeeded.
pub fn set_remote_url(repo: &Path, remote: &str, url: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["remote", "set-url", remote, url])
        .status()
        .map(|s| s.success())
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
    // Capture (and drop) output rather than inheriting stderr: a failed fetch —
    // unreachable remote, missing ssh key — otherwise dumps git's `fatal:` wall
    // into the middle of the dashboard. The dashboard shows each repo's state
    // regardless, so a quiet fetch keeps the table clean, matching grove's
    // one-line-error style elsewhere.
    let _ = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["fetch", "--quiet"])
        .output();
}

/// `git pull` (honoring the user's `pull.rebase`/`pull.ff` config — grove doesn't
/// impose a strategy, so a fleet pull behaves exactly like `git pull` in each repo).
/// If it fails part-way — a rebase or merge that hit a conflict — abort the
/// in-progress operation so a bulk pull never strands a repo half-applied; the repo
/// is left as it was and reported unpulled. Returns whether the pull succeeded.
pub fn pull(repo: &Path) -> Result<bool> {
    // Capture (and drop) output rather than inheriting it: a conflicting rebase
    // otherwise dumps git's "CONFLICT …" wall into the middle of the fleet result.
    // The dashboard reprints each repo's state afterwards, so quiet keeps it clean.
    let ok = Command::new("git").arg("-C").arg(repo).args(["pull", "--quiet"]).output()?.status.success();
    if !ok {
        // No-ops when nothing is in progress; one of them cleans up on a conflict.
        for op in [["rebase", "--abort"], ["merge", "--abort"]] {
            let _ = Command::new("git").arg("-C").arg(repo).args(op).output();
        }
    }
    Ok(ok)
}

pub fn push(repo: &Path) -> Result<bool> {
    // Capture output (see `pull`): a rejected push shouldn't leak `! [rejected]`
    // noise into the fleet result — the reprinted dashboard shows what moved.
    Ok(Command::new("git").arg("-C").arg(repo).args(["push", "--quiet"]).output()?.status.success())
}

#[cfg(test)]
mod tests {
    use super::{https_to_ssh, remote_to_web};

    #[test]
    fn rewrites_the_common_github_form() {
        assert_eq!(
            https_to_ssh("https://github.com/owner/repo.git").as_deref(),
            Some("git@github.com:owner/repo.git")
        );
    }

    #[test]
    fn preserves_a_missing_dot_git_suffix() {
        assert_eq!(
            https_to_ssh("https://github.com/owner/repo").as_deref(),
            Some("git@github.com:owner/repo")
        );
    }

    #[test]
    fn keeps_nested_gitlab_groups() {
        assert_eq!(
            https_to_ssh("https://gitlab.com/group/subgroup/repo.git").as_deref(),
            Some("git@gitlab.com:group/subgroup/repo.git")
        );
    }

    #[test]
    fn drops_an_embedded_token() {
        assert_eq!(
            https_to_ssh("https://ghp_secret@github.com/owner/repo.git").as_deref(),
            Some("git@github.com:owner/repo.git")
        );
    }

    #[test]
    fn drops_embedded_user_and_password() {
        assert_eq!(
            https_to_ssh("https://user:pw@gitlab.com/owner/repo.git").as_deref(),
            Some("git@gitlab.com:owner/repo.git")
        );
    }

    #[test]
    fn uses_ssh_scheme_when_a_port_is_present() {
        assert_eq!(
            https_to_ssh("https://git.company.com:8443/owner/repo.git").as_deref(),
            Some("ssh://git@git.company.com:8443/owner/repo.git")
        );
    }

    #[test]
    fn returns_none_for_non_https_or_malformed() {
        assert_eq!(https_to_ssh("git@github.com:owner/repo.git"), None);
        assert_eq!(https_to_ssh("ssh://git@github.com/owner/repo.git"), None);
        assert_eq!(https_to_ssh("https://github.com"), None); // no path
        assert_eq!(https_to_ssh("https://github.com/"), None); // empty path
    }

    #[test]
    fn web_url_from_scp_ssh_form() {
        assert_eq!(
            remote_to_web("git@github.com:owner/repo.git").as_deref(),
            Some("https://github.com/owner/repo")
        );
    }

    #[test]
    fn web_url_from_https_strips_git_and_credentials() {
        assert_eq!(
            remote_to_web("https://ghp_tok@github.com/owner/repo.git").as_deref(),
            Some("https://github.com/owner/repo")
        );
    }

    #[test]
    fn web_url_from_ssh_scheme_drops_port() {
        assert_eq!(
            remote_to_web("ssh://git@git.company.com:2222/owner/repo.git").as_deref(),
            Some("https://git.company.com/owner/repo")
        );
    }

    #[test]
    fn web_url_keeps_nested_gitlab_groups() {
        assert_eq!(
            remote_to_web("git@gitlab.com:group/subgroup/repo.git").as_deref(),
            Some("https://gitlab.com/group/subgroup/repo")
        );
    }

    #[test]
    fn web_url_none_for_local_paths() {
        assert_eq!(remote_to_web("../bare/repo.git"), None);
        assert_eq!(remote_to_web("/srv/git/repo.git"), None);
    }
}
