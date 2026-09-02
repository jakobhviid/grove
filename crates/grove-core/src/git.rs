//! Discover the git repos directly under a folder and read their state by
//! shelling out to `git`. Going through the real git binary (rather than a
//! library) means the user's config, credentials, and SSH agent all apply —
//! exactly matching the shell functions grove replaces.
use anyhow::Result;
use serde::Serialize;
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
///
/// The host a browser needs is not always the host git dials. Anyone juggling
/// two accounts on one forge points their repos at a `~/.ssh/config` alias
/// (`git@github-work:org/repo`) to pick the right key, and that alias resolves
/// on their machine alone. It reaches a repo two ways — written into the remote,
/// or swapped in by a `url.<alias>.insteadOf` rule — and both are unwound here
/// so the link lands on the forge.
pub fn web_url(repo: &Path) -> Option<String> {
    // `git remote get-url` applies insteadOf rewriting; the raw config value is
    // the URL as written, before any local alias took its place.
    let url = git_out(repo, &["config", "--get", "remote.origin.url"])
        .filter(|u| !u.is_empty())
        .or_else(|| git_out(repo, &["remote", "get-url", "origin"]))?;
    let (host, path) = split_host_path(&url)?;
    web_page(&browser_host(&host), &path)
}

/// Pure counterpart of [`web_url`]: map any git remote URL to its https web page.
pub fn remote_to_web(url: &str) -> Option<String> {
    let (host, path) = split_host_path(url)?;
    web_page(&host, &path)
}

fn web_page(host: &str, path: &str) -> Option<String> {
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some(format!("https://{host}/{path}"))
}

