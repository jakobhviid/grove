//! `tree` (the `lt` alias): a self-contained tree view — no eza, no external
//! deps. Directories first then files (both sorted), Nerd-Font icons by file
//! type, dotfiles hidden, `--level` deep (default 2). Directories that are git
//! repositories get a git icon next to their name, so a workspace folder shows at
//! a glance which children are repos. `--git-ignore` leaves out what git ignores.
//!
//! [`collect`] builds the nested [`TreeReport`] (the `--json` document);
//! [`render_human`] draws the connector-and-icon tree.
use crate::{git, ui};
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

/// What a tree shows.
pub struct Options {
    /// How many levels deep to descend (at least 1).
    pub level: usize,
    /// Show dotfiles too.
    pub all: bool,
    /// Leave out what git ignores, by git's own rules — see [`git::ignored`].
    /// Only folders inside a work tree have anything to leave out; a folder of
    /// repos is listed whole, and each repo in it is filtered by its own rules.
    pub git_ignore: bool,
}

pub fn collect(dir: Option<&Path>, opts: &Options) -> Result<TreeReport> {
    let root = dir.unwrap_or_else(|| Path::new("."));
    if !root.is_dir() {
        bail!("not a directory: {}", root.display());
    }
    let opts = Options { level: opts.level.max(1), ..*opts };
    let in_repo = opts.git_ignore && git::in_worktree(root);
    let entries = build(root, 1, &opts, in_repo);
    Ok(TreeReport { root: root.display().to_string(), entries })
}

