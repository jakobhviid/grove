# grove

Your git shortcuts as **one portable binary** — install once, get the same
commands in **zsh, bash, or fish**, on **macOS or Linux**. `grove` bundles the
everyday git verbs (`status add commit pull push`, each `exec`-ing git), four
multi-repo tools with real logic — a **dashboard** across a folder of repos, an
**auto pull/push sync**, and single-direction **pull-all** / **push-all** — and a
self-contained **tree** view that flags which folders are git repos. All are
subcommands of the single `grove` binary.

The short names you actually type — `gs ga gc gcp gp gpp` for the git verbs and
`lg lgs lgp lgpp lt` for the multi-repo/tree tools — are **shell aliases**, not
binaries. Nothing short lands on your `PATH`, so nothing collides with other
tools (notably `lg`, which many people alias to lazygit) — and you can rename
any that clash by editing one file. `grove setup` provisions them for your shell
in one step, and `brew upgrade grove` updates every machine and every shell at
once.

## Install

**Homebrew** (macOS & Linux) — pours a prebuilt bottle on x86_64 Linux, so no
compiler/build tools are needed:

```sh
brew install jakobhviid/tap/grove
```

**Or paste one line** — no Homebrew, no compiler, no root (installs to
`~/.local/bin`; override with `GROVE_BIN_DIR`). Ideal for servers, containers,
and immutable distros:

```sh
curl -fsSL https://raw.githubusercontent.com/jakobhviid/grove/main/install.sh | sh
```

Then wire the short aliases into your shell (once):

```sh
grove setup                # writes the grove file + one line in your shell rc
```

Build from source (needs Rust):

```sh
cargo build --release      # the grove binary lands in ./target/release/
```

## Upgrading from 1.x

In **2.0** the separate `lg` / `lgp` / `lgpp` / `lt` binaries became `grove`
subcommands — `grove overview` / `sync` / `push-all` / `tree`. The short names
live on as **shell aliases**: run `grove setup` once and it adds them to your
grove file (topping up any that are missing), then open a new shell. Scripts
should call the full subcommands (`grove overview ~/src`), which need no shell
setup.

Since then the multi-repo alias set became a symmetric trio: **`lgs`** is now
`grove sync` (it does both directions, so "sync" is the honest name), **`lgp`** is
the new **`grove pull-all`** (fast-forward every behind repo), and **`lgpp`** stays
`grove push-all`. `grove setup` tops up `lgs` and offers to migrate a `lgp` that
still points at `sync`.

## Commands

`grove` is one binary; everything is a subcommand. They all work the moment
`grove` is on `PATH` — no shell setup. The git verbs forward any extra arguments
straight to git, so flags, color, pager, signals, and exit codes are git's own.

| Command | Alias | What it does |
|---|---|---|
| `grove status [args]` | `gs` | `git status` — extra args forwarded |
| `grove add [paths]` | `ga` | `git add` — stages `.` by default, or the paths you pass |
| `grove commit [-a] [-p] <msg>` | `gc` | `git commit -m <msg>`; `-a`/`--all` stages tracked changes first, `-p`/`--push` pushes after a successful commit |
| `grove pull [args]` | `gp` | `git pull` |
| `grove push [args]` | `gpp` | `git push` |
| `grove overview [dir]` | `lg` | **Dashboard** of every repo in a folder: branch, ahead/behind, dirty counts, a clickable repo name that opens the folder, and a clickable forge icon linking to its web page — with a summary roll-up and next-step hints |
| `grove sync [dir]` | `lgs` | **Sync**: fast-forward-pull the behind repos and push the ahead ones (only clean, in-sync ones), then show the dashboard |
| `grove pull-all [dir]` | `lgp` | **Bulk pull**: fast-forward every repo that is behind, then show the dashboard (no push) |
| `grove push-all [dir]` | `lgpp` | **Bulk push**: push every repo with unpushed commits, then show the dashboard (no pull) |
| `grove tree [dir] [-a] [-l N]` | `lt` | **Tree** view (2 levels by default, `-l` to change, `-a` for dotfiles); git repos get a git icon, folder names are clickable |
| `grove ssh [dir] [-y]` | — | **Switch to SSH**: rewrite the HTTPS remotes of every repo in a folder to SSH (so `overview`/`sync`/`pull-all`/`push-all` can fetch them). Previews every change and asks first; `-y` skips the prompt |
| `grove configure [key] [value]` | — | **Settings**: get/set `cache`, `cache_ttl`, `default_dir` in `~/.config/grove/config` (no args lists them all) |

The multi-repo commands (`overview`, `sync`, `pull-all`, `push-all`) operate on
the **immediate subdirectories** of the folder (default: the current directory)
that contain a `.git`. `--version`/`-V` and a man page are available; the data
tools take **`--json`** (see below). Run bare `grove` for a one-screen overview
of the whole suite. Set a **`default_dir`** — `grove setup` shows a menu of the
repo folders under your home to pick from, or set it with `grove configure
default_dir <path>` — and the multi-repo verbs fall back to it when the current
folder has no repos of its own.

## Machine-readable output (`--json`)

The data-producing verbs — `overview`, `sync`, `pull-all`, `push-all`, `tree` —
take `--json`, which emits one document to stdout (progress still goes to stderr,
so the pipe stays clean):

```sh
grove overview ~/src --json | jq '.summary'
grove tree ~/src -l 1 --json | jq '.entries[] | select(.is_repo)'
```

`sync --json`, `pull-all --json`, and `push-all --json` report what they touched
**and** embed the post-run dashboard, so an agent can act and re-check in one
call. The passthrough git verbs have no `--json` — they `exec` git, and git owns
their output.

