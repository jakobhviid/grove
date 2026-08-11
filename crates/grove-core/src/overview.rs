//! `overview` (the `lg` alias): a one-screen dashboard of every repo directly
//! under a folder — branch, ahead/behind vs upstream, and staged/modified/
//! untracked counts. Repos are fetched in parallel first; https remotes are
//! flagged (not fetched) so you can switch them to SSH.
//!
//! Split in two so the CLI can render either surface without the logic knowing
//! which: [`collect`] gathers the state into a serializable [`Report`] (this is
//! the machine result behind `--json`), and [`render_human`] paints the table.
use crate::{git, ui};
use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// Nerd Font forge marks for the clickable link column. Known SaaS hosts get
// their brand glyph; self-hosted / Gitea / Forgejo / Codeberg / GitHub
// Enterprise / anything else falls back to a generic git logo (their domains
// are arbitrary, so the host can't identify them). Assumes a Nerd Font, exactly
// as `tree` does.
const ICON_GITHUB: &str = "\u{f09b}"; //  (octocat)
const ICON_GITLAB: &str = "\u{f296}"; //  (fox)
const ICON_BITBUCKET: &str = "\u{f171}"; //
const ICON_GIT: &str = "\u{e702}"; //  (generic git)
const ICON_LINK: &str = "\u{f0c1}"; //  (chain link — the column header)

// Trouble marks for the flag column. Deliberately *not* Nerd Font glyphs: unlike
// the decorative forge marks, a failure the reader can't see is worse than no
// column at all, so these come from ranges an ordinary monospace font covers
// (Mathematical Operators, Arrows, Misc Symbols, Dingbats) and each is one cell
// wide, so the column never misaligns. Emoji were the other candidate and are out
// for being two cells wide — 🔒 would shift every troubled row by one column.
const MARK_DENIED: &str = "⊘"; // U+2298
const MARK_UNREACHABLE: &str = "↯"; // U+21AF
const MARK_NEEDS_HAND: &str = "⚠"; // U+26A0
const MARK_FAILED: &str = "✗"; // U+2717 — pairs with the ✓ the Status column prints

/// A trouble's mark and color for callers outside this module (the action verbs
/// print failures as they happen, before the dashboard is rendered) — so the same
/// glyph and color come from one place and can't drift between the two surfaces.
pub fn mark_for(kind: git::Trouble) -> (&'static str, &'static str) {
    let (glyph, color, _) = mark(kind);
    (glyph, color)
}

/// A trouble's mark, its color, and the words the legend spells it out with.
///
/// Color follows *severity*, not identity, so the table reads at a glance: yellow
/// is "you can fix this", red is a hard failure and stays scarce so it keeps
/// meaning "read this now", and dim is environmental — an offline laptop flags the
/// whole fleet at once, which is one condition, not twenty problems.
fn mark(kind: git::Trouble) -> (&'static str, &'static str, &'static str) {
    match kind {
        git::Trouble::Denied => (MARK_DENIED, "33", "no access to origin"),
        git::Trouble::NeedsHand => (MARK_NEEDS_HAND, "33", "needs a hand"),
        git::Trouble::Failed => (MARK_FAILED, "31", "git failed"),
        git::Trouble::Unreachable => (MARK_UNREACHABLE, "90", "remote unreachable"),
    }
}

/// How a trouble reads on its own `→` line, ahead of git's own words. `Failed` has
/// no honest summary of its own — git's line *is* the explanation — so it adds none.
fn cause(kind: git::Trouble) -> &'static str {
    match kind {
        git::Trouble::Denied => "no access to origin",
        git::Trouble::Unreachable => "can't reach the remote",
        git::Trouble::NeedsHand => "needs a hand",
        git::Trouble::Failed => "",
    }
}

/// Severity order for the `→` lines: what you can fix first, environment last.
/// (The table itself stays in name order — this only ranks the detail lines.)
fn rank(kind: git::Trouble) -> u8 {
    match kind {
        git::Trouble::Denied => 0,
        git::Trouble::NeedsHand => 1,
        git::Trouble::Failed => 2,
        git::Trouble::Unreachable => 3,
    }
}

