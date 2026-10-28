//! `grove ssh`: rewrite the HTTPS remotes of every repo under a folder — nested
//! ones included, per `depth` — to their SSH equivalents, so
//! `lg`/`lgs`/`lgp`/`lgpp` can fetch and sync them (they flag HTTPS remotes and
//! skip them). The switch is trivially reversible with `git remote set-url`, but
//! mutating remotes still earns a yes.
//!
//! [`plan`] finds the rewrites and [`apply`] makes them. Two flows run on top:
//! [`run`] is the human one (preview, ask, apply, dashboard), and [`report`] is
//! the `--json` one, which never asks — `-y` alone decides whether it applies the
//! plan or hands it back as a dry run.
use crate::{git, overview, ui};
use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

/// One remote to switch: a line of the preview, and an element of `--json`'s
/// `planned`/`switched`. `name`/`group` split the same way as a dashboard row.
#[derive(Serialize, Clone)]
pub struct Rewrite {
    pub name: String,
    pub group: Option<String>,
    /// Absolute path to the repo.
    pub path: String,
    pub remote: String,
    /// The HTTPS URL as configured, with any embedded credentials masked
    /// (`https://***@host/…`): the SSH URL drops them, so nothing needs to show
    /// them — least of all a document an agent reads.
    pub from: String,
    pub to: String,
}

impl Rewrite {
    fn label(&self) -> String {
        git::label(self.group.as_deref(), &self.name)
    }

    /// Only name the remote when it isn't the usual `origin`, to keep the common
    /// single-remote case uncluttered.
    fn remote_label(&self) -> String {
        if self.remote == "origin" {
            self.label()
        } else {
            format!("{} ({})", self.label(), self.remote)
        }
    }
}

/// A rewrite git refused, with git's own line for why.
#[derive(Serialize)]
pub struct RewriteFailure {
    #[serde(flatten)]
    pub rewrite: Rewrite,
    pub detail: String,
}

/// Every HTTPS remote under a folder: what `grove ssh` proposes to switch.
pub struct Plan {
    pub dir: PathBuf,
    /// How many repos the scan found, so "no repos here" and "no HTTPS remotes"
    /// read differently.
    pub repos: usize,
    pub rewrites: Vec<Rewrite>,
}

/// The `--json` document: what would be switched and — with `-y` — what was, plus
/// the dashboard afterwards, so an agent can migrate and re-check in one call.
#[derive(Serialize)]
pub struct SshReport {
    pub dir: String,
    /// No `-y`: nothing was changed and nothing fetched; `planned` is what `-y`
    /// would switch.
    pub dry_run: bool,
    pub planned: Vec<Rewrite>,
    pub switched: Vec<Rewrite>,
    pub failed: Vec<RewriteFailure>,
    /// The dashboard after the switch, fetched fresh so each switched repo shows
    /// its real sync state — or the trouble its SSH access runs into. `null` on a
    /// dry run.
    pub overview: Option<overview::Report>,
}

/// Find every HTTPS remote under `dir`. A repo may have more than one — origin
/// and, on a fork, upstream — and each is planned. Reading remotes is a cheap
/// local git call, so the repos fan out.
pub fn plan(dir: Option<&Path>, depth: usize) -> Result<Plan> {
    let dir = dir.unwrap_or_else(|| Path::new("."));
    if !dir.is_dir() {
        anyhow::bail!("not a directory: {}", dir.display());
    }
    let repos = git::discover(dir, depth);
    let mut rewrites: Vec<Rewrite> = repos
        .par_iter()
        .flat_map_iter(|r| {
            let path = std::fs::canonicalize(&r.path).unwrap_or_else(|_| r.path.clone()).display().to_string();
            git::remotes(&r.path).into_iter().filter_map(move |(remote, url)| {
                let to = git::https_to_ssh(&url)?;
                Some(Rewrite {
                    name: r.name.clone(),
                    group: r.group.clone(),
                    path: path.clone(),
                    remote,
                    from: git::mask_credentials(&url),
                    to,
                })
            })
        })
        .collect();
    // The dashboard's order — group, then repo — so the preview reads like the table.
    rewrites.sort_by(|a, b| (&a.group, &a.name, &a.remote).cmp(&(&b.group, &b.name, &b.remote)));
    Ok(Plan { dir: dir.to_path_buf(), repos: repos.len(), rewrites })
}

