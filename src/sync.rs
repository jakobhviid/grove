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
    let repos = git::discover(dir);
    if repos.is_empty() {
        println!("No git repos found.");
        return Ok(());
    }

    ui::info("Fetching...");
    repos
        .par_iter()
        .filter(|r| !git::is_https(&r.path))
        .for_each(|r| git::fetch(&r.path));

    for r in &repos {
        if git::is_https(&r.path) || git::dirty(&r.path).any() {
            continue;
        }
        let Some((ahead, behind)) = git::ahead_behind(&r.path) else {
            continue;
        };
        if behind > 0 && ahead == 0 {
            println!("{} {}", ui::paint("32", "↓"), r.name);
            git::pull(&r.path)?;
        } else if ahead > 0 && behind == 0 {
            println!("{} {}", ui::paint("32", "↑"), r.name);
            git::push(&r.path)?;
        }
    }

    overview::run(Some(dir))
}