/// At most this many `→` trouble lines; the rest are counted, never dropped
/// silently. Enough to name a handful of repos without burying the roll-up.
const MAX_TROUBLE_LINES: usize = 6;

/// Pick the forge glyph for a repo's https web URL (`https://host/owner/repo`).
fn forge_icon(web_url: &str) -> &'static str {
    let host = web_url
        .strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("")
        .to_ascii_lowercase();
    if host == "github.com" || host.ends_with(".github.com") {
        ICON_GITHUB
    } else if host.contains("gitlab") {
        ICON_GITLAB
    } else if host.contains("bitbucket") {
        ICON_BITBUCKET
    } else {
        ICON_GIT
    }
}

/// One repo's state — the row of the dashboard, and one element of the `--json`
/// document. `ahead`/`behind` are both `null` when there is no upstream.
#[derive(Serialize)]
pub struct RepoStatus {
    pub name: String,
    /// Absolute path to the repo — the target of the clickable-name `file://`
    /// link, and handy for `--json` consumers that want to act on the repo.
    pub path: String,
    pub branch: String,
    /// origin is still on https (flagged, never fetched).
    pub https: bool,
    /// browser URL for origin, if it resolves to one.
    pub web_url: Option<String>,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub staged: u32,
    pub modified: u32,
    pub untracked: u32,
    /// Why git couldn't talk to this repo's remote — the mark in the flag column.
    /// A fetch that failed lands here, and a `sync`/`pull-all`/`push-all` that
    /// failed replaces it (the action is the more specific answer).
    pub trouble: Option<git::Trouble>,
    /// git's own one-line explanation for `trouble`, kept verbatim so `--json`
    /// consumers and the `→` lines quote git rather than paraphrase it.
    pub trouble_detail: Option<String>,
    /// The remote state below is the last one we managed to fetch — this run's
    /// fetch failed, so ahead/behind may be out of date. Renders the Status cell
    /// dim, so a mark next to `✓` reads "last known", not "up to date".
    pub stale: bool,
    /// This row's remote state was served from the per-repo cache (its fetch was
    /// skipped as recently-settled), not refreshed this run. Ephemeral display
    /// state, so it's kept out of the `--json` document.
    #[serde(skip)]
    pub cached: bool,
}

impl RepoStatus {
    pub fn dirty(&self) -> bool {
        self.staged > 0 || self.modified > 0 || self.untracked > 0
    }

    /// A fully-clean, in-sync ssh repo — nothing needs attention, so the human
    /// table leaves it un-bolded and the fetch cache may skip it next time. A repo
    /// in trouble is never calm, however clean it looks: that both bolds the row
    /// and keeps the fetch cache from stamping it settled, so the mark can't be
    /// hidden behind a cache hit on the next run.
    pub fn calm(&self) -> bool {
        !self.https && !self.dirty() && self.trouble.is_none() && matches!((self.ahead, self.behind), (Some(0), Some(0)))
    }
}

/// Roll-up counts under the table. Every repo lands in exactly one sync bucket
/// (`clean`/`ahead`/`behind`/`diverged`/`https`/`no_upstream`); `dirty` is an
/// independent overlay (a repo can be both dirty and ahead), and so are the four
/// trouble counts (a denied repo still counts in whichever sync bucket its
/// last-known state puts it).
#[derive(Serialize)]
pub struct Summary {
    pub repos: usize,
    pub clean: usize,
    pub dirty: usize,
    pub ahead: usize,
    pub behind: usize,
    pub diverged: usize,
    pub https: usize,
    pub no_upstream: usize,
    pub denied: usize,
    pub unreachable: usize,
    pub needs_hand: usize,
    pub failed: usize,
}

/// The whole dashboard: the folder, every repo under it, and the roll-up. This
/// is what `--json` serializes.
#[derive(Serialize)]
pub struct Report {
    pub dir: String,
    pub repos: Vec<RepoStatus>,
    pub summary: Summary,
}

