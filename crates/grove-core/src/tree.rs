//! `tree` (alias lt): a self-contained tree view — no eza, no external deps.
//! Directories first then files (both sorted), Nerd-Font icons, dotfiles hidden,
//! `--level` deep (default 2). Directories that are git repositories get a git
//! icon next to their name, so a workspace folder shows at a glance which
//! children are repos.
use crate::ui;
use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};

const ICON_DIR: &str = "\u{f07b}"; //
const ICON_GIT: &str = "\u{e725}"; //  (git branch)
const ICON_FILE: &str = "\u{f15b}"; //

pub fn run(dir: Option<&Path>, level: usize, all: bool) -> Result<()> {
    let root = dir.unwrap_or_else(|| Path::new("."));
    if !root.is_dir() {
        bail!("not a directory: {}", root.display());
    }
    println!("{}", ui::paint("1;34", &root.display().to_string()));
    walk(root, 1, level.max(1), all, "");
    Ok(())
}

/// Immediate children: directories first then files (each by name). Dotfiles are
/// hidden unless `all` is set.
fn children(dir: &Path, all: bool) -> Vec<(PathBuf, bool)> {
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<(PathBuf, bool)> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            all || !p
                .file_name()
                .map(|n| n.to_string_lossy().starts_with('.'))
                .unwrap_or(true)
        })
        .map(|p| {
            let is_dir = p.is_dir();
            (p, is_dir)
        })
        .collect();
    v.sort_by(|(pa, da), (pb, db)| db.cmp(da).then_with(|| name(pa).cmp(&name(pb))));
    v
}

fn name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn walk(dir: &Path, depth: usize, max: usize, all: bool, prefix: &str) {
    let items = children(dir, all);
    let n = items.len();
    for (i, (path, is_dir)) in items.iter().enumerate() {
        let last = i + 1 == n;
        let connector = if last { "└── " } else { "├── " };

        let label = if *is_dir {
            let mut s = format!("{} {}", ui::paint("34", ICON_DIR), ui::paint("1;34", &name(path)));
            if path.join(".git").exists() {
                s.push(' ');
                s.push_str(&ui::paint("38;5;208", ICON_GIT)); // git orange
            }
            s
        } else {
            format!("{} {}", ui::paint("90", ICON_FILE), name(path))
        };

        println!("{prefix}{connector}{label}");

        if *is_dir && depth < max {
            let child_prefix = format!("{prefix}{}", if last { "    " } else { "│   " });
            walk(path, depth + 1, max, all, &child_prefix);
        }
    }
}
