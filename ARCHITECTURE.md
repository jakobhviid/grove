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
      src/completions.rs         # shell completions + man, from the one clap definition
    grove-core/                  # all the multi-repo/git logic, as typed functions
      src/lib.rs                 # crate doc + module index + reset_sigpipe
      src/git.rs                 # discover repos, read state by shelling out to git
      src/overview.rs            # `overview` (lg): collect the dashboard + render
      src/sync.rs                # `sync` (lgp) + `push-all` (lgpp): act, then overview
      src/tree.rs                # `tree` (lt): build the tree + render
      src/remote.rs              # `ssh`: rewrite HTTPS remotes to SSH
      src/passthrough.rs         # the git verbs (status/add/commit/pull/push): exec git
      src/ui.rs                  # colour + hyperlink + progress-bar discipline
```

There is **one binary**, `grove`. It bundles four kinds of command:

- **Passthrough git verbs** — `status`, `add`, `commit`, `pull`, `push`. Each
  exec-replaces the process with `git` (on Unix) so colour, pager, signals, and
  the exit code are git's own. grove leaves no wrapper behind.
- **Multi-repo tools** — `overview`, `sync`, `push-all`, over a folder of repos.
- **The tree view** — `tree`.
- **Shell-alias setup** — `setup`, `init`, `example`, plus `ssh` and hidden
  `completions`/`man`.

The short names you actually type — `gs`/`ga`/`gc`/`gcp`/`gp`/`gpp` for the git
verbs and `lg`/`lgp`/`lgpp`/`lt` for the multi-repo/tree tools — are **shell
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

- `overview::collect(dir, fetch) -> Report`, `overview::render_human(&Report)`
- `sync::run(dir) -> SyncReport`, `sync::render_human(&SyncReport)`
- `sync::push_all(dir) -> PushReport`, `sync::render_push(&PushReport)`
- `tree::collect(dir, level, all) -> TreeReport`, `tree::render_human(&TreeReport)`

The `Report` types are `#[derive(Serialize)]`; the CLI renders JSON with a single
`serde_json::to_string_pretty(&report)`. `sync`/`push-all` embed the post-run
`overview::Report`, so their JSON carries the dashboard as it stands afterwards.

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

## `grove-core` module responsibilities

- **`git`** — the only module that shells out to `git`. Going through the real
  git binary (not a library) means the user's config, credentials, and SSH agent
  all apply. `discover` finds the immediate sub-repos; `is_https`/`web_url`/
  `ahead_behind`/`dirty`/`fetch`/`pull`/`push` read or act on one repo.
- **`overview`** — the dashboard: discover, fetch ssh repos in parallel, classify
  each into the roll-up buckets, render the aligned colour table + hints.
- **`sync`** — fast-forward-pull the strictly-behind and push the strictly-ahead
  clean repos; `push_all` is the push-only, worktree-agnostic variant.
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