/// How the next-step `→` hints should name the commands that clear pending work.
/// The binary fills each field with the alias the user actually bound in their
/// grove file (e.g. `lgpp` for `grove push-all`), or `None` when they haven't —
/// then the hint falls back to the long `grove …` form. `configured` is false
/// when there's no grove file at all, which turns on a one-line `grove setup`
/// nudge. Defaulting every field (`Hints::default()`) yields the long forms with
/// no nudge — the right behavior for `--json` callers, which never render hints.
#[derive(Default)]
pub struct Hints {
    /// Alias bound to `grove pull-all` (default `lgp`), if any.
    pub pull_all: Option<String>,
    /// Alias bound to `grove push-all` (default `lgpp`), if any.
    pub push_all: Option<String>,
    /// Alias bound to `grove ssh` (there is no default alias for it), if any.
    pub ssh: Option<String>,
    /// Whether a grove file exists (aliases are provisioned); gates the nudge.
    pub configured: bool,
}

/// The command token a hint should show: the user's short alias in backticks
/// when they have one, else the long `grove …` form.
fn token(alias: &Option<String>, long: &str) -> String {
    match alias {
        Some(a) => format!("`{a}`"),
        None => format!("`{long}`"),
    }
}

/// Whether — and per repo, which — remote-tracking refs to refresh before reading
/// state. Fetching is the slow part, so the caller controls it precisely.
pub enum Fetch<'a> {
    /// Fetch every ssh repo (a fresh look, or `--force`).
    All,
    /// Fetch nothing — reuse the refs already on disk (e.g. re-reading state right
    /// after a pull/push, where git already advanced the refs locally).
    None,
    /// Per-repo: fetch a repo only when the closure says so; repos it skips are
    /// flagged `cached` (served from a recent fetch within the cache window).
    Cache(&'a (dyn Fn(&Path) -> bool + Sync)),
}

impl Fetch<'_> {
    fn wants(&self, repo: &Path) -> bool {
        match self {
            Fetch::All => true,
            Fetch::None => false,
            Fetch::Cache(f) => f(repo),
        }
    }
    /// A skipped ssh repo is a "cache hit" only under `Cache`; under `None` it's a
    /// deliberate post-action re-read, not stale.
    fn tags_cached(&self) -> bool {
        matches!(self, Fetch::Cache(_))
    }
}

/// Run `work` on a thread pool sized for network I/O — more concurrency than
/// cores, since each `git fetch` is a blocking SSH round-trip, not CPU work.
/// Falls back to the default pool if a sized one can't be built.
fn run_wide<R: Send>(n: usize, work: impl FnOnce() -> R + Send) -> R {
    match rayon::ThreadPoolBuilder::new().num_threads(n.clamp(1, 32)).build() {
        Ok(pool) => pool.install(work),
        Err(_) => work(),
    }
}

