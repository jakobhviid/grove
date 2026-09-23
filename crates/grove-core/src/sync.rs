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

/// One repo `sync` touched, and which direction. `name`/`group` split the same
/// way as a dashboard row, so a nested repo is identified the same everywhere.
#[derive(Serialize)]
pub struct Synced {
    pub name: String,
    pub group: Option<String>,
    pub op: &'static str, // "pull" | "push"
}

impl Synced {
    fn new(repo: &overview::RepoStatus, op: &'static str) -> Self {
        Synced { name: repo.name.clone(), group: repo.group.clone(), op }
    }

    fn label(&self) -> String {
        git::label(self.group.as_deref(), &self.name)
    }
}

/// One repo an action tried and failed to move: which way it was going, what kind
/// of trouble, and git's own explanation. A fleet action that skips a repo has to
/// say so — an unexplained absence from the moved list is how "it just didn't pull"
/// happens.
#[derive(Serialize)]
pub struct Failure {
    pub name: String,
    /// The organizing folder the repo sits in, as on its dashboard row.
    pub group: Option<String>,
    /// Absolute path, so the trouble can be stamped onto the re-read dashboard.
    pub path: String,
    pub op: &'static str, // "pull" | "push"
    pub kind: git::Trouble,
    pub detail: String,
}

impl Failure {
    fn new(repo: &overview::RepoStatus, op: &'static str, fail: git::Fail) -> Self {
        Failure {
            name: repo.name.clone(),
            group: repo.group.clone(),
            path: repo.path.clone(),
            op,
            kind: fail.kind,
            detail: fail.detail,
        }
    }

    fn label(&self) -> String {
        git::label(self.group.as_deref(), &self.name)
    }
}

/// What `sync` did, what it couldn't do, plus the dashboard as it stands afterwards.
#[derive(Serialize)]
pub struct SyncReport {
    pub synced: Vec<Synced>,
    pub failed: Vec<Failure>,
    pub overview: overview::Report,
}

/// What `push_all` pushed, what it couldn't, plus the dashboard afterwards. The
/// pushed list names each repo the way the dashboard does (`work/api` when it sits
/// in a group), since a bare name is not unique across groups.
#[derive(Serialize)]
pub struct PushReport {
    pub pushed: Vec<String>,
    pub failed: Vec<Failure>,
    pub overview: overview::Report,
}

/// What `pull_all` pulled, what it couldn't, plus the dashboard afterwards. The
/// pulled list names each repo the way the dashboard does (`work/api` when it sits
/// in a group), since a bare name is not unique across groups.
#[derive(Serialize)]
pub struct PullReport {
    pub pulled: Vec<String>,
    pub failed: Vec<Failure>,
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
pub fn act_sync(report: &overview::Report) -> (Vec<Synced>, Vec<Failure>) {
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
        return (Vec::new(), Vec::new());
    }
    let pb = ui::bar(ops.len() as u64, "Syncing");
    let results: Vec<Result<Synced, Failure>> = ops
        .par_iter()
        .map(|(r, op)| {
            let outcome = match *op {
                "pull" => git::pull(Path::new(&r.path)),
                _ => git::push(Path::new(&r.path)),
            };
            pb.inc(1);
            match outcome {
                Ok(None) => Ok(Synced::new(r, op)),
                Ok(Some(fail)) => Err(Failure::new(r, op, fail)),
                Err(e) => Err(Failure::new(r, op, spawn_failed(&e))),
            }
        })
        .collect();
    pb.finish_and_clear();
    split(results)
}

/// git couldn't even be run (no binary, no permission on the folder) — rare, and
/// still the user's answer, so it becomes a trouble like any other.
fn spawn_failed(e: &anyhow::Error) -> git::Fail {
    git::Fail { kind: git::Trouble::Failed, detail: format!("could not run git: {e}") }
}

/// Partition per-repo outcomes into moved and failed, preserving order.
fn split<T>(results: Vec<Result<T, Failure>>) -> (Vec<T>, Vec<Failure>) {
    let mut moved = Vec::new();
    let mut failed = Vec::new();
    for r in results {
        match r {
            Ok(v) => moved.push(v),
            Err(f) => failed.push(f),
        }
    }
    (moved, failed)
}

