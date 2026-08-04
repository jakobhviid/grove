//! `tree` (the `lt` alias): a self-contained tree view — no eza, no external
//! deps. Directories first then files (both sorted), Nerd-Font icons, dotfiles
//! hidden, `--level` deep (default 2). Directories that are git repositories get
//! a git icon next to their name, so a workspace folder shows at a glance which
//! children are repos.
//!
//! [`collect`] builds the nested [`TreeReport`] (the `--json` document);
//! [`render_human`] draws the connector-and-icon tree.
use crate::ui;
use anyhow::{bail, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const ICON_DIR: &str = "\u{f07b}"; //
const ICON_GIT: &str = "\u{e725}"; //  (git branch)
const ICON_FILE: &str = "\u{f15b}"; //

/// One node of the tree. Files carry no `is_repo`/`children`; a directory at the
/// depth limit has empty `children` (both are omitted from JSON when empty).
#[derive(Serialize)]
pub struct Entry {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: &'static str, // "dir" | "file"
    #[serde(skip_serializing_if = "is_false")]
    pub is_repo: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Entry>,
}

fn is_false(value: &bool) -> bool {
    !value
}

/// The tree rooted at `root`.
#[derive(Serialize)]
pub struct TreeReport {
    pub root: String,
    pub entries: Vec<Entry>,
}

pub fn collect(dir: Option<&Path>, level: usize, all: bool) -> Result<TreeReport> {
    let root = dir.unwrap_or_else(|| Path::new("."));
    if !root.is_dir() {
        bail!("not a directory: {}", root.display());
    }
    let entries = build(root, 1, level.max(1), all);
    Ok(TreeReport { root: root.display().to_string(), entries })
}

/// Immediate children as [`Entry`]s, recursing into directories until `depth`
/// reaches `max`. Directories first then files (each by name); dotfiles hidden
/// unless `all`.
fn build(dir: &Path, depth: usize, max: usize, all: bool) -> Vec<Entry> {
    children(dir, all)
        .into_iter()
        .map(|(path, is_dir)| {
            let is_repo = is_dir && path.join(".git").exists();
            let children = if is_dir && depth < max { build(&path, depth + 1, max, all) } else { Vec::new() };
            Entry { name: name(&path), kind: if is_dir { "dir" } else { "file" }, is_repo, children }
        })
        .collect()
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

pub fn render_human(report: &TreeReport) {
    // Absolute base so each folder's file:// link resolves regardless of where the
    // tree was rooted (`.`, a relative path, …); fall back to the printed root.
    let base = fs::canonicalize(&report.root).unwrap_or_else(|_| PathBuf::from(&report.root));
    let root_label = ui::paint("1;34", &report.root);
    if ui::hyperlinks() {
        println!("{}", ui::open(&base.display().to_string(), &root_label));
    } else {
        println!("{root_label}");
    }
    render_entries(&report.entries, "", &base);
}

fn render_entries(entries: &[Entry], prefix: &str, base: &Path) {
    let links = ui::hyperlinks();
    let n = entries.len();
    for (i, entry) in entries.iter().enumerate() {
        let last = i + 1 == n;
        let connector = if last { "└── " } else { "├── " };
        let path = base.join(&entry.name);

        let label = if entry.kind == "dir" {
            // Folder names are clickable — a file:// link opens the directory in
            // the OS file manager (only where OSC 8 renders; plain text elsewhere).
            let painted = ui::paint("1;34", &entry.name);
            let name = if links { ui::open(&path.display().to_string(), &painted) } else { painted };
            let mut s = format!("{} {}", ui::paint("34", ICON_DIR), name);
            if entry.is_repo {
                s.push(' ');
                s.push_str(&ui::paint("38;5;208", ICON_GIT)); // git orange
            }
            s
        } else {
            format!("{} {}", ui::paint("90", ICON_FILE), entry.name)
        };

        println!("{prefix}{connector}{label}");

        if entry.kind == "dir" && !entry.children.is_empty() {
            let child_prefix = format!("{prefix}{}", if last { "    " } else { "│   " });
            render_entries(&entry.children, &child_prefix, &path);
        }
    }
}