/// Immediate children as [`Entry`]s, recursing into directories until `depth`
/// reaches the level. Directories first then files (each by name); dotfiles hidden
/// unless `all`. `in_repo` says whether `dir` sits in a work tree, so git is only
/// asked about folders it has rules for — and a repo met on the way down starts
/// its own.
fn build(dir: &Path, depth: usize, opts: &Options, in_repo: bool) -> Vec<Entry> {
    let mut kids = children(dir, opts.all);
    if opts.git_ignore && in_repo {
        let names: Vec<String> = kids.iter().map(|(p, _)| name(p)).collect();
        let ignored = git::ignored(dir, &names);
        kids.retain(|(p, _)| !ignored.contains(&name(p)));
    }
    kids.into_iter()
        .map(|(path, is_dir)| {
            let is_repo = is_dir && path.join(".git").exists();
            let children = if is_dir && depth < opts.level { build(&path, depth + 1, opts, in_repo || is_repo) } else { Vec::new() };
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

// File-type icons: a Nerd Font glyph and the colour to paint it, for the files a
// source tree is mostly made of. Exact names win over extensions (`Cargo.lock` is
// a lockfile before it is anything else); both match case-insensitively, and an
// unknown file keeps the plain file glyph. Every glyph comes from the long-stable
// Font Awesome, Devicons, Seti, Octicons and Font Logos sets, so it renders on
// Nerd Font 2 and 3 alike.
const RUST: (&str, &str) = ("\u{e7a8}", "38;5;173");
const PYTHON: (&str, &str) = ("\u{e606}", "33");
const JS: (&str, &str) = ("\u{e60c}", "33");
const TS: (&str, &str) = ("\u{e628}", "34");
const REACT: (&str, &str) = ("\u{e7ba}", "36");
const NODE: (&str, &str) = ("\u{e718}", "32");
const GO: (&str, &str) = ("\u{e627}", "36");
const JSON: (&str, &str) = ("\u{e60b}", "33");
const CONFIG: (&str, &str) = ("\u{e615}", "90");
const MARKDOWN: (&str, &str) = ("\u{e609}", "37");
const SHELL: (&str, &str) = ("\u{f489}", "32");
const C: (&str, &str) = ("\u{e61e}", "34");
const CPP: (&str, &str) = ("\u{e61d}", "34");
const JAVA: (&str, &str) = ("\u{e738}", "31");
const KOTLIN: (&str, &str) = ("\u{e634}", "35");
const SWIFT: (&str, &str) = ("\u{e755}", "38;5;208");
const RUBY: (&str, &str) = ("\u{e739}", "31");
const PHP: (&str, &str) = ("\u{e73d}", "35");
const LUA: (&str, &str) = ("\u{e620}", "34");
const VIM: (&str, &str) = ("\u{e62b}", "32");
const NIX: (&str, &str) = ("\u{f313}", "34");
const HASKELL: (&str, &str) = ("\u{e777}", "35");
const HTML: (&str, &str) = ("\u{f13b}", "38;5;208");
const CSS: (&str, &str) = ("\u{e749}", "34");
const SASS: (&str, &str) = ("\u{e603}", "35");
const DOCKER: (&str, &str) = ("\u{f308}", "34");
const MAKE: (&str, &str) = ("\u{e779}", "90");
const GIT: (&str, &str) = ("\u{e702}", "38;5;208");
const DATABASE: (&str, &str) = ("\u{f1c0}", "33");
const SHEET: (&str, &str) = ("\u{f1c3}", "32");
const IMAGE: (&str, &str) = ("\u{f1c5}", "35");
const AUDIO: (&str, &str) = ("\u{f1c7}", "36");
const VIDEO: (&str, &str) = ("\u{f1c8}", "36");
const ARCHIVE: (&str, &str) = ("\u{f1c6}", "31");
const PDF: (&str, &str) = ("\u{f1c1}", "31");
const LOCK: (&str, &str) = ("\u{f023}", "90");
const KEY: (&str, &str) = ("\u{f084}", "33");
const TEXT: (&str, &str) = ("\u{f0f6}", "90");
const BOOK: (&str, &str) = ("\u{f02d}", "37");

const BY_NAME: &[(&[&str], (&str, &str))] = &[
    (&["cargo.toml"], RUST),
    (&["package.json"], NODE),
    (&["cargo.lock", "package-lock.json", "yarn.lock", "pnpm-lock.yaml", "flake.lock", "gemfile.lock", "poetry.lock", "uv.lock"], LOCK),
    (&["dockerfile", "containerfile", "docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"], DOCKER),
    (&["makefile", "gnumakefile", "justfile"], MAKE),
    (&[".gitignore", ".gitattributes", ".gitmodules", ".gitconfig", ".mailmap"], GIT),
    (&["license", "license.md", "license.txt", "copying", "unlicense"], BOOK),
    (&[".env", ".envrc", ".editorconfig"], CONFIG),
];

const BY_EXT: &[(&[&str], (&str, &str))] = &[
    (&["rs"], RUST),
    (&["py", "pyi"], PYTHON),
    (&["js", "mjs", "cjs"], JS),
    (&["ts", "mts", "cts"], TS),
    (&["jsx", "tsx"], REACT),
    (&["go"], GO),
    (&["json", "jsonc", "json5"], JSON),
    (&["toml", "yaml", "yml", "ini", "cfg", "conf"], CONFIG),
    (&["md", "markdown", "mdx"], MARKDOWN),
    (&["sh", "bash", "zsh", "fish"], SHELL),
    (&["c", "h"], C),
    (&["cpp", "cc", "cxx", "hpp", "hh", "hxx"], CPP),
    (&["java", "jar"], JAVA),
    (&["kt", "kts"], KOTLIN),
    (&["swift"], SWIFT),
    (&["rb"], RUBY),
    (&["php"], PHP),
    (&["lua"], LUA),
    (&["vim"], VIM),
    (&["nix"], NIX),
    (&["hs"], HASKELL),
    (&["html", "htm"], HTML),
    (&["css"], CSS),
    (&["scss", "sass"], SASS),
    (&["sql", "db", "sqlite", "sqlite3"], DATABASE),
    (&["csv", "tsv"], SHEET),
    (&["png", "jpg", "jpeg", "gif", "svg", "webp", "ico", "bmp", "tif", "tiff", "avif", "heic"], IMAGE),
    (&["mp3", "wav", "flac", "ogg", "m4a"], AUDIO),
    (&["mp4", "mkv", "mov", "avi", "webm"], VIDEO),
    (&["zip", "tar", "gz", "tgz", "xz", "bz2", "zst", "7z", "rar"], ARCHIVE),
    (&["pdf"], PDF),
    (&["lock"], LOCK),
    (&["pem", "key", "crt", "cer", "pub", "asc", "gpg"], KEY),
    (&["txt", "log"], TEXT),
];

/// The icon and colour for a file called `name` — see the tables above.
fn file_icon(name: &str) -> (&'static str, &'static str) {
    let lower = name.to_ascii_lowercase();
    let find = |table: &[(&[&str], (&'static str, &'static str))], key: &str| table.iter().find(|(keys, _)| keys.contains(&key)).map(|(_, icon)| *icon);
    if let Some(icon) = find(BY_NAME, &lower) {
        return icon;
    }
    lower
        .rsplit_once('.')
        .filter(|(stem, _)| !stem.is_empty())
        .and_then(|(_, ext)| find(BY_EXT, ext))
        .unwrap_or((ICON_FILE, "90"))
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
            let (icon, color) = file_icon(&entry.name);
            format!("{} {}", ui::paint(color, icon), entry.name)
        };

        println!("{prefix}{connector}{label}");

        if entry.kind == "dir" && !entry.children.is_empty() {
            let child_prefix = format!("{prefix}{}", if last { "    " } else { "│   " });
            render_entries(&entry.children, &child_prefix, &path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{file_icon, ICON_FILE, LOCK, MARKDOWN, RUST, SHELL};

    #[test]
    fn a_file_is_known_by_its_name_before_its_extension() {
        assert_eq!(file_icon("main.rs"), RUST);
        assert_eq!(file_icon("Cargo.toml"), RUST);
        assert_eq!(file_icon("Cargo.lock"), LOCK, "a lockfile is a lockfile before it is a crate file");
        assert_eq!(file_icon("README.MD"), MARKDOWN, "case doesn't matter");
        assert_eq!(file_icon("install.sh"), SHELL);
    }

    #[test]
    fn an_unknown_file_and_a_bare_dotfile_get_the_plain_glyph() {
        assert_eq!(file_icon("notes").0, ICON_FILE);
        assert_eq!(file_icon("archive.weird").0, ICON_FILE);
        // `.md` is a dotfile called "md", not a markdown file with no name.
        assert_eq!(file_icon(".md").0, ICON_FILE);
    }
}