/// Re-attach to the post-action dashboard everything the run learned: the trouble
/// the pre-action fetch found, then — winning over it, as the newer and more
/// specific verdict — the repos this action tried and failed to move. Without this
/// the re-read (which deliberately doesn't fetch) would render a fleet that looks
/// untroubled seconds after telling you otherwise.
pub fn settle(after: &mut overview::Report, before: &overview::Report, failed: &[Failure]) {
    overview::carry_trouble(after, before);
    for f in failed {
        if let Some(repo) = after.repos.iter_mut().find(|r| r.path == f.path) {
            repo.trouble = Some(f.kind);
            repo.trouble_detail = Some(f.detail.clone());
        }
    }
    overview::resummarize(after);
}

/// `pull_all` (`lgp`): pull every repo behind its upstream — strictly-behind ones
/// fast-forward, diverged ones rebase/merge per the user's `git pull` config. On a
/// conflict (or dirty tracked changes blocking a rebase) the pull aborts cleanly
/// and the repo is left untouched, so the fleet never ends up half-applied.
pub fn act_pull_all(report: &overview::Report) -> (Vec<String>, Vec<Failure>) {
    let to_pull: Vec<&overview::RepoStatus> = report.repos.iter().filter(|r| behind(r)).collect();
    if to_pull.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let pb = ui::bar(to_pull.len() as u64, "Pulling");
    let results: Vec<Result<String, Failure>> = to_pull
        .par_iter()
        .map(|r| {
            let outcome = git::pull(Path::new(&r.path));
            pb.inc(1);
            match outcome {
                Ok(None) => Ok(r.label()),
                Ok(Some(fail)) => Err(Failure::new(r, "pull", fail)),
                Err(e) => Err(Failure::new(r, "pull", spawn_failed(&e))),
            }
        })
        .collect();
    pb.finish_and_clear();
    split(results)
}

/// `push_all` (`lgpp`): push every repo strictly ahead of its upstream — never
/// pulls, does not require a clean worktree, and skips diverged repos a plain push
/// would reject.
pub fn act_push_all(report: &overview::Report) -> (Vec<String>, Vec<Failure>) {
    let to_push: Vec<&overview::RepoStatus> = report.repos.iter().filter(|r| strictly_ahead(r)).collect();
    if to_push.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let pb = ui::bar(to_push.len() as u64, "Pushing");
    let results: Vec<Result<String, Failure>> = to_push
        .par_iter()
        .map(|r| {
            let outcome = git::push(Path::new(&r.path));
            pb.inc(1);
            match outcome {
                Ok(None) => Ok(r.label()),
                Ok(Some(fail)) => Err(Failure::new(r, "push", fail)),
                Err(e) => Err(Failure::new(r, "push", spawn_failed(&e))),
            }
        })
        .collect();
    pb.finish_and_clear();
    split(results)
}

/// The repos an action couldn't move, listed right where the moved ones are — same
/// shape of line, so what failed is as visible as what worked, with git's reason on
/// it. The dashboard below repeats them as marks; this is the running commentary.
fn render_failures(failed: &[Failure]) {
    for f in failed {
        let (glyph, color) = overview::mark_for(f.kind);
        println!("  {} {} {}", ui::paint(color, glyph), f.label(), ui::paint("90", &format!("— {} failed: {}", f.op, f.detail)));
    }
}

pub fn render_human(report: &SyncReport, hints: &overview::Hints) {
    for item in &report.synced {
        let arrow = if item.op == "pull" { "↓" } else { "↑" };
        println!("  {} {}", ui::paint("32", arrow), item.label());
    }
    render_failures(&report.failed);
    overview::render_human(&report.overview, hints);
}

pub fn render_pull(report: &PullReport, hints: &overview::Hints) {
    if report.pulled.is_empty() && report.failed.is_empty() {
        println!("{}", ui::paint("90", "Nothing to pull."));
    } else {
        for name in &report.pulled {
            println!("  {} {}", ui::paint("32", "↓"), name);
        }
        render_failures(&report.failed);
    }
    overview::render_human(&report.overview, hints);
}

pub fn render_push(report: &PushReport, hints: &overview::Hints) {
    if report.pushed.is_empty() && report.failed.is_empty() {
        println!("{}", ui::paint("90", "Nothing to push."));
    } else {
        for name in &report.pushed {
            println!("  {} {}", ui::paint("32", "↑"), name);
        }
        render_failures(&report.failed);
    }
    overview::render_human(&report.overview, hints);
}