/// Point each remote at its SSH URL, in order. A rewrite git refuses is kept with
/// git's reason, never dropped from the result.
pub fn apply(rewrites: &[Rewrite]) -> (Vec<Rewrite>, Vec<RewriteFailure>) {
    let mut switched = Vec::new();
    let mut failed = Vec::new();
    for r in rewrites {
        match git::set_remote_url(Path::new(&r.path), &r.remote, &r.to) {
            None => switched.push(r.clone()),
            Some(detail) => failed.push(RewriteFailure { rewrite: r.clone(), detail }),
        }
    }
    (switched, failed)
}

/// The `--json` flow. A machine caller can't answer a prompt, so `yes` (`-y`) is
/// the whole decision: with it the plan is applied and the dashboard re-fetched;
/// without it the document is the dry run, and nothing is touched or fetched.
pub fn report(dir: Option<&Path>, depth: usize, yes: bool) -> Result<SshReport> {
    let plan = plan(dir, depth)?;
    let (switched, failed) = if yes { apply(&plan.rewrites) } else { (Vec::new(), Vec::new()) };
    let overview = if yes { Some(overview::collect(Some(&plan.dir), depth, overview::Fetch::All)?) } else { None };
    Ok(SshReport { dir: plan.dir.display().to_string(), dry_run: !yes, planned: plan.rewrites, switched, failed, overview })
}

/// The human flow: preview every change, ask (unless `assume_yes`), apply, then
/// fetch and print the dashboard so each switched repo visibly flips from the red
/// HTTPS flag to a real sync state.
pub fn run(dir: Option<&Path>, depth: usize, assume_yes: bool, hints: &overview::Hints) -> Result<()> {
    let plan = plan(dir, depth)?;
    if plan.repos == 0 {
        println!("No git repositories in {}", plan.dir.display());
        return Ok(());
    }

    if plan.rewrites.is_empty() {
        println!("{}", ui::paint("90", "No HTTPS remotes to switch."));
        // Still show the dashboard so a bare `grove ssh` doubles as `grove overview`.
        let report = overview::collect(Some(&plan.dir), depth, overview::Fetch::All)?;
        overview::render_human(&report, hints);
        return Ok(());
    }

    preview(&plan.rewrites);

    if !confirm(assume_yes) {
        println!("{}", ui::paint("90", "Left unchanged."));
        return Ok(());
    }

    // set-url is an instant local op, so no progress bar — apply in order and
    // report per repo. The visible network work is the dashboard's fetch below.
    let (switched, failed) = apply(&plan.rewrites);
    for r in &switched {
        println!("  {} {}", ui::paint("32", "✓"), r.remote_label());
    }
    for f in &failed {
        ui::err(&format!("could not switch {} — {}", f.rewrite.remote_label(), f.detail));
    }
    if !failed.is_empty() {
        println!("{}", ui::paint("33", &format!("  {} remote(s) could not be switched.", failed.len())));
    }

    // Fetch the now-SSH repos and print the dashboard — any repo whose SSH auth
    // isn't set up surfaces right away.
    println!();
    let report = overview::collect(Some(&plan.dir), depth, overview::Fetch::All)?;
    overview::render_human(&report, hints);
    Ok(())
}

/// Two lines per change — the URLs are long, so old-above-new reads cleaner than
/// a single wrapped line and makes the rewrite unmistakable.
fn preview(rewrites: &[Rewrite]) {
    let n = rewrites.len();
    let repos = {
        let mut paths: Vec<&str> = rewrites.iter().map(|r| r.path.as_str()).collect();
        paths.dedup();
        paths.len()
    };
    println!();
    println!(
        "  {}",
        ui::paint("1", &format!("will switch {n} remote(s) in {repos} repo(s) to SSH:"))
    );
    for r in rewrites {
        println!("  {} {}", ui::paint("36", "↪"), ui::paint("1", &r.remote_label()));
        println!("      {}", ui::paint("31", &r.from));
        println!("      {} {}", ui::paint("90", "→"), ui::paint("32", &r.to));
    }
    println!();
}

/// Interactive y/N gate. `assume_yes` (`-y`) skips it. When stdin isn't a TTY and
/// `-y` wasn't given (scripts, agents, pipes), refuse rather than block on a read
/// that can't be answered — the preview above already served as a dry-run.
fn confirm(assume_yes: bool) -> bool {
    if assume_yes {
        return true;
    }
    if !io::stdin().is_terminal() {
        println!("{}", ui::paint("90", "Not a terminal — re-run with -y to apply the changes above (or --json for the plan as data)."));
        return false;
    }
    print!("  Proceed? [y/N] ");
    let _ = io::stdout().flush();
    let mut s = String::new();
    if io::stdin().read_line(&mut s).is_err() {
        return false;
    }
    matches!(s.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}