/// Discover the repos directly under `dir` and read each one's state. `fetch`
/// decides which ssh repos get a fresh `git fetch` first (all, none, or per-repo
/// via the cache). Pure data: prints nothing but the shared "Fetching" progress
/// bar (stderr). Render with [`render_human`], or serialize the [`Report`] as JSON.
pub fn collect(dir: Option<&Path>, fetch: Fetch) -> Result<Report> {
    let dir = dir.unwrap_or_else(|| Path::new("."));
    if !dir.is_dir() {
        anyhow::bail!("not a directory: {}", dir.display());
    }
    let repos = git::discover(dir);

    // Classify remotes once, then decide per repo whether to fetch (https repos
    // are flagged, never fetched).
    let https: Vec<bool> = repos.par_iter().map(|r| git::is_https(&r.path)).collect();
    let to_fetch: Vec<bool> = repos.iter().zip(&https).map(|(r, &h)| !h && fetch.wants(&r.path)).collect();

    // Fetch phase — its own wide pool. Fetching is network-bound (one SSH
    // handshake per repo), so overlapping many at once beats a cpu-sized pool. A
    // fetch that fails is kept (by path) rather than dropped: it becomes the repo's
    // trouble mark in the table below.
    let fetch_repos: Vec<&git::Repo> = repos.iter().zip(&to_fetch).filter(|(_, &f)| f).map(|(r, _)| r).collect();
    let mut fetch_failed: HashMap<PathBuf, git::Fail> = HashMap::new();
    if !fetch_repos.is_empty() {
        let pb = ui::bar(fetch_repos.len() as u64, "Fetching");
        let fails: Vec<(PathBuf, git::Fail)> = run_wide(fetch_repos.len(), || {
            fetch_repos
                .par_iter()
                .filter_map(|r| {
                    let fail = git::fetch(&r.path);
                    pb.inc(1);
                    fail.map(|f| (r.path.clone(), f))
                })
                .collect()
        });
        pb.finish_and_clear();
        fetch_failed.extend(fails);
    }

    // State phase — cheap local git reads for every repo.
    let cached_tag = fetch.tags_cached();
    let repos: Vec<RepoStatus> = repos
        .par_iter()
        .enumerate()
        .map(|(i, r)| {
            let branch = git::branch(&r.path);
            let web_url = git::web_url(&r.path);
            // Absolute path for the clickable-name file:// link; fall back to the
            // discovered path if canonicalization fails (e.g. a race removing it).
            let path = std::fs::canonicalize(&r.path).unwrap_or_else(|_| r.path.clone()).display().to_string();
            if https[i] {
                return RepoStatus {
                    name: r.name.clone(),
                    path,
                    branch,
                    https: true,
                    web_url,
                    ahead: None,
                    behind: None,
                    staged: 0,
                    modified: 0,
                    untracked: 0,
                    trouble: None,
                    trouble_detail: None,
                    stale: false,
                    cached: false,
                };
            }
            let ab = git::ahead_behind(&r.path);
            let dirty = git::dirty(&r.path);
            let fail = fetch_failed.get(&r.path);
            RepoStatus {
                name: r.name.clone(),
                path,
                branch,
                https: false,
                web_url,
                ahead: ab.map(|(ahead, _)| ahead),
                behind: ab.map(|(_, behind)| behind),
                staged: dirty.staged,
                modified: dirty.modified,
                untracked: dirty.untracked,
                trouble: fail.map(|f| f.kind),
                trouble_detail: fail.map(|f| f.detail.clone()),
                // The refs are whatever the last successful fetch left behind.
                stale: fail.is_some(),
                // An ssh repo we skipped under Cache mode is served from cache.
                cached: cached_tag && !to_fetch[i],
            }
        })
        .collect();

    let summary = summarize(&repos);
    Ok(Report { dir: dir.display().to_string(), repos, summary })
}

/// Tally every repo into the roll-up buckets. Kept in core (it's classification
/// logic, not rendering) so `--json` and the human roll-up agree by construction.
fn summarize(repos: &[RepoStatus]) -> Summary {
    let mut summary = Summary {
        repos: repos.len(),
        clean: 0,
        dirty: 0,
        ahead: 0,
        behind: 0,
        diverged: 0,
        https: 0,
        no_upstream: 0,
        denied: 0,
        unreachable: 0,
        needs_hand: 0,
        failed: 0,
    };
    for repo in repos {
        match repo.trouble {
            Some(git::Trouble::Denied) => summary.denied += 1,
            Some(git::Trouble::Unreachable) => summary.unreachable += 1,
            Some(git::Trouble::NeedsHand) => summary.needs_hand += 1,
            Some(git::Trouble::Failed) => summary.failed += 1,
            None => {}
        }
        if repo.https {
            summary.https += 1;
            continue;
        }
        if repo.dirty() {
            summary.dirty += 1;
        }
        match (repo.ahead, repo.behind) {
            (Some(ahead), Some(behind)) if ahead > 0 && behind > 0 => summary.diverged += 1,
            (Some(ahead), _) if ahead > 0 => summary.ahead += 1,
            (_, Some(behind)) if behind > 0 => summary.behind += 1,
            (Some(_), Some(_)) => {
                if !repo.dirty() {
                    summary.clean += 1;
                }
            }
            _ => summary.no_upstream += 1,
        }
    }
    summary
}

/// Recompute the roll-up after rows were changed in place — the post-action
/// dashboard stamps trouble onto rows that [`collect`] read before the pull/push
/// verdict was in, and the counts have to follow.
pub fn resummarize(report: &mut Report) {
    report.summary = summarize(&report.repos);
}

