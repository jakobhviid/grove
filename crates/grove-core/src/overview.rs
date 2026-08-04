//! `overview` (the `lg` alias): a one-screen dashboard of every repo directly
//! under a folder — branch, ahead/behind vs upstream, and staged/modified/
//! untracked counts. Repos are fetched in parallel first; https remotes are
//! flagged (not fetched) so you can switch them to SSH.
//!
//! Split in two so the CLI can render either surface without the logic knowing
//! which: [`collect`] gathers the state into a serializable [`Report`] (this is
//! the machine result behind `--json`), and [`render_human`] paints the table.
use crate::{git, ui};
use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;
use std::path::Path;

// Nerd Font forge marks for the clickable link column. Known SaaS hosts get
// their brand glyph; self-hosted / Gitea / Forgejo / Codeberg / GitHub
// Enterprise / anything else falls back to a generic git logo (their domains
// are arbitrary, so the host can't identify them). Assumes a Nerd Font, exactly
// as `tree` does.
const ICON_GITHUB: &str = "\u{f09b}"; //  (octocat)
const ICON_GITLAB: &str = "\u{f296}"; //  (fox)
const ICON_BITBUCKET: &str = "\u{f171}"; //
const ICON_GIT: &str = "\u{e702}"; //  (generic git)
const ICON_LINK: &str = "\u{f0c1}"; //  (chain link — the column header)

/// Pick the forge glyph for a repo's https web URL (`https://host/owner/repo`).
fn forge_icon(web_url: &str) -> &'static str {
    let host = web_url
        .strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("")
        .to_ascii_lowercase();
    if host == "github.com" || host.ends_with(".github.com") {
        ICON_GITHUB
    } else if host.contains("gitlab") {
        ICON_GITLAB
    } else if host.contains("bitbucket") {
        ICON_BITBUCKET
    } else {
        ICON_GIT
    }
}

/// One repo's state — the row of the dashboard, and one element of the `--json`
/// document. `ahead`/`behind` are both `null` when there is no upstream.
#[derive(Serialize)]
pub struct RepoStatus {
    pub name: String,
    /// Absolute path to the repo — the target of the clickable-name `file://`
    /// link, and handy for `--json` consumers that want to act on the repo.
    pub path: String,
    pub branch: String,
    /// origin is still on https (flagged, never fetched).
    pub https: bool,
    /// browser URL for origin, if it resolves to one.
    pub web_url: Option<String>,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub staged: u32,
    pub modified: u32,
    pub untracked: u32,
}

impl RepoStatus {
    fn dirty(&self) -> bool {
        self.staged > 0 || self.modified > 0 || self.untracked > 0
    }

    /// A fully-clean, in-sync ssh repo — nothing needs attention, so the human
    /// table leaves it un-bolded.
    fn calm(&self) -> bool {
        !self.https && !self.dirty() && matches!((self.ahead, self.behind), (Some(0), Some(0)))
    }
}

/// Roll-up counts under the table. Every repo lands in exactly one sync bucket
/// (`clean`/`ahead`/`behind`/`diverged`/`https`/`no_upstream`); `dirty` is an
/// independent overlay (a repo can be both dirty and ahead).
#[derive(Serialize)]
pub struct Summary {
    pub repos: usize,
    pub clean: usize,
    pub dirty: usize,
    pub ahead: usize,
    pub behind: usize,
    pub diverged: usize,
    pub https: usize,
    pub no_upstream: usize,
}

/// The whole dashboard: the folder, every repo under it, and the roll-up. This
/// is what `--json` serializes.
#[derive(Serialize)]
pub struct Report {
    pub dir: String,
    pub repos: Vec<RepoStatus>,
    pub summary: Summary,
}

