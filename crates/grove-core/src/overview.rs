//! `overview` (alias lg): a one-screen dashboard of every repo directly under a
//! folder — branch, ahead/behind vs upstream, and staged/modified/untracked
//! counts. Repos are fetched in parallel first; https remotes are flagged (not
//! fetched) so you can switch them to SSH.
use crate::{git, ui};
use anyhow::Result;
use rayon::prelude::*;
use std::path::Path;

// Nerd Font forge marks for the clickable link column. Known SaaS hosts get
// their brand glyph; self-hosted / Gitea / Forgejo / Codeberg / GitHub
// Enterprise / anything else falls back to a generic git logo (their domains
// are arbitrary, so the host can't identify them). Assumes a Nerd Font, exactly
// as `lt` does.
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

struct Row {
    name: String,
    branch: String,
    https: bool,
    url: Option<String>,
    ab: Option<(u32, u32)>,
    dirty: git::Dirty,
}

pub fn run(dir: Option<&Path>) -> Result<()> {
    run_inner(dir, true)
}

/// Like `run`, but assumes the caller already fetched (used by `sync`), so it
/// skips the fetch and its progress bar and just renders current state — avoids
/// a second fetch + second "Fetching" bar during `lgp`.
pub fn run_no_fetch(dir: Option<&Path>) -> Result<()> {
    run_inner(dir, false)
}

fn run_inner(dir: Option<&Path>, fetch: bool) -> Result<()> {
    let dir = dir.unwrap_or_else(|| Path::new("."));
    if !dir.is_dir() {
        anyhow::bail!("not a directory: {}", dir.display());
    }
    let repos = git::discover(dir);
    if repos.is_empty() {
        println!("No git repositories in {}", dir.display());
        return Ok(());
    }

    // Size the bar to the repos we'll actually fetch (ssh only — https are
    // flagged, not fetched), so the count reflects real work: "Fetching 3/8".
    let pb = if fetch {
        let n = repos.iter().filter(|r| !git::is_https(&r.path)).count();
        (n > 0).then(|| ui::bar(n as u64, "Fetching"))
    } else {
        None
    };

    let rows: Vec<Row> = repos
        .par_iter()
        .map(|r| {
            let https = git::is_https(&r.path);
            let branch = git::branch(&r.path);
            let url = git::web_url(&r.path);
            if https {
                return Row { name: r.name.clone(), branch, https, url, ab: None, dirty: git::Dirty::default() };
            }
            if fetch {
                git::fetch(&r.path);
                if let Some(pb) = &pb {
                    pb.inc(1);
                }
            }
            Row {
                name: r.name.clone(),
                branch,
                https,
                url,
                ab: git::ahead_behind(&r.path),
                dirty: git::dirty(&r.path),
            }
        })
        .collect();
    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    render(&rows);
    Ok(())
}

