# ARCHITECTURE

How grove is put together. grove is **one binary over one library** — the
canonical house shape (thin CLI + `-core`). This document is embedded verbatim
into `grove --llm`, so it stays in sync with the shipped binary.

See also: `WORKFLOWS.md` for task-oriented usage, and `README.md` for the
landing page.

## Crate layout

```
grove/
  Cargo.toml                     # [workspace] + [workspace.package] + [profile.release]
  crates/
    grove/                       # the thin CLI (the `grove` binary)
      src/main.rs                # clap definition, --llm, and the per-verb handlers
      src/config.rs              # the grove file + `grove setup`/`init` (shell aliases)
      src/settings.rs            # the settings file + `grove configure` (cache, default_dir)
      src/cache.rs               # fetch-freshness stamps under ~/.cache/grove
      src/completions.rs         # shell completions + man, from the one clap definition
    grove-core/                  # all the multi-repo/git logic, as typed functions
      src/lib.rs                 # crate doc + module index + reset_sigpipe
      src/git.rs                 # discover repos, read state by shelling out to git
      src/overview.rs            # `overview` (lg): collect the dashboard + render
      src/sync.rs                # `sync` (lgs) + `pull-all` (lgp) + `push-all` (lgpp): act, then overview
      src/tree.rs                # `tree` (lt): build the tree + render
      src/remote.rs              # `ssh`: rewrite HTTPS remotes to SSH
      src/passthrough.rs         # the git verbs (status/add/commit/pull/push): exec git
      src/ui.rs                  # colour + hyperlink + progress-bar discipline
```

There is **one binary**, `grove`. It bundles four kinds of command:

- **Passthrough git verbs** — `status`, `add`, `commit`, `pull`, `push`. Each
  exec-replaces the process with `git` (on Unix) so colour, pager, signals, and
  the exit code are git's own. grove leaves no wrapper behind.
- **Multi-repo tools** — `overview`, `sync`, `pull-all`, `push-all`, over a
  folder of repos. `sync` is the everyday bidirectional one; `pull-all`/`push-all`
  are the single-direction escape hatches.
- **The tree view** — `tree`.
- **Shell-alias setup & settings** — `setup`, `init`, `example`, `configure`,
  plus `ssh` and hidden `completions`/`man`.

The short names you actually type — `gs`/`ga`/`gc`/`gcp`/`gp`/`gpp` for the git
verbs and `lg`/`lgs`/`lgp`/`lgpp`/`lt` for the multi-repo/tree tools — are **shell
aliases**, emitted by `grove setup` into the grove file, not separate binaries.
Nothing short lands on `PATH`, so nothing collides with other tools (notably
`lg`, which many people alias to lazygit); each alias is yours to rename. The
real subcommands (`grove overview`, `grove tree`, …) always work with no setup,
which is what scripts and agents should call.

## The CLI is thin; the logic lives in `grove-core`

Every verb in `main.rs` does exactly three things: **resolve inputs, call one
`grove-core` function, render the result** (a human view or, with `--json`, a
machine document). No verb holds domain logic — the test is that a second
frontend (a TUI, a library consumer) could reuse `grove-core` without
reimplementing anything.

### collect / render split

The data-producing tools separate *gathering* state from *rendering* it, so the
same core call backs both the human table and `--json`:

- `overview::collect(dir, Fetch) -> Report`, `overview::render_human(&Report, &Hints)`
- `sync::act_sync(&Report) -> Vec<Synced>`, `sync::render_human(&SyncReport, &Hints)`
- `sync::act_pull_all(&Report) -> Vec<String>`, `sync::render_pull(&PullReport, &Hints)`
- `sync::act_push_all(&Report) -> Vec<String>`, `sync::render_push(&PushReport, &Hints)`
- `tree::collect(dir, level, all) -> TreeReport`, `tree::render_human(&TreeReport)`

`collect` owns the fetch: its `Fetch` arg is `All`, `None`, or `Cache(closure)`
(fetch a repo only when the closure allows — the per-repo cache), and it runs the
fetch on a wide pool since fetching is network-bound, not CPU-bound. The `sync`
family acts purely off an already-collected `Report` — no network to *decide*, just
the pull/push transfers — so the binary does **collect (fetch) → act → collect
(`Fetch::None`, re-read post-action)**, and builds the `SyncReport`/`PullReport`/
`PushReport` (each embedding the post-run dashboard) itself. The `Report` types are
`#[derive(Serialize)]`; the CLI renders JSON with one `serde_json::to_string_pretty`.
The renderers take a `Hints` (built in the binary from the grove file) so the `→`
hints name the user's actual aliases.

