//! The multi-repo actions, driven off a already-collected [`overview::Report`]:
//! `sync` (the `lgs` alias) fast-forward-pulls the strictly-behind and pushes the
//! strictly-ahead **clean, in-sync** repos; `pull_all` (`lgp`) pulls **every**
//! behind repo — fast-forwarding the strictly-behind and rebasing/merging the
//! diverged per the user's `git pull` config (aborting cleanly on conflict);
//! `push_all` (`lgpp`) pushes every strictly-ahead one. Each reports only the
//! repos that actually moved.
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

/// Behind its upstream at all — strictly behind *or* diverged. `pull-all` pulls
/// these; `git pull` fast-forwards the strictly-behind and rebases/merges the
/// diverged per the user's config (aborting cleanly on conflict).
fn behind(r: &overview::RepoStatus) -> bool {
    !r.https && matches!(r.behind, Some(b) if b > 0)
}

/// `sync` (`lgs`): for every clean, in-sync repo, fast-forward-pull the ones only
/// behind and push the ones only ahead. Dirty, diverged, https, and upstream-less
/// repos are left untouched (use `pull-all` for the diverged ones). Acts in
/// parallel; reports only the repos that actually moved.
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
    let synced: Vec<Synced> = ops
        .par_iter()
        .filter_map(|(r, op)| {
            let ok = match *op {
                "pull" => git::pull(Path::new(&r.path)),
                _ => git::push(Path::new(&r.path)),
            }
            .unwrap_or(false);
            pb.inc(1);
            ok.then(|| Synced { name: r.name.clone(), op })
        })
        .collect();
    pb.finish_and_clear();
    synced
}

/// `pull_all` (`lgp`): pull every repo behind its upstream — strictly-behind ones
/// fast-forward, diverged ones rebase/merge per the user's `git pull` config. On a
/// conflict (or dirty tracked changes blocking a rebase) the pull aborts cleanly
/// and the repo is left untouched, so the fleet never ends up half-applied.
pub fn act_pull_all(report: &overview::Report) -> Vec<String> {
    let to_pull: Vec<&overview::RepoStatus> = report.repos.iter().filter(|r| behind(r)).collect();
    if to_pull.is_empty() {
        return Vec::new();
    }
    let pb = ui::bar(to_pull.len() as u64, "Pulling");
    let pulled: Vec<String> = to_pull
        .par_iter()
        .filter_map(|r| {
            let ok = git::pull(Path::new(&r.path)).unwrap_or(false);
            pb.inc(1);
            ok.then(|| r.name.clone())
        })
        .collect();
    pb.finish_and_clear();
    pulled
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
    let pushed: Vec<String> = to_push
        .par_iter()
        .filter_map(|r| {
            let ok = git::push(Path::new(&r.path)).unwrap_or(false);
            pb.inc(1);
            ok.then(|| r.name.clone())
        })
        .collect();
    pb.finish_and_clear();
    pushed
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
