# grove

Your git shortcuts as **portable binaries** — install once, get the same
commands in **zsh, bash, or fish**, on **macOS or Linux**. `grove` bundles the
everyday git verbs as subcommands (`status add commit pull push`, each `exec`-ing
git) and ships three multi-repo tools with real logic — a **dashboard** across a
folder of repos, an **auto pull/push sync**, and a **bulk push** — plus a
self-contained **tree** view that flags which folders are git repos.

The short names you actually type (`gs ga gc gcp gp gpp`) are **shell aliases**,
not binaries. Nothing short lands on your `PATH`, so nothing collides with other
tools — and you can rename any that clash (say `gc`) by editing one file.
`grove setup` provisions them for your shell in one step, and `brew upgrade
grove` updates every machine and every shell at once.

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
cargo build --release      # binaries land in ./target/release/
```

## Commands

`grove` bundles the git verbs; `lg lgp lgpp lt` are standalone binaries. All of
these work the moment they're on `PATH` — no shell setup. The git verbs forward
any extra arguments straight to git, so flags, color, pager, signals, and exit
codes are git's own.

| Command | What it does |
|---|---|
| `grove status [args]` | `git status` — extra args forwarded |
| `grove add [paths]` | `git add` — stages `.` by default, or the paths you pass |
| `grove commit [-a] [-p] <msg>` | `git commit -m <msg>`; `-a`/`--all` stages tracked changes first, `-p`/`--push` pushes after a successful commit |
| `grove pull [args]` | `git pull` |
| `grove push [args]` | `git push` |
| `grove ssh [dir] [-y]` | **Switch to SSH**: rewrite the HTTPS remotes of every repo in a folder to SSH (so `lg`/`lgp`/`lgpp` can fetch them). Previews every change and asks first; `-y` skips the prompt |
| `lg [dir]` | **Dashboard** of every repo in a folder: branch, ahead/behind, dirty counts, and a clickable forge icon linking to its web page — with a summary roll-up and next-step hints |
| `lgp [dir]` | **Sync**: fast-forward-pull the behind repos and push the ahead ones (only clean, in-sync ones), then show the dashboard |
| `lgpp [dir]` | **Bulk push**: push every repo with unpushed commits, then show the dashboard (no pull) |
| `lt [dir] [-a] [-l N]` | **Tree** view (2 levels by default, `-l` to change, `-a` for dotfiles); git repos get a git icon |

The short aliases `grove setup` installs map onto the git verbs:
`gs`→`grove status`, `ga`→`grove add`, `gc`→`grove commit`,
`gcp`→`grove commit --all --push`, `gp`→`grove pull`, `gpp`→`grove push`.

The multi-repo commands (`lg`, `lgp`, `lgpp`) operate on the **immediate
subdirectories** of the folder (default: the current directory) that contain a
`.git`. `--version`/`-V` and a man page are available on every command.

## Shell aliases

The short names are shell aliases, kept in a *grove file* so there's one source
of truth. Provision them in one step:

```sh
grove setup                # auto-detects your shell from $SHELL
grove setup zsh            # or name it explicitly (zsh | bash | fish)
```

`grove setup` writes `~/.config/grove/aliases` (a starter grove file) if it's
missing, and appends **one idempotent, marker-delimited block** to your shell rc
(`~/.zshrc`, `~/.bashrc`, or `~/.config/fish/config.fish`) that loads the aliases
on every startup. Re-running never adds a second block. Then open a new shell.

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
gcp = grove commit --all --push
gl  = lg                    # your own shortcuts, too
```

## Behavior notes

- **Dashboard (`lg`).** Fetches every SSH repo in parallel first, then shows a
  row per repo: a **clickable forge icon** (GitHub/GitLab/Bitbucket, else a
  generic git mark) linking to the repo's web page, then branch, sync state
  (`↑` ahead, `↓` behind, `✓` in sync, `—` no upstream), and dirty counts (`+`
  staged, `!` modified, `?` untracked). The icon is an OSC 8 terminal
  hyperlink derived from `origin` whatever its transport, and appears **only on
  terminals that support hyperlinks** — elsewhere the column is omitted rather
  than left as a dead glyph (set `FORCE_HYPERLINK=1` to force it on). It uses a
  Nerd Font icon, like `lt`. Repos that need attention are **bold**; clean,
  in-sync repos stay plain. The
  table ends with a severity-colored roll-up (`N repos · X clean · Y dirty · Z
  to push …`) and `→` hints naming the command that clears each kind of pending
  work.
- **HTTPS remotes are flagged, not fetched.** Any repo whose `origin` is still
  on HTTPS is called out and skipped during fetch/sync — run **`grove ssh`** to
  rewrite them all to SSH (it previews each change and asks before touching
  anything; embedded tokens are dropped, ports become `ssh://`, then it fetches
  and reprints the dashboard so the switch is confirmed).
- **Sync is conservative.** `lgp` only touches clean repos with an upstream:
  it fast-forward-pulls the ones strictly behind and pushes the ones strictly
  ahead. Dirty, diverged, HTTPS, and upstream-less repos are left untouched.
  `lgpp` pushes every repo strictly ahead — it never pulls and does not require a
  clean worktree, and skips diverged repos a plain push would reject.
- **Tree (`lt`)** has no external dependencies (no eza), lists directories
  before files, and hides dotfiles unless `-a` is given.
- **Nerd Font icons** — use a Nerd Font for `lt` to render correctly.
- The git verbs `exec` git in place (leaving no wrapper process); outside a repo
  they print a friendly one-line error instead of git's `fatal:` wall of text.

## Completions

Homebrew and the `curl` installer place a zsh completion file (`_grove`) covering
`grove` and `lg lgp lgpp lt`. The short aliases inherit grove's completion
automatically, and the git verbs delegate to zsh's own git completion — so
`grove status` (and `gs`) tab-complete exactly like `git status`. bash/fish get
clap-generated completions for `grove`.

## For scripts & agents

The full commands need no shell state, so **automation should call them
directly** (aliases aren't expanded in scripts anyway): `grove commit -a -p
"msg"`, `lgpp ~/repos`, and so on. `grove setup` is safe to run unattended
(idempotent, no prompts). `grove --llm` prints a single self-contained,
machine-readable guide — the command reference, a WORKFLOWS section (including
unattended provisioning), and this README — so an agent can drive the suite from
zero.

## AI disclosure

Parts of this codebase were written with the assistance of AI coding agents
(Claude Code, opencode, and others). All changes were reviewed by the maintainer.