/// Carry what an earlier collection learned about the remotes onto a freshly-read
/// report. The post-action dashboard re-reads state with [`Fetch::None`], so it has
/// no fetch of its own to learn from; without this, a denied repo would show a mark
/// during the fetch and lose it one line later. Rows that already carry trouble
/// (the action just failed on them) keep theirs — that verdict is newer.
pub fn carry_trouble(report: &mut Report, earlier: &Report) {
    let before: HashMap<&str, (git::Trouble, Option<&String>, bool)> = earlier
        .repos
        .iter()
        .filter_map(|r| r.trouble.map(|t| (r.path.as_str(), (t, r.trouble_detail.as_ref(), r.stale))))
        .collect();
    for repo in &mut report.repos {
        if repo.trouble.is_some() {
            continue;
        }
        if let Some((kind, detail, stale)) = before.get(repo.path.as_str()) {
            repo.trouble = Some(*kind);
            repo.trouble_detail = detail.map(|d| d.to_string());
            repo.stale = *stale;
        }
    }
    resummarize(report);
}

/// Paint the dashboard for a human: the aligned, colored table plus the roll-up
/// and next-step hints. `hints` decides whether those hints name the user's short
/// aliases or the long `grove …` forms. `--json` callers skip this and serialize
/// the [`Report`].
pub fn render_human(report: &Report, hints: &Hints) {
    if report.repos.is_empty() {
        println!("No git repositories in {}", report.dir);
        return;
    }
    let rows = &report.repos;

    // Size the Repository and Branch columns to their widest entry (never below
    // the header) so a long name like `opencode-dynamic-custom-providers` can't
    // shove the rest of the row out of alignment. Count chars, not bytes, so a
    // Danish æ/ø/å in a name lines up the same as an ASCII one.
    let width = |header: &str, field: &dyn Fn(&RepoStatus) -> usize| {
        rows.iter().map(field).max().unwrap_or(0).max(header.chars().count())
    };
    let name_w = width("Repository", &|r| r.name.chars().count());
    let branch_w = width("Branch", &|r| r.branch.chars().count());

    // Two spaces between every column — a single space read as cramped once the
    // wide URL became a lone glyph.
    let gap = "  ";

    // The forge-link column sits right after the repo name and only appears on
    // terminals that render OSC 8 hyperlinks — otherwise a lone Nerd-Font glyph
    // would be unclickable decoration, so we drop the column entirely rather than
    // show a dead icon. The glyph assumes a Nerd Font, like `tree` (see the brew
    // caveat). When on, the header labels it with a chain-link glyph; each row's
    // cell is a 1-wide forge glyph (or a blank when the repo has no origin), so
    // it always lines up under that header. When off, the whole column vanishes.
    // The flag column leads the row — a one-wide trouble mark hugging the repo name,
    // where the eye lands first. It exists only when something is wrong, so
    // a healthy fleet renders exactly as it always has and the column appearing is
    // itself the signal. One space (not the usual two) between mark and name, so the
    // mark reads as belonging to that repo rather than floating in its own column.
    let flagged = rows.iter().any(|r| r.trouble.is_some());
    let flag_seg = |cell: &str| if flagged { format!("{cell} ") } else { String::new() };
    let row_flag = |r: &RepoStatus| match r.trouble {
        Some(kind) => {
            let (glyph, color, _) = mark(kind);
            flag_seg(&ui::paint(color, glyph))
        }
        // Healthy row in a table that has a flag column: a blank keeps it aligned.
        None => flag_seg(" "),
    };

    let links = ui::hyperlinks();
    let link_seg = |cell: String| if links { format!("{gap}{cell}") } else { String::new() };
    let row_link = |r: &RepoStatus| match &r.web_url {
        // The glyph is the click target; clicking opens the repo's web page.
        Some(url) => link_seg(ui::link(url, &ui::paint("36", forge_icon(url)))),
        // No origin: a blank keeps the Branch column aligned under the header.
        None => link_seg(" ".to_string()),
    };

    println!();
    println!(
        "  {}{}",
        flag_seg(" "),
        ui::paint("1", &format!("{:<name_w$}{}{gap}{:<branch_w$}{gap}Status", "Repository", link_seg(ICON_LINK.to_string()), "Branch"))
    );
    println!(
        "  {}{}",
        flag_seg(" "),
        ui::paint("90", &format!("{}{}{gap}{}{gap}──────", "─".repeat(name_w), link_seg("─".to_string()), "─".repeat(branch_w)))
    );

    for r in rows {
        // A fully-clean, in-sync ssh repo needs no attention. Rather than dim the
        // clean rows (which vanish on a dark terminal), keep them normal and make
        // the rows that DO need attention **bold**, so the eye lands on them. The
        // name is also a file:// link that opens the repo folder (the counterpart
        // to the forge glyph, which opens its web page) — on terminals that render
        // OSC 8; elsewhere it's plain text.
        let name = {
            let padded = format!("{:<name_w$}", r.name);
            let painted = if r.calm() { padded } else { ui::paint("1", &padded) };
            if links && !r.path.is_empty() { ui::open(&r.path, &painted) } else { painted }
        };
        let flag = row_flag(r);
        let link = row_link(r);
        let branch = ui::paint("34", &format!("{:<branch_w$}", r.branch));

        if r.https {
            println!("  {flag}{name}{link}{gap}{branch}{gap}{}", ui::paint("31", "HTTPS — switch to SSH"));
            continue;
        }

        let (sync, color) = match (r.ahead, r.behind) {
            (Some(ahead), Some(behind)) if ahead > 0 && behind > 0 => (format!("↑{ahead} ↓{behind}"), "33"),
            (Some(ahead), _) if ahead > 0 => (format!("↑{ahead}"), "33"),
            (_, Some(behind)) if behind > 0 => (format!("↓{behind}"), "31"),
            (Some(_), Some(_)) => ("✓".to_string(), "32"),
            _ => ("—".to_string(), "37"),
        };
        // A failed fetch leaves these counts at whatever the last good fetch saw, so
        // dim them: a mark beside a dim ✓ reads "last known state", never "in sync".
        let color = if r.stale { "90" } else { color };

        let mut line = format!("  {flag}{name}{link}{gap}{branch}{gap}{}", ui::paint(color, &sync));
        if r.staged > 0 {
            line += &format!(" {}", ui::paint("32", &format!("+{}", r.staged)));
        }
        if r.modified > 0 {
            line += &format!(" {}", ui::paint("33", &format!("!{}", r.modified)));
        }
        if r.untracked > 0 {
            line += &format!(" {}", ui::paint("34", &format!("?{}", r.untracked)));
        }
        println!("{line}");
    }
    render_legend(report);
    render_summary(report, hints);
    println!();
}