### `--json`, and why it is per-verb, not global

`--json` is declared on the four data-producing verbs (`overview`, `sync`,
`push-all`, `tree`) — not as a global flag. Two reasons:

1. The passthrough git verbs can't emit grove JSON: they exec git, and git owns
   stdout.
2. A global flag combined with the git verbs' `trailing_var_arg` would let
   `grove commit fix the --json bug` swallow or misfire the flag. `--llm` is
   non-global for the same reason.

Output discipline (`ui.rs`): the command's **result** goes to stdout (the human
view, or the `--json` document); **progress** — the "Fetching"/"Syncing" bars —
goes to stderr, and auto-hides when stdout isn't a terminal. So `grove overview
--json | jq` stays pipe-clean while a human still sees progress.

### Settings, cache, and the default-dir fallback (binary-owned)

Anything that reaches into the environment (XDG paths, `$HOME`) lives in the
**binary**, so `grove-core` stays env-free and reusable:

- **`config.rs`** — the grove file (`~/.config/grove/aliases`) and `setup`/`init`.
  It also resolves the alias a user bound to a verb, which fills the `Hints` the
  renderers use.
- **`settings.rs`** — the settings file (`~/.config/grove/config`, same
  `key = value` shape) and `grove configure`: `cache`, `cache_ttl`, `default_dir`.
- **`cache.rs`** — per-repo fetch cache, **on by default**. One zero-byte stamp
  per repo under `~/.cache/grove` (mtime = last real fetch that left it settled).
  `collect`'s cache closure skips a repo's fetch when it was settled within
  `cache_ttl`; anything dirty/ahead/behind/diverged/https always re-fetches, so the
  actionable repos stay live and only the quiet rows can lag (marked `cached`).
  Bounded: `main.rs` re-stamps only repos it *fetched* — never on a skip — so a repo
  re-fetches at most `cache_ttl` after its last real fetch. `--force` bypasses it.
  This is a count-cutting complement to the wide fetch pool (skip most repos *and*
  fetch the rest fast), not a freshness trade — the repos you act on are never stale.
- **default-dir fallback** — when a multi-repo verb gets no folder and the current
  directory is unrelated to git (not inside a repo, no immediate sub-repo),
  `main.rs` substitutes `default_dir` and prints a dim note to stderr.

## `grove-core` module responsibilities

- **`git`** — the only module that shells out to `git`. Going through the real
  git binary (not a library) means the user's config, credentials, and SSH agent
  all apply. `discover` finds the immediate sub-repos; `is_https`/`web_url`/
  `ahead_behind`/`dirty`/`fetch`/`pull`/`push` read or act on one repo.
- **`overview`** — the dashboard: discover, fetch (per the `Fetch` policy) on a
  wide pool, classify each repo into the roll-up buckets, render the aligned colour
  table + hints. Each repo name is an OSC 8 `file://` link that opens the folder;
  the forge glyph links to its web page.
- **`sync`** — the actions, run off a collected `Report`: `act_sync` ff-pulls the
  strictly-behind and pushes the strictly-ahead clean repos; `act_pull_all` /
  `act_push_all` are the worktree-agnostic single-direction variants.
- **`tree`** — a dependency-free tree walk (dirs first, Nerd-Font icons, git repos
  flagged).
- **`remote`** — preview and (after confirmation) rewrite HTTPS remotes to SSH,
  dropping any embedded credentials.
- **`passthrough`** — exec git for the git verbs; `commit` is the one that
  spawns-and-waits because it may chain a push.
- **`ui`** — one colour helper (ANSI, gated on `NO_COLOR` + TTY, computed once),
  OSC 8 hyperlinks (only on terminals that support them), the shared progress
  bar, and the red-✗ error line. No colour crate.

## Note for the house guidelines

The house `rust-cli-guidelines` describe one app per repo and flag the
multi-command / multi-tool case as an open question (DECISIONS D12). grove
resolves it by **collapsing to a single binary with subcommands** rather than
shipping several binaries from one repo: one clap definition drives `--help`,
the man page, completions, and `--llm`; one `-core` library holds the logic; and
the many short command names are shell aliases (exactly the mechanism already
used for the git-verb shortcuts), not entries on `PATH`.