fn render(rows: &[Row]) {
    // Size the Repository and Branch columns to their widest entry (never below
    // the header) so a long name like `opencode-dynamic-custom-providers` can't
    // shove the rest of the row out of alignment. Count chars, not bytes, so a
    // Danish æ/ø/å in a name lines up the same as an ASCII one.
    let width = |header: &str, f: &dyn Fn(&Row) -> usize| {
        rows.iter().map(f).max().unwrap_or(0).max(header.chars().count())
    };
    let name_w = width("Repository", &|r| r.name.chars().count());
    let branch_w = width("Branch", &|r| r.branch.chars().count());

    // Two spaces between every column — a single space read as cramped once the
    // wide URL became a lone glyph.
    let gap = "  ";

    // The forge-link column sits right after the repo name and only appears on
    // terminals that render OSC 8 hyperlinks — otherwise a lone Nerd-Font glyph
    // would be unclickable decoration, so we drop the column entirely rather than
    // show a dead icon. The glyph assumes a Nerd Font, like `lt` (see the brew
    // caveat). When on, the header labels it with a chain-link glyph; each row's
    // cell is a 1-wide forge glyph (or a blank when the repo has no origin), so
    // it always lines up under that header. When off, the whole column vanishes.
    let links = ui::hyperlinks();
    let link_seg = |cell: String| if links { format!("{gap}{cell}") } else { String::new() };
    let row_link = |r: &Row| match &r.url {
        // The glyph is the click target; clicking opens the repo's web page.
        Some(u) => link_seg(ui::link(u, &ui::paint("36", forge_icon(u)))),
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
        // the rows that DO need attention **bold**, so the eye lands on them.
        let calm = !r.https && !r.dirty.any() && matches!(r.ab, Some((0, 0)));
        let name = {
            let padded = format!("{:<name_w$}", r.name);
            if calm { padded } else { ui::paint("1", &padded) }
        };
        let link = row_link(r);
        let branch = ui::paint("34", &format!("{:<branch_w$}", r.branch));

        if r.https {
            println!("  {name}{link}{gap}{branch}{gap}{}", ui::paint("31", "HTTPS — switch to SSH"));
            continue;
        }

        let (sync, color) = match r.ab {
            Some((a, b)) if a > 0 && b > 0 => (format!("↑{a} ↓{b}"), "33"),
            Some((a, _)) if a > 0 => (format!("↑{a}"), "33"),
            Some((_, b)) if b > 0 => (format!("↓{b}"), "31"),
            Some(_) => ("✓".to_string(), "32"),
            None => ("—".to_string(), "37"),
        };

        let mut line = format!("  {name}{link}{gap}{branch}{gap}{}", ui::paint(color, &sync));
        if r.dirty.staged > 0 {
            line += &format!(" {}", ui::paint("32", &format!("+{}", r.dirty.staged)));
        }
        if r.dirty.modified > 0 {
            line += &format!(" {}", ui::paint("33", &format!("!{}", r.dirty.modified)));
        }
        if r.dirty.untracked > 0 {
            line += &format!(" {}", ui::paint("34", &format!("?{}", r.dirty.untracked)));
        }
        println!("{line}");
    }
    summary(rows);
    println!();
}

/// A one-line roll-up under the table — counts toned by severity — plus the exact
/// command to clear each kind of pending work. This is the at-a-glance triage.
fn summary(rows: &[Row]) {
    let https: Vec<&str> = rows.iter().filter(|r| r.https).map(|r| r.name.as_str()).collect();
    let (mut clean, mut dirty, mut ahead, mut behind, mut diverged, mut noup) = (0, 0, 0, 0, 0, 0);
    for r in rows.iter().filter(|r| !r.https) {
        if r.dirty.any() {
            dirty += 1;
        }
        match r.ab {
            Some((a, b)) if a > 0 && b > 0 => diverged += 1,
            Some((a, _)) if a > 0 => ahead += 1,
            Some((_, b)) if b > 0 => behind += 1,
            Some(_) => { if !r.dirty.any() { clean += 1 } }
            None => noup += 1,
        }
    }

    let sep = ui::paint("90", " · ");
    let mut parts = vec![ui::paint("1", &format!("{} repos", rows.len()))];
    let mut add = |cnt: usize, label: &str, color: &str| {
        if cnt > 0 {
            parts.push(ui::paint(color, &format!("{cnt} {label}")));
        }
    };
    add(clean, "clean", "32");
    add(dirty, "dirty", "33");
    add(ahead, "to push", "33");
    add(behind, "to pull", "31");
    add(diverged, "diverged", "31");
    add(https.len(), "https", "31");
    add(noup, "no upstream", "90");
    println!("\n  {}", parts.join(&sep));

    let mut hints: Vec<String> = Vec::new();
    if ahead > 0 {
        hints.push(format!("`lgpp` pushes {ahead} with unpushed commits"));
    }
    if behind > 0 {
        hints.push("`lgp` fast-forwards the clean, behind repos".into());
    }
    if !https.is_empty() {
        hints.push(format!("`grove ssh` switches {} to SSH: {}", https.len(), https.join(", ")));
    }
    if diverged > 0 {
        hints.push(format!("{diverged} diverged — reconcile by hand"));
    }
    for h in hints {
        println!("  {} {}", ui::paint("36", "→"), h);
    }
}