/// How the next-step `→` hints should name the commands that clear pending work.
/// The binary fills each field with the alias the user actually bound in their
/// grove file (e.g. `lgpp` for `grove push-all`), or `None` when they haven't —
/// then the hint falls back to the long `grove …` form. `configured` is false
/// when there's no grove file at all, which turns on a one-line `grove setup`
/// nudge. Defaulting every field (`Hints::default()`) yields the long forms with
/// no nudge — the right behavior for `--json` callers, which never render hints.
#[derive(Default)]
pub struct Hints {
    /// Alias bound to `grove pull-all` (default `lgp`), if any.
    pub pull_all: Option<String>,
    /// Alias bound to `grove push-all` (default `lgpp`), if any.
    pub push_all: Option<String>,
    /// Alias bound to `grove ssh` (there is no default alias for it), if any.
    pub ssh: Option<String>,
    /// Whether a grove file exists (aliases are provisioned); gates the nudge.
    pub configured: bool,
}

/// The command token a hint should show: the user's short alias in backticks
/// when they have one, else the long `grove …` form.
fn token(alias: &Option<String>, long: &str) -> String {
    match alias {
        Some(a) => format!("`{a}`"),
        None => format!("`{long}`"),
    }
}

/// Discover the repos directly under `dir` and read each one's state, fetching
/// the ssh repos first in parallel when `fetch`. Pure data: prints nothing but
/// the shared "Fetching" progress bar (stderr). Render with [`render_human`], or
/// serialize the [`Report`] as JSON.
pub fn collect(dir: Option<&Path>, fetch: bool) -> Result<Report> {
    let dir = dir.unwrap_or_else(|| Path::new("."));
    if !dir.is_dir() {
        anyhow::bail!("not a directory: {}", dir.display());
    }
    let repos = git::discover(dir);

    // Size the bar to the repos we'll actually fetch (ssh only — https are
    // flagged, not fetched), so the count reflects real work: "Fetching 3/8".
    let pb = if fetch {
        let n = repos.iter().filter(|r| !git::is_https(&r.path)).count();
        (n > 0).then(|| ui::bar(n as u64, "Fetching"))
    } else {
        None
    };

    let repos: Vec<RepoStatus> = repos
        .par_iter()
        .map(|r| {
            let https = git::is_https(&r.path);
            let branch = git::branch(&r.path);
            let web_url = git::web_url(&r.path);
            // Absolute path for the clickable-name file:// link; fall back to the
            // discovered path if canonicalization fails (e.g. a race removing it).
            let path = std::fs::canonicalize(&r.path).unwrap_or_else(|_| r.path.clone()).display().to_string();
            if https {
                return RepoStatus {
                    name: r.name.clone(),
                    path,
                    branch,
                    https,
                    web_url,
                    ahead: None,
                    behind: None,
                    staged: 0,
                    modified: 0,
                    untracked: 0,
                };
            }
            if fetch {
                git::fetch(&r.path);
                if let Some(pb) = &pb {
                    pb.inc(1);
                }
            }
            let ab = git::ahead_behind(&r.path);
            let dirty = git::dirty(&r.path);
            RepoStatus {
                name: r.name.clone(),
                path,
                branch,
                https,
                web_url,
                ahead: ab.map(|(ahead, _)| ahead),
                behind: ab.map(|(_, behind)| behind),
                staged: dirty.staged,
                modified: dirty.modified,
                untracked: dirty.untracked,
            }
        })
        .collect();
    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    let summary = summarize(&repos);
    Ok(Report { dir: dir.display().to_string(), repos, summary })
}

/// Tally every repo into the roll-up buckets. Kept in core (it's classification
/// logic, not rendering) so `--json` and the human roll-up agree by construction.
fn summarize(repos: &[RepoStatus]) -> Summary {
    let mut summary = Summary { repos: repos.len(), clean: 0, dirty: 0, ahead: 0, behind: 0, diverged: 0, https: 0, no_upstream: 0 };
    for repo in repos {
        if repo.https {
            summary.https += 1;
            continue;
        }
        if repo.dirty() {
            summary.dirty += 1;
        }
        match (repo.ahead, repo.behind) {
            (Some(ahead), Some(behind)) if ahead > 0 && behind > 0 => summary.diverged += 1,
            (Some(ahead), _) if ahead > 0 => summary.ahead += 1,
            (_, Some(behind)) if behind > 0 => summary.behind += 1,
            (Some(_), Some(_)) => {
                if !repo.dirty() {
                    summary.clean += 1;
                }
            }
            _ => summary.no_upstream += 1,
        }
    }
    summary
}