/// Spell out the marks the table just used — and only those, in the order the
/// column ranks them. A mark carries the meaning; the legend is what makes it
/// readable without knowing grove, and what keeps the design honest on a terminal
/// whose font renders a glyph you don't recognize.
fn render_legend(report: &Report) {
    let mut kinds: Vec<git::Trouble> = report.repos.iter().filter_map(|r| r.trouble).collect();
    kinds.sort_by_key(|k| rank(*k));
    kinds.dedup();
    if kinds.is_empty() {
        return;
    }
    let entries: Vec<String> = kinds
        .iter()
        .map(|k| {
            let (glyph, color, words) = mark(*k);
            format!("{} {}", ui::paint(color, glyph), ui::paint("90", words))
        })
        .collect();
    println!("\n  {} {}", ui::paint("90", "legend:"), entries.join(&ui::paint("90", " · ")));
}

/// A one-line roll-up under the table — counts toned by severity — plus the exact
/// command to clear each kind of pending work. This is the at-a-glance triage.
fn render_summary(report: &Report, hints: &Hints) {
    let summary = &report.summary;
    let https_names: Vec<&str> = report.repos.iter().filter(|r| r.https).map(|r| r.name.as_str()).collect();

    let sep = ui::paint("90", " · ");
    let mut parts = vec![ui::paint("1", &format!("{} repos", summary.repos))];
    let mut add = |count: usize, label: &str, color: &str| {
        if count > 0 {
            parts.push(ui::paint(color, &format!("{count} {label}")));
        }
    };
    add(summary.clean, "clean", "32");
    add(summary.dirty, "dirty", "33");
    add(summary.ahead, "to push", "33");
    add(summary.behind, "to pull", "31");
    add(summary.diverged, "diverged", "31");
    add(summary.https, "https", "31");
    add(summary.no_upstream, "no upstream", "90");
    // Rows served from the fetch cache (not refreshed this run) — call them out so
    // an unchanged table reads as "reused a recent fetch", never as frozen.
    let cached = report.repos.iter().filter(|r| r.cached).count();
    add(cached, "cached", "90");
    // Trouble counts carry their own mark, so the roll-up and the flag column can't
    // drift apart — each count reads as the tally of exactly that glyph. They sit at
    // the end, right above the `→` lines that name the repos.
    for (count, kind, label) in [
        (summary.denied, git::Trouble::Denied, "denied"),
        (summary.needs_hand, git::Trouble::NeedsHand, "need a hand"),
        (summary.failed, git::Trouble::Failed, "failed"),
        (summary.unreachable, git::Trouble::Unreachable, "unreachable"),
    ] {
        if count > 0 {
            let (glyph, color, _) = mark(kind);
            parts.push(ui::paint(color, &format!("{glyph} {count} {label}")));
        }
    }
    println!("\n  {}", parts.join(&sep));

    // Each hint names the command that clears that kind of work, in the caller's
    // preferred form: the user's short alias when they have one (`lgpp`), else the
    // long `grove …` verb. The direction-specific verbs pull-all/push-all mirror
    // the behind/ahead counts exactly; `ssh` has no default alias, so it stays long.
    // Trouble first, and each line names the repo *and* quotes git's own words. This
    // is the part that answers "why didn't it pull?" — the mark says a repo is stuck,
    // this says what it is stuck on, in git's language, so the next step is obvious.
    let mut troubled: Vec<&RepoStatus> = report.repos.iter().filter(|r| r.trouble.is_some()).collect();
    troubled.sort_by_key(|r| rank(r.trouble.unwrap_or(git::Trouble::Failed)));
    for repo in troubled.iter().take(MAX_TROUBLE_LINES) {
        let kind = repo.trouble.unwrap_or(git::Trouble::Failed);
        let (glyph, color, _) = mark(kind);
        let detail = repo.trouble_detail.as_deref().unwrap_or("no details from git");
        let said = match cause(kind) {
            "" => detail.to_string(),
            cause => format!("{cause} ({detail})"),
        };
        println!("  {} {} {}", ui::paint(color, glyph), ui::paint("1", &repo.name), ui::paint("90", &format!("— {said}")));
    }
    if troubled.len() > MAX_TROUBLE_LINES {
        let rest = troubled.len() - MAX_TROUBLE_LINES;
        println!("  {}", ui::paint("90", &format!("  … and {rest} more — `--json` lists every one")));
    }

    let mut lines: Vec<String> = Vec::new();
    if summary.ahead > 0 {
        lines.push(format!("{} pushes {} with unpushed commits", token(&hints.push_all, "grove push-all"), summary.ahead));
    }
    if summary.behind > 0 {
        lines.push(format!("{} fast-forward-pulls {} behind {}", token(&hints.pull_all, "grove pull-all"), summary.behind, if summary.behind == 1 { "repo" } else { "repos" }));
    }
    if !https_names.is_empty() {
        lines.push(format!("{} switches {} to SSH: {}", token(&hints.ssh, "grove ssh"), https_names.len(), https_names.join(", ")));
    }
    if summary.diverged > 0 {
        lines.push(format!("{} pulls {} diverged too (git pull per your config; conflicts still need a hand)", token(&hints.pull_all, "grove pull-all"), summary.diverged));
    }
    for line in lines {
        println!("  {} {}", ui::paint("36", "→"), line);
    }
    if cached > 0 {
        println!("  {} {}", ui::paint("90", "→"), ui::paint("90", &format!("{cached} served from a recent fetch — `--force` to refetch")));
    }
    // When aliases aren't provisioned yet, the hints above showed the long forms —
    // point out that `grove setup` installs the short ones. Dropped once set up.
    if !hints.configured {
        println!("  {} {}", ui::paint("90", "→"), ui::paint("90", "tip: `grove setup` installs the short aliases (lg lgs lgp lgpp lt)"));
    }
}

#[cfg(test)]
mod tests {
    use super::token;

    #[test]
    fn token_prefers_the_bound_alias_else_the_long_form() {
        assert_eq!(token(&Some("lgpp".to_string()), "grove push-all"), "`lgpp`");
        assert_eq!(token(&None, "grove push-all"), "`grove push-all`");
    }
}
