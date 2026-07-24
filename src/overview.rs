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
    let dir = dir.unwrap_or_else(|| Path::new("."));
    if !dir.is_dir() {
        anyhow::bail!("not a directory: {}", dir.display());
    }
    let repos = git::discover(dir);
    if repos.is_empty() {
        println!("No git repositories in {}", dir.display());
        return Ok(());
    }

    // Gather every repo's state in parallel (fetch happens inside, ssh only).
    let rows: Vec<Row> = repos
        .par_iter()
        .map(|r| {
            let https = git::is_https(&r.path);
            let branch = git::branch(&r.path);
            if https {
                return Row { name: r.name.clone(), branch, https, ab: None, dirty: git::Dirty::default() };
            }
            git::fetch(&r.path);
            Row {
                name: r.name.clone(),
                branch,
                https,
                ab: git::ahead_behind(&r.path),
                dirty: git::dirty(&r.path),
            }
        })
        .collect();

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
        let name = format!("{:<25}", r.name);
        let branch = ui::paint("34", &format!("{:<14}", r.branch));

        if r.https {
            println!("  {name} {branch} {}", ui::paint("31", "HTTPS — switch to SSH"));
            continue;
        }

        let (sync, color) = match r.ab {
            Some((a, b)) if a > 0 && b > 0 => (format!("↑{a} ↓{b}"), "33"),
            Some((a, _)) if a > 0 => (format!("↑{a}"), "33"),
            Some((_, b)) if b > 0 => (format!("↓{b}"), "31"),
            Some(_) => ("✓".to_string(), "32"),
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
    println!();
}