/// The hostname a browser can reach for `host`, asking ssh to expand any
/// `~/.ssh/config` alias behind it. A dotless name is the only shape an alias
/// takes — no forge answers at a single label — so an ordinary remote never pays
/// for the lookup, and a name ssh doesn't recognize comes back unchanged.
fn browser_host(host: &str) -> String {
    if host.contains('.') {
        return host.to_string();
    }
    let Ok(out) = Command::new("ssh").args(["-G", host]).output() else {
        return host.to_string();
    };
    if !out.status.success() {
        return host.to_string();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("hostname "))
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .map_or_else(|| host.to_string(), str::to_string)
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

/// Why a remote operation (fetch, pull, push) failed, in the four kinds a fleet
/// view can act on. The kind decides the mark and color in the dashboard; the
/// human-readable cause travels alongside it in [`Fail::detail`].
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Trouble {
    /// The remote refused us: no key, wrong key, no permission on the repo, or an
    /// unverified host key. Persistent until the access is fixed.
    Denied,
    /// The remote couldn't be reached at all — DNS, timeout, refused connection.
    /// Usually the network, not the repo, and usually transient.
    Unreachable,
    /// git stopped and wants a human: a conflict, local changes in the way, a
    /// rejected push, a leftover lock file.
    NeedsHand,
    /// Anything we don't classify. The detail line carries git's own words.
    Failed,
}

/// A failed git operation: what kind of trouble, and the one line worth showing.
#[derive(Serialize, Clone, Debug)]
pub struct Fail {
    pub kind: Trouble,
    /// git's own explanation, condensed to a single line (see [`detail`]).
    pub detail: String,
}

/// Sort git's/ssh's stderr into a [`Trouble`]. Matching is on the stable, decades-old
/// phrases (git keeps these for scripts and its own tests), lowercased so casing
/// differences between transports don't matter. Order matters: the specific
/// access/network phrases are tested before the generic ones, and anything
/// unrecognized falls through to [`Trouble::Failed`] — which still shows the user
/// git's own words, so a miss here degrades to "unclassified", never to silence.
pub fn classify(stderr: &str) -> Trouble {
    let s = stderr.to_ascii_lowercase();
    let any = |needles: &[&str]| needles.iter().any(|n| s.contains(n));

    // Access. GitHub answers "repository not found" for a private repo you can't
    // see, so that phrase is a permission problem, not a missing repo. A host key
    // that doesn't verify is also access: the transport refuses to talk.
    if any(&[
        "permission denied",
        "authentication failed",
        "denied to",
        "access denied",
        "repository not found",
        "could not read username",
        "could not read password",
        "terminal prompts disabled",
        "invalid username or password",
        "403 forbidden",
        "401 unauthorized",
        // GitLab masks "no permission" as "could not be found or you don't have
        // permission to view it"; Gitea/Forgejo say "not allowed".
        "have permission",
        "not allowed",
        "not authorized",
        "unauthorized",
        "host key verification failed",
        "remote host identification has changed",
        "no supported authentication methods",
    ]) {
        return Trouble::Denied;
    }
    // Network.
    if any(&[
        "could not resolve host",
        "couldn't connect to server",
        "connection timed out",
        "connection refused",
        "connection closed by remote host",
        "network is unreachable",
        "no route to host",
        "operation timed out",
        "temporary failure in name resolution",
        "failed to connect to",
    ]) {
        return Trouble::Unreachable;
    }
    // Needs a human.
    if any(&[
        "conflict",
        "could not apply",
        "automatic merge failed",
        "would be overwritten",
        "cannot pull with rebase",
        "you have unstaged changes",
        "needs merge",
        "index.lock",
        "another git process",
        "non-fast-forward",
        "[rejected]",
        "updates were rejected",
        "hook declined",
        "protected branch",
        "unmerged files",
        "not possible to fast-forward",
        "divergent branches",
    ]) {
        return Trouble::NeedsHand;
    }
    Trouble::Failed
}

/// Peel git's line prefixes off, however they stack — a forge speaking through the
/// transport reaches us as `remote: ERROR: …`, so one pass isn't enough.
fn strip_prefixes(line: &str) -> &str {
    let mut line = line.trim();
    while let Some(shorter) = ["fatal:", "error:", "ERROR:", "remote:", "warning:", "git:"].iter().find_map(|p| line.strip_prefix(p)) {
        line = shorter.trim_start();
    }
    line
}

/// One output line, stripped of prefixes, if it explains anything — else `None`.
/// Filtered out: git's hints and headers, the two-line "Please make sure you have
/// the correct access rights" boilerplate that trails every failed handshake, a
/// forge's `=====` banner rules, and bare `remote:` spacers.
fn explaining(raw: &str) -> Option<&str> {
    let raw = raw.trim();
    const SCAFFOLD: [&str; 7] = ["hint:", "To ", "Auto-merging", "Everything up-to-date", "Warning: Permanently added", "Please make sure you have", "and the repository exists"];
    if SCAFFOLD.iter().any(|p| raw.starts_with(p)) {
        return None;
    }
    let line = strip_prefixes(raw);
    // No letter or digit left means the line was decoration or a bare prefix.
    line.chars().any(|c| c.is_alphanumeric()).then_some(line)
}

/// The one line of git's output worth showing a human: the first line that carries
/// a real explanation. git prefixes its own noise (`fatal:`, `error:`, `remote:`,
/// `hint:`) and pads with blanks and progress; we skip the lines that only
/// scaffold and strip the prefixes off the one we keep, then cap the length so a
/// pathological remote message can't wrap the dashboard.
///
/// A `CONFLICT` line wins when there is one: it names the file that needs merging,
/// which is the next thing the reader will want, where git's accompanying
/// `could not apply <sha>… <subject>` names only the commit.
pub fn detail(out: &str) -> String {
    let lines = || out.lines().filter_map(explaining);
    let line = lines().find(|l| l.starts_with("CONFLICT")).or_else(|| lines().next()).unwrap_or("git failed without an explanation");
    let line = line.trim_end_matches('.').trim();
    if line.chars().count() > 120 {
        let cut: String = line.chars().take(117).collect();
        return format!("{cut}…");
    }
    line.to_string()
}

/// Build the [`Fail`] for a finished command. Both streams are read: git splits a
/// failure across them — the `fatal:`/`error:` lines go to stderr while a merge
/// writes its `CONFLICT (content): …` to stdout — so classifying stderr alone would
/// file a plain conflict under "unclassified". stderr leads, so its lines are
/// preferred when picking the one to show.
fn failed(out: &std::process::Output) -> Fail {
    let text = format!("{}\n{}", String::from_utf8_lossy(&out.stderr), String::from_utf8_lossy(&out.stdout));
    Fail { kind: classify(&text), detail: detail(&text) }
}

/// `git fetch`, returning why it failed (or `None` when it worked).
///
/// Capture stderr rather than inheriting it: a failed fetch — unreachable remote,
/// missing ssh key — otherwise dumps git's `fatal:` wall into the middle of the
/// dashboard. The wall isn't lost; it's classified into a mark on the repo's row
/// and one line under the table, which is grove's one-line-error style everywhere
/// else. A repo whose fetch failed still gets a row: its remote state is simply the
/// last one we managed to fetch.
pub fn fetch(repo: &Path) -> Option<Fail> {
    let out = Command::new("git").arg("-C").arg(repo).args(["fetch", "--quiet"]).output().ok()?;
    (!out.status.success()).then(|| failed(&out))
}

/// `git pull` (honoring the user's `pull.rebase`/`pull.ff` config — grove doesn't
/// impose a strategy, so a fleet pull behaves exactly like `git pull` in each repo).
/// If it fails part-way — a rebase or merge that hit a conflict — abort the
/// in-progress operation so a bulk pull never strands a repo half-applied; the repo
/// is left as it was and reported with the reason it didn't move.
pub fn pull(repo: &Path) -> Result<Option<Fail>> {
    // Capture stderr rather than inheriting it (see `fetch`): a conflicting rebase
    // otherwise dumps git's "CONFLICT …" wall into the middle of the fleet result.
    let out = Command::new("git").arg("-C").arg(repo).args(["pull", "--quiet"]).output()?;
    if out.status.success() {
        return Ok(None);
    }
    // No-ops when nothing is in progress; one of them cleans up on a conflict.
    for op in [["rebase", "--abort"], ["merge", "--abort"]] {
        let _ = Command::new("git").arg("-C").arg(repo).args(op).output();
    }
    Ok(Some(failed(&out)))
}

/// `git push`, returning why it failed (or `None` when it worked).
pub fn push(repo: &Path) -> Result<Option<Fail>> {
    let out = Command::new("git").arg("-C").arg(repo).args(["push", "--quiet"]).output()?;
    Ok((!out.status.success()).then(|| failed(&out)))
}

#[cfg(test)]
mod tests {
    use super::{browser_host, classify, detail, https_to_ssh, remote_to_web, Trouble};

    /// Verbatim stderr from the real transports, one per classified kind. These are
    /// the messages the mark on the dashboard is derived from, so they're worth
    /// pinning: a git release that rewords one shows up here, not as a silently
    /// unclassified repo.
    #[test]
    fn classifies_denied_access() {
        for msg in [
            "git@github.com: Permission denied (publickey).\nfatal: Could not read from remote repository.",
            "remote: ERROR: Permission to owner/repo.git denied to someone.",
            "fatal: Authentication failed for 'https://github.com/owner/repo.git/'",
            "remote: Repository not found.\nfatal: repository 'https://github.com/owner/private.git/' not found",
            "fatal: could not read Username for 'https://github.com': terminal prompts disabled",
            "Host key verification failed.\nfatal: Could not read from remote repository.",
            // GitLab's banner: a private repo you can't see is reported as missing.
            "remote: \nremote: =====\nremote: ERROR: The project you were looking for could not be found or you don't have permission to view it.\nfatal: Could not read from remote repository.",
        ] {
            assert_eq!(classify(msg), Trouble::Denied, "{msg}");
        }
    }

    #[test]
    fn classifies_an_unreachable_remote() {
        for msg in [
            "ssh: Could not resolve hostname github.com: Name or service not known",
            "ssh: connect to host github.com port 22: Connection timed out",
            "ssh: connect to host 127.0.0.1 port 1: Connection refused",
            "fatal: unable to access 'https://example.com/repo.git/': Failed to connect to example.com port 443",
        ] {
            assert_eq!(classify(msg), Trouble::Unreachable, "{msg}");
        }
    }

    #[test]
    fn classifies_what_needs_a_human() {
        for msg in [
            "CONFLICT (content): Merge conflict in src/main.rs\nAutomatic merge failed; fix conflicts and then commit the result.",
            "error: Your local changes to the following files would be overwritten by merge:\n\tsrc/main.rs",
            "error: cannot pull with rebase: You have unstaged changes.",
            " ! [rejected]        main -> main (non-fast-forward)",
            "remote: error: GH006: Protected branch update failed for refs/heads/main.",
            "fatal: Unable to create '/repo/.git/index.lock': File exists.",
        ] {
            assert_eq!(classify(msg), Trouble::NeedsHand, "{msg}");
        }
    }

    #[test]
    fn unrecognized_stderr_falls_through_to_failed() {
        assert_eq!(classify("fatal: the remote end hung up unexpectedly"), Trouble::Failed);
        assert_eq!(classify(""), Trouble::Failed);
    }

    #[test]
    fn detail_keeps_the_explaining_line_and_drops_gits_scaffolding() {
        // `hint:` lines and the `To <remote>` header explain nothing on their own.
        let stderr = "To github.com:owner/repo.git\n ! [rejected]        main -> main (fetch first)\nhint: Updates were rejected because…";
        assert_eq!(detail(stderr), "! [rejected]        main -> main (fetch first)");
        // Prefix and trailing period stripped, so the line reads inside our own sentence.
        assert_eq!(detail("fatal: Authentication failed for 'https://x/y.git/'"), "Authentication failed for 'https://x/y.git/'");
        assert_eq!(detail("remote: Permission to owner/repo.git denied to someone."), "Permission to owner/repo.git denied to someone");
        assert_eq!(detail("   \n\n"), "git failed without an explanation");
    }

    #[test]
    fn detail_walks_past_a_forge_banner_to_the_line_that_explains() {
        // GitLab pads its message with bare `remote:` spacers and `====` rules, and
        // git appends two lines of access-rights boilerplate. The explanation is the
        // one line in the middle, behind two stacked prefixes.
        let stderr = "remote: \n\
                      remote: ========================================================================\n\
                      remote: \n\
                      remote: ERROR: The project you were looking for could not be found or you don't have permission to view it.\n\
                      remote: \n\
                      fatal: Could not read from remote repository.\n\
                      \n\
                      Please make sure you have the correct access rights\n\
                      and the repository exists.";
        assert_eq!(detail(stderr), "The project you were looking for could not be found or you don't have permission to view it");
    }

    #[test]
    fn detail_prefers_the_conflict_line_that_names_the_file() {
        // git reports the conflict on stdout and "could not apply" on stderr; the
        // file name is what the reader needs, so it wins wherever it appears.
        let out = "error: could not apply 44e3910... mine\nCONFLICT (content): Merge conflict in file.txt";
        assert_eq!(detail(out), "CONFLICT (content): Merge conflict in file.txt");
    }

    #[test]
    fn detail_caps_a_pathological_line() {
        let long = format!("fatal: {}", "x".repeat(500));
        let out = detail(&long);
        assert_eq!(out.chars().count(), 118, "capped to 117 chars plus the ellipsis");
        assert!(out.ends_with('…'));
    }

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

    #[test]
    fn a_real_forge_host_is_left_alone() {
        assert_eq!(browser_host("github.com"), "github.com");
        assert_eq!(browser_host("git.company.com"), "git.company.com");
    }

    #[test]
    fn an_unknown_single_label_host_is_left_alone() {
        assert_eq!(browser_host("grove-no-such-ssh-alias"), "grove-no-such-ssh-alias");
    }
}