/// Paint the dashboard for a human: the aligned, colored table plus the roll-up
/// and next-step hints. `hints` decides whether those hints name the user's short
/// aliases or the long `grove …` forms. `--json` callers skip this and serialize
/// the [`Report`].
pub fn render_human(report: &Report, hints: &Hints) {
    if report.repos.is_empty() {
        println!("No git repositories in {}", report.dir);
        return;
    }
    let rows = &report.repos;

    // Size the Repository and Branch columns to their widest entry (never below
    // the header) so a long name like `opencode-dynamic-custom-providers` can't
    // shove the rest of the row out of alignment. Count chars, not bytes, so a
    // Danish æ/ø/å in a name lines up the same as an ASCII one.
    let width = |header: &str, field: &dyn Fn(&RepoStatus) -> usize| {
        rows.iter().map(field).max().unwrap_or(0).max(header.chars().count())
    };
    let name_w = width("Repository", &|r| r.name.chars().count());
    let branch_w = width("Branch", &|r| r.branch.chars().count());

    // Two spaces between every column — a single space read as cramped once the
    // wide URL became a lone glyph.
    let gap = "  ";

    // The forge-link column sits right after the repo name and only appears on
    // terminals that render OSC 8 hyperlinks — otherwise a lone Nerd-Font glyph
    // would be unclickable decoration, so we drop the column entirely rather than
    // show a dead icon. The glyph assumes a Nerd Font, like `tree` (see the brew
    // caveat). When on, the header labels it with a chain-link glyph; each row's
    // cell is a 1-wide forge glyph (or a blank when the repo has no origin), so
    // it always lines up under that header. When off, the whole column vanishes.
    let links = ui::hyperlinks();
    let link_seg = |cell: String| if links { format!("{gap}{cell}") } else { String::new() };
    let row_link = |r: &RepoStatus| match &r.web_url {
        // The glyph is the click target; clicking opens the repo's web page.
        Some(url) => link_seg(ui::link(url, &ui::paint("36", forge_icon(url)))),
        // No origin: a blank keeps the Branch column aligned under the header.
        None => link_seg(" ".to_string()),
    };

    println!();
    println!(
        "  {}",
        ui::paint("1", &format!("{:<name_w$}{}{gap}{:<branch_w$}{gap}Status", "Repository", link_seg(ICON_LINK.to_string()), "Branch"))
    );
    println!(
        "  {}",
        ui::paint("90", &format!("{}{}{gap}{}{gap}──────", "─".repeat(name_w), link_seg("─".to_string()), "─".repeat(branch_w)))
    );

    for r in rows {
        // A fully-clean, in-sync ssh repo needs no attention. Rather than dim the
        // clean rows (which vanish on a dark terminal), keep them normal and make
        // the rows that DO need attention **bold**, so the eye lands on them. The
        // name is also a file:// link that opens the repo folder (the counterpart
        // to the forge glyph, which opens its web page) — on terminals that render
        // OSC 8; elsewhere it's plain text.
        let name = {
            let padded = format!("{:<name_w$}", r.name);
            let painted = if r.calm() { padded } else { ui::paint("1", &padded) };
            if links && !r.path.is_empty() { ui::open(&r.path, &painted) } else { painted }
        };
        let link = row_link(r);
        let branch = ui::paint("34", &format!("{:<branch_w$}", r.branch));

        if r.https {
            println!("  {name}{link}{gap}{branch}{gap}{}", ui::paint("31", "HTTPS — switch to SSH"));
            continue;
        }

        let (sync, color) = match (r.ahead, r.behind) {
            (Some(ahead), Some(behind)) if ahead > 0 && behind > 0 => (format!("↑{ahead} ↓{behind}"), "33"),
            (Some(ahead), _) if ahead > 0 => (format!("↑{ahead}"), "33"),
            (_, Some(behind)) if behind > 0 => (format!("↓{behind}"), "31"),
            (Some(_), Some(_)) => ("✓".to_string(), "32"),
            _ => ("—".to_string(), "37"),
        };

        let mut line = format!("  {name}{link}{gap}{branch}{gap}{}", ui::paint(color, &sync));
        if r.staged > 0 {
            line += &format!(" {}", ui::paint("32", &format!("+{}", r.staged)));
        }
        if r.modified > 0 {
            line += &format!(" {}", ui::paint("33", &format!("!{}", r.modified)));
        }
        if r.untracked > 0 {
            line += &format!(" {}", ui::paint("34", &format!("?{}", r.untracked)));
        }
        println!("{line}");
    }
    render_summary(report, hints);
    println!();
}

