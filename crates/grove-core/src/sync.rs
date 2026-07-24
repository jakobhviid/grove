//! `sync` (alias lgp): for every clean, in-sync repo under a folder, fast-forward
//! pull the ones that are only behind and push the ones that are only ahead —
//! then print the overview. Repos that are dirty, diverged, https, or without an
//! upstream are left untouched.
use crate::{git, overview, ui};
use anyhow::Result;
use rayon::prelude::*;
use std::path::Path;

pub fn run(dir: Option<&Path>) -> Result<()> {
    let dir = dir.unwrap_or_else(|| Path::new("."));
    if !dir.is_dir() {
        anyhow::bail!("not a directory: {}", dir.display());
    }
    let repos = git::discover(dir);
    if repos.is_empty() {
        println!("No git repositories in {}", dir.display());
        return Ok(());
    }

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
    // so give it its own progress bar, then report what was synced.
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
        for (r, op) in &to_sync {
            let arrow = match op {
                Op::Pull => "↓",
                Op::Push => "↑",
            };
            println!("  {} {}", ui::paint("32", arrow), r.name);
        }
    }

    // sync already fetched above — render without fetching again (no 2nd bar).
    overview::run_no_fetch(Some(dir))
}
