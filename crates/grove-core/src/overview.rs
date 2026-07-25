//! `overview` (alias lg): a one-screen dashboard of every repo directly under a
//! folder — branch, ahead/behind vs upstream, and staged/modified/untracked
//! counts. Repos are fetched in parallel first; https remotes are flagged (not
//! fetched) so you can switch them to SSH.
use crate::{git, ui};
use anyhow::Result;
use rayon::prelude::*;
use std::path::Path;

struct Row {
    name: String,
    branch: String,
    https: bool,
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
            if https {
                return Row { name: r.name.clone(), branch, https, ab: None, dirty: git::Dirty::default() };
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
    println!();
    println!(
        "  {}",
        ui::paint("1", &format!("{:<25} {:<14} {}", "Repository", "Branch", "Status"))
    );
    println!(
        "  {}",
        ui::paint("90", &format!("{:<25} {:<14} {}", "─".repeat(25), "─".repeat(14), "──────"))
    );

    for r in rows {
        // A fully-clean, in-sync ssh repo needs no attention — dim its name so the
        // eye jumps to the repos that do.
        let calm = !r.https && !r.dirty.any() && matches!(r.ab, Some((0, 0)));
        let name = {
            let padded = format!("{:<25}", r.name);
            if calm { ui::paint("90", &padded) } else { padded }
        };
        let branch = ui::paint("34", &format!("{:<14}", r.branch));

        if r.https {
            println!("  {name} {branch} {}", ui::paint("31", "HTTPS — switch to SSH"));
            continue;
        }

        let (sync, color) = match r.ab {
            Some((a, b)) if a > 0 && b > 0 => (format!("↑{a} ↓{b}"), "33"),
            Some((a, _)) if a > 0 => (format!("↑{a}"), "33"),
            Some((_, b)) if b > 0 => (format!("↓{b}"), "31"),
            Some(_) => ("✓".to_string(), if calm { "90" } else { "32" }),
            None => ("—".to_string(), "37"),
        };

        let mut line = format!("  {name} {branch} {}", ui::paint(color, &sync));
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
        hints.push(format!("switch to SSH: {}", https.join(", ")));
    }
    if diverged > 0 {
        hints.push(format!("{diverged} diverged — reconcile by hand"));
    }
    for h in hints {
        println!("  {} {}", ui::paint("36", "→"), h);
    }
}
