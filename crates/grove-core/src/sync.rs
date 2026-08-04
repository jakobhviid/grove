//! The multi-repo actions, driven off a already-collected [`overview::Report`]:
//! `sync` (the `lgs` alias) fast-forward-pulls the strictly-behind and pushes the
//! strictly-ahead **clean, in-sync** repos; `pull_all` (`lgp`) fast-forwards every
//! strictly-behind repo; `push_all` (`lgpp`) pushes every strictly-ahead one. So
//! `sync` ≈ `pull_all` + `push_all` restricted to the clean repos.
//!
//! Fetching already happened in [`overview::collect`] (which fills the report),
//! so these act purely on that report's ahead/behind — no network round-trip to
//! decide, just the pull/push transfers. The binary re-collects with
//! [`overview::Fetch::None`] afterward to show the post-action state, and pairs the
//! result with [`render_human`]/[`render_push`]/[`render_pull`].
use crate::{git, overview, ui};
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

/// What `pull_all` fast-forwarded, plus the dashboard afterwards.
#[derive(Serialize)]
pub struct PullReport {
    pub pulled: Vec<String>,
    pub overview: overview::Report,
}

fn strictly_behind(r: &overview::RepoStatus) -> bool {
    !r.https && matches!((r.ahead, r.behind), (Some(0), Some(b)) if b > 0)
}

fn strictly_ahead(r: &overview::RepoStatus) -> bool {
    !r.https && matches!((r.ahead, r.behind), (Some(a), Some(0)) if a > 0)
}

/// `sync` (`lgs`): for every clean, in-sync repo, fast-forward-pull the ones only
/// behind and push the ones only ahead. Dirty, diverged, https, and upstream-less
/// repos are left untouched. Acts in parallel; swallows individual git errors (the
/// re-collected overview shows what actually moved).
pub fn act_sync(report: &overview::Report) -> Vec<Synced> {
    let ops: Vec<(&overview::RepoStatus, &'static str)> = report
        .repos
        .iter()
        .filter(|r| !r.dirty())
        .filter_map(|r| {
            if strictly_behind(r) {
                Some((r, "pull"))
            } else if strictly_ahead(r) {
                Some((r, "push"))
            } else {
                None
            }
        })
        .collect();
    if ops.is_empty() {
        return Vec::new();
    }
    let pb = ui::bar(ops.len() as u64, "Syncing");
    ops.par_iter().for_each(|(r, op)| {
        let _ = match *op {
            "pull" => git::pull(Path::new(&r.path)),
            _ => git::push(Path::new(&r.path)),
        };
        pb.inc(1);
    });
    pb.finish_and_clear();
    ops.iter().map(|(r, op)| Synced { name: r.name.clone(), op }).collect()
}

/// `pull_all` (`lgp`): fast-forward every repo strictly behind its upstream. A
/// pure ff (no local commits to reconcile); git refuses any that would clobber
/// uncommitted changes, so a dirty behind repo is attempted and simply left as-is.
pub fn act_pull_all(report: &overview::Report) -> Vec<String> {
    let to_pull: Vec<&overview::RepoStatus> = report.repos.iter().filter(|r| strictly_behind(r)).collect();
    if to_pull.is_empty() {
        return Vec::new();
    }
    let pb = ui::bar(to_pull.len() as u64, "Pulling");
    to_pull.par_iter().for_each(|r| {
        let _ = git::pull(Path::new(&r.path));
        pb.inc(1);
    });
    pb.finish_and_clear();
    to_pull.iter().map(|r| r.name.clone()).collect()
}

/// `push_all` (`lgpp`): push every repo strictly ahead of its upstream — never
/// pulls, does not require a clean worktree, and skips diverged repos a plain push
/// would reject.
pub fn act_push_all(report: &overview::Report) -> Vec<String> {
    let to_push: Vec<&overview::RepoStatus> = report.repos.iter().filter(|r| strictly_ahead(r)).collect();
    if to_push.is_empty() {
        return Vec::new();
    }
    let pb = ui::bar(to_push.len() as u64, "Pushing");
    to_push.par_iter().for_each(|r| {
        let _ = git::push(Path::new(&r.path));
        pb.inc(1);
    });
    pb.finish_and_clear();
    to_push.iter().map(|r| r.name.clone()).collect()
}

pub fn render_human(report: &SyncReport, hints: &overview::Hints) {
    for item in &report.synced {
        let arrow = if item.op == "pull" { "↓" } else { "↑" };
        println!("  {} {}", ui::paint("32", arrow), item.name);
    }
    overview::render_human(&report.overview, hints);
}

pub fn render_pull(report: &PullReport, hints: &overview::Hints) {
    if report.pulled.is_empty() {
        println!("{}", ui::paint("90", "Nothing to pull."));
    } else {
        for name in &report.pulled {
            println!("  {} {}", ui::paint("32", "↓"), name);
        }
    }
    overview::render_human(&report.overview, hints);
}

pub fn render_push(report: &PushReport, hints: &overview::Hints) {
    if report.pushed.is_empty() {
        println!("{}", ui::paint("90", "Nothing to push."));
    } else {
        for name in &report.pushed {
            println!("  {} {}", ui::paint("32", "↑"), name);
        }
    }
    overview::render_human(&report.overview, hints);
}
