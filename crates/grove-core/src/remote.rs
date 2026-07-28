//! `switch_ssh` (grove ssh): rewrite the HTTPS remotes of every repo under a
//! folder to their SSH equivalents, so `lg`/`lgp`/`lgpp` can fetch and sync them
//! (they flag HTTPS remotes and skip them). Previews every change and asks for
//! confirmation before touching any config; the switch is trivially reversible
//! with `git remote set-url`, but mutating remotes still earns an explicit yes.
use crate::{git, overview, ui};
use anyhow::Result;
use rayon::prelude::*;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

struct Change {
    repo: String,
    path: PathBuf,
    remote: String,
    from: String,
    to: String,
}

pub fn run(dir: Option<&Path>, assume_yes: bool) -> Result<()> {
    let dir = dir.unwrap_or_else(|| Path::new("."));
    if !dir.is_dir() {
        anyhow::bail!("not a directory: {}", dir.display());
    }
    let repos = git::discover(dir);
    if repos.is_empty() {
        println!("No git repositories in {}", dir.display());
        return Ok(());
    }

    // Gather every HTTPS remote across all repos (reading remotes is a cheap
    // local git call, so fan out). A repo may have more than one — origin and,
    // on a fork, upstream — and we rewrite each.
    let mut changes: Vec<Change> = repos
        .par_iter()
        .flat_map_iter(|r| {
            git::remotes(&r.path).into_iter().filter_map(move |(remote, url)| {
                let to = git::https_to_ssh(&url)?;
                Some(Change { repo: r.name.clone(), path: r.path.clone(), remote, from: url, to })
            })
        })
        .collect();
    changes.sort_by(|a, b| a.repo.cmp(&b.repo).then(a.remote.cmp(&b.remote)));

    if changes.is_empty() {
        println!("{}", ui::paint("90", "No HTTPS remotes to switch."));
        // Still show the dashboard so a bare `grove ssh` doubles as `lg`.
        return overview::run(Some(dir));
    }

    preview(&changes);

    if !confirm(assume_yes) {
        println!("{}", ui::paint("90", "Left unchanged."));
        return Ok(());
    }

    // set-url is an instant local op, so no progress bar — apply in order and
    // report per repo. The visible network work is the dashboard's fetch below.
    let mut failed = 0;
    for c in &changes {
        if git::set_remote_url(&c.path, &c.remote, &c.to) {
            println!("  {} {}", ui::paint("32", "✓"), remote_label(c));
        } else {
            failed += 1;
            ui::err(&format!("could not switch {} ({})", c.repo, c.remote));
        }
    }
    if failed > 0 {
        println!("{}", ui::paint("33", &format!("  {failed} remote(s) could not be switched.")));
    }

    // Fetch the now-SSH repos and print the dashboard, so each switched repo
    // visibly flips from the red HTTPS flag to a real sync state — and any repo
    // whose SSH auth isn't set up surfaces right away.
    println!();
    overview::run(Some(dir))
}

/// Only name the remote when it isn't the usual `origin`, to keep the common
/// single-remote case uncluttered.
fn remote_label(c: &Change) -> String {
    if c.remote == "origin" {
        c.repo.clone()
    } else {
        format!("{} ({})", c.repo, c.remote)
    }
}

/// Two lines per change — the URLs are long, so old-above-new reads cleaner than
/// a single wrapped line and makes the rewrite unmistakable.
fn preview(changes: &[Change]) {
    let n = changes.len();
    let repos = {
        let mut names: Vec<&str> = changes.iter().map(|c| c.repo.as_str()).collect();
        names.dedup();
        names.len()
    };
    println!();
    println!(
        "  {}",
        ui::paint("1", &format!("will switch {n} remote(s) in {repos} repo(s) to SSH:"))
    );
    for c in changes {
        println!("  {} {}", ui::paint("36", "↪"), ui::paint("1", &remote_label(c)));
        println!("      {}", ui::paint("31", &c.from));
        println!("      {} {}", ui::paint("90", "→"), ui::paint("32", &c.to));
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
        println!("{}", ui::paint("90", "Not a terminal — re-run with -y to apply the changes above."));
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