## Shell aliases

The short names are shell aliases, kept in a *grove file* so there's one source
of truth. Provision them in one step:

```sh
grove setup                # auto-detects your shell from $SHELL
grove setup zsh            # or name it explicitly (zsh | bash | fish)
```

`grove setup` writes `~/.config/grove/aliases` (a starter grove file) if it's
missing — and, on an existing file, tops up any default alias it's missing (this
is how a 1.x file gains `lg`/`lgp`/`lgpp`/`lt`). It also appends **one
idempotent, marker-delimited block** to your shell rc (`~/.zshrc`, `~/.bashrc`,
or `~/.config/fish/config.fish`) that loads the aliases on every startup.
Re-running never adds a second block. Then open a new shell.

Prefer to manage your own dotfiles? `grove init <shell>` just **prints** the
alias lines (it changes nothing) — the block `grove setup` writes simply calls
it:

```sh
eval "$(grove init zsh)"       # zsh / bash
grove init fish | source       # fish
```

The grove file is shell-agnostic `name = command` lines (`grove example` prints a
starter). Edit it to rename a clashing alias or add your own:

```
gc  = grove commit          # rename to `gk` if `gc` clashes on your system
lg  = grove overview        # rename if you use `lg` for lazygit
co  = grove commit          # your own shortcuts, too
```

## Behavior notes

- **Dashboard (`grove overview`, alias `lg`).** Fetches every SSH repo in
  parallel first, then shows a row per repo: the **repo name is a clickable link
  that opens the folder** (a `file://` OSC 8 link), then a **clickable forge icon**
  (GitHub/GitLab/Bitbucket, else a generic git mark) linking to the repo's web
  page, then branch, sync state (`↑` ahead, `↓` behind, `✓` in sync, `—` no
  upstream), and dirty counts (`+` staged, `!` modified, `?` untracked). Both
  links are OSC 8 terminal hyperlinks that appear **only on terminals that support
  them** — elsewhere the name is plain text and the icon column is omitted rather
  than left as a dead glyph (set `FORCE_HYPERLINK=1` to force it on). The forge
  icon uses a Nerd Font, like `grove tree`. Repos that need attention are **bold**;
  clean, in-sync repos stay plain. The table ends with a severity-colored roll-up
  (`N repos · X clean · Y dirty · Z to push …`) and `→` hints naming the command
  that clears each kind of pending work — in your own short aliases when they're
  installed, else the long `grove …` forms plus a one-line `grove setup` nudge.
- **HTTPS remotes are flagged, not fetched.** Any repo whose `origin` is still
  on HTTPS is called out and skipped during fetch/sync — run **`grove ssh`** to
  rewrite them all to SSH (it previews each change and asks before touching
  anything; embedded tokens are dropped, ports become `ssh://`, then it fetches
  and reprints the dashboard so the switch is confirmed).
- **Sync is conservative; pull-all / push-all are the escape hatches.** `grove
  sync` (`lgs`) only touches clean repos with an upstream: it fast-forward-pulls
  the ones strictly behind and pushes the ones strictly ahead. Dirty, diverged,
  HTTPS, and upstream-less repos are left untouched. The single-direction pair do
  one side each and skip diverged repos: `grove pull-all` (`lgp`) fast-forwards
  every repo strictly behind (git refuses any pull that would clobber uncommitted
  changes, so those simply stay behind), and `grove push-all` (`lgpp`) pushes
  every repo strictly ahead — never pulling and not requiring a clean worktree.
- **Fast on big fleets.** Fetching runs on a wide pool (network-bound, not
  CPU-bound), and a **per-repo cache** (on by default) skips re-fetching any repo a
  recent fetch left fully settled — clean and in sync. Anything dirty, ahead,
  behind, or diverged **always** re-fetches, so the repos you'd act on are never
  stale; only the quiet rows can lag, for at most `cache_ttl` seconds (default 5),
  and they're marked `cached` in the roll-up. `--force` (`-f`) re-fetches
  everything; `grove configure cache off` disables it. On a settled fleet a repeat
  run fetches only the handful of active repos.
- **Tree (`grove tree`, alias `lt`)** has no external dependencies (no eza),
  lists directories before files, hides dotfiles unless `-a` is given, and makes
  folder names clickable (a `file://` link that opens the directory).
- **Nerd Font icons** — use a Nerd Font for `grove tree` and the dashboard's
  forge links to render correctly.
- The git verbs `exec` git in place (leaving no wrapper process); outside a repo
  they print a friendly one-line error instead of git's `fatal:` wall of text.

## Completions

Homebrew and the `curl` installer place a zsh completion file (`_grove`) covering
`grove` and all its subcommands. The short aliases inherit grove's completion
automatically (zsh resolves `alias lg='grove overview'` and completes it as
`grove overview`), and the git verbs delegate to zsh's own git completion — so
`grove status` (and `gs`) tab-complete exactly like `git status`. bash/fish get
clap-generated completions for `grove`.

## For scripts & agents

The full subcommands need no shell state, so **automation should call them
directly** (aliases aren't expanded in scripts anyway): `grove commit -a -p
"msg"`, `grove overview ~/repos --json`, `grove push-all ~/repos`, and so on.
`grove setup` is safe to run unattended (idempotent, no prompts). `grove --llm`
prints a single self-contained, machine-readable guide — the command reference,
ARCHITECTURE, a WORKFLOWS section, and this README — so an agent can drive the
suite from zero.

## AI disclosure

Parts of this project were written with the assistance of AI coding agents (Claude
Code, opencode, and others). All changes are reviewed by the maintainer. This is the
single place that fact is disclosed; it is deliberately kept out of the commit history.
