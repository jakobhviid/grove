//! `sync` (the `lgp` alias): for every clean, in-sync repo under a folder,
//! fast-forward pull the ones that are only behind and push the ones that are
//! only ahead — then show the overview. Repos that are dirty, diverged, https,
//! or without an upstream are left untouched. `push_all` (the `lgpp` alias) is
//! the push-only variant.
//!
//! Both return a serializable report (the machine result behind `--json`) that
//! embeds the post-run [`overview::Report`]; [`render_human`]/[`render_push`]
//! paint them.
use crate::{git, overview, ui};
use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;
use std::path::Path;

/// One repo `sync` touched, and which direction.
#[derive(Serialize)]
pub struct Synced {
    pub name: String,
    pub op: &'static str, // "pull" | "push"
}

/// What `sync` did, plus the dashboard as it stands afterwards.
#[derive(Serialize)]
pub struct SyncReport {
    pub synced: Vec<Synced>,
    pub overview: overview::Report,
}

/// What `push_all` pushed, plus the dashboard afterwards.
#[derive(Serialize)]
pub struct PushReport {
    pub pushed: Vec<String>,
    pub overview: overview::Report,
}

pub fn run(dir: Option<&Path>) -> Result<SyncReport> {
    let dir = dir.unwrap_or_else(|| Path::new("."));
    if !dir.is_dir() {
        anyhow::bail!("not a directory: {}", dir.display());
    }
    let repos = git::discover(dir);

    // Fetch every ssh repo in parallel, with a progress bar — this is the slow
    // part, since each fetch is a network round-trip.
    let to_fetch: Vec<_> = repos.iter().filter(|r| !git::is_https(&r.path)).collect();
    if !to_fetch.is_empty() {
        let pb = ui::bar(to_fetch.len() as u64, "Fetching");
        to_fetch.par_iter().for_each(|r| {
            git::fetch(&r.path);
            pb.inc(1);
        });
        pb.finish_and_clear();
    }

    // Decide what actually needs syncing: clean, ssh repos that are strictly
    // behind (→ pull) or strictly ahead (→ push). Determined in parallel — cheap
    // local git calls.
    #[derive(Clone, Copy)]
    enum Op {
        Pull,
        Push,
    }
    let to_sync: Vec<(&git::Repo, Op)> = repos
        .par_iter()
        .filter_map(|r| {
            if git::is_https(&r.path) || git::dirty(&r.path).any() {
                return None;
            }
            let (ahead, behind) = git::ahead_behind(&r.path)?;
            if behind > 0 && ahead == 0 {
                Some((r, Op::Pull))
            } else if ahead > 0 && behind == 0 {
                Some((r, Op::Push))
            } else {
                None
            }
        })
        .collect();

    // The pull/push is the part that actually transfers data (and runs quiet),
    // so give it its own progress bar.
    let mut synced: Vec<Synced> = Vec::new();
    if !to_sync.is_empty() {
        let pb = ui::bar(to_sync.len() as u64, "Syncing");
        to_sync.par_iter().for_each(|(r, op)| {
            let _ = match op {
                Op::Pull => git::pull(&r.path),
                Op::Push => git::push(&r.path),
            };
            pb.inc(1);
        });
        pb.finish_and_clear();
        synced = to_sync
            .iter()
            .map(|(r, op)| Synced {
                name: r.name.clone(),
                op: match op {
                    Op::Pull => "pull",
                    Op::Push => "push",
                },
            })
            .collect();
    }

    // We already fetched above, so collect the post-sync state without a second
    // fetch (no duplicate "Fetching" bar).
    let overview = overview::collect(Some(dir), false)?;
    Ok(SyncReport { synced, overview })
}

pub fn render_human(report: &SyncReport) {
    for item in &report.synced {
        let arrow = if item.op == "pull" { "↓" } else { "↑" };
        println!("  {} {}", ui::paint("32", arrow), item.name);
    }
    overview::render_human(&report.overview);
}

/// `push_all` (the `lgpp` alias): push every repo under a folder that has
/// unpushed commits — strictly ahead of its upstream. Unlike `sync` it never
/// pulls and does NOT require a clean worktree (pushing committed work is safe
/// even with uncommitted changes present). Repos that are up-to-date, behind,
/// diverged, https, or have no upstream are left alone.
pub fn push_all(dir: Option<&Path>) -> Result<PushReport> {
    let dir = dir.unwrap_or_else(|| Path::new("."));
    if !dir.is_dir() {
        anyhow::bail!("not a directory: {}", dir.display());
    }
    let repos = git::discover(dir);

    // Fetch first so ahead/behind is accurate (don't push into a diverged remote).
    let to_fetch: Vec<_> = repos.iter().filter(|r| !git::is_https(&r.path)).collect();
    if !to_fetch.is_empty() {
        let pb = ui::bar(to_fetch.len() as u64, "Fetching");
        to_fetch.par_iter().for_each(|r| {
            git::fetch(&r.path);
            pb.inc(1);
        });
        pb.finish_and_clear();
    }

    // Only strictly-ahead repos have something to push (skip diverged — a plain
    // push would be rejected).
    let to_push: Vec<&git::Repo> = repos
        .par_iter()
        .filter(|r| {
            !git::is_https(&r.path)
                && matches!(git::ahead_behind(&r.path), Some((ahead, behind)) if ahead > 0 && behind == 0)
        })
        .collect();

    let mut pushed: Vec<String> = Vec::new();
    if !to_push.is_empty() {
        let pb = ui::bar(to_push.len() as u64, "Pushing");
        to_push.par_iter().for_each(|r| {
            let _ = git::push(&r.path);
            pb.inc(1);
        });
        pb.finish_and_clear();
        pushed = to_push.iter().map(|r| r.name.clone()).collect();
    }

    let overview = overview::collect(Some(dir), false)?;
    Ok(PushReport { pushed, overview })
}

pub fn render_push(report: &PushReport) {
    if report.pushed.is_empty() {
        println!("{}", ui::paint("90", "Nothing to push."));
    } else {
        for name in &report.pushed {
            println!("  {} {}", ui::paint("32", "↑"), name);
        }
    }
    overview::render_human(&report.overview);
}