/// A one-line roll-up under the table — counts toned by severity — plus the exact
/// command to clear each kind of pending work. This is the at-a-glance triage.
fn render_summary(report: &Report, hints: &Hints) {
    let summary = &report.summary;
    let https_names: Vec<&str> = report.repos.iter().filter(|r| r.https).map(|r| r.name.as_str()).collect();

    let sep = ui::paint("90", " · ");
    let mut parts = vec![ui::paint("1", &format!("{} repos", summary.repos))];
    let mut add = |count: usize, label: &str, color: &str| {
        if count > 0 {
            parts.push(ui::paint(color, &format!("{count} {label}")));
        }
    };
    add(summary.clean, "clean", "32");
    add(summary.dirty, "dirty", "33");
    add(summary.ahead, "to push", "33");
    add(summary.behind, "to pull", "31");
    add(summary.diverged, "diverged", "31");
    add(summary.https, "https", "31");
    add(summary.no_upstream, "no upstream", "90");
    println!("\n  {}", parts.join(&sep));

    // Each hint names the command that clears that kind of work, in the caller's
    // preferred form: the user's short alias when they have one (`lgpp`), else the
    // long `grove …` verb. The direction-specific verbs pull-all/push-all mirror
    // the behind/ahead counts exactly; `ssh` has no default alias, so it stays long.
    let mut lines: Vec<String> = Vec::new();
    if summary.ahead > 0 {
        lines.push(format!("{} pushes {} with unpushed commits", token(&hints.push_all, "grove push-all"), summary.ahead));
    }
    if summary.behind > 0 {
        lines.push(format!("{} fast-forward-pulls {} behind {}", token(&hints.pull_all, "grove pull-all"), summary.behind, if summary.behind == 1 { "repo" } else { "repos" }));
    }
    if !https_names.is_empty() {
        lines.push(format!("{} switches {} to SSH: {}", token(&hints.ssh, "grove ssh"), https_names.len(), https_names.join(", ")));
    }
    if summary.diverged > 0 {
        lines.push(format!("{} diverged — reconcile by hand", summary.diverged));
    }
    for line in lines {
        println!("  {} {}", ui::paint("36", "→"), line);
    }
    // When aliases aren't provisioned yet, the hints above showed the long forms —
    // point out that `grove setup` installs the short ones. Dropped once set up.
    if !hints.configured {
        println!("  {} {}", ui::paint("90", "→"), ui::paint("90", "tip: `grove setup` installs the short aliases (lg lgs lgp lgpp lt)"));
    }
}

#[cfg(test)]
mod tests {
    use super::token;

    #[test]
    fn token_prefers_the_bound_alias_else_the_long_form() {
        assert_eq!(token(&Some("lgpp".to_string()), "grove push-all"), "`lgpp`");
        assert_eq!(token(&None, "grove push-all"), "`grove push-all`");
    }
}
