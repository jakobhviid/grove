# grove

Your git shortcuts as **one portable binary** — install once, get the same
aliases in **zsh, bash, or fish**, on **macOS or Linux**. `grove` replaces a pile
of shell functions with a single tool: thin passthroughs for the everyday git
verbs, plus two commands with real logic — a **multi-repo dashboard** and an
**auto pull/push sync** across a folder of repos — and a self-contained **tree**
view that flags which folders are git repos.

The trivial aliases aren't rewritten in Rust; they're *shipped* by it. `grove
init <shell>` prints them for your shell, so there's one source of truth and
`brew upgrade grove` updates every machine and every shell at once.

## Install

**Homebrew** (macOS & Linux) — pours a prebuilt bottle on x86_64 Linux, so no
compiler/build tools are needed:

```sh
brew install jakobhviid/tap/grove
```

**Or paste one line** — no Homebrew, no compiler, no root (installs to
`~/.local/bin`). Ideal for servers, containers, and immutable distros:

```sh
curl -fsSL https://raw.githubusercontent.com/jakobhviid/grove/main/install.sh | sh
```

Either way the commands (`gst ga gc gp gpp lg lgp lt`) work immediately. The
short colliding aliases (`gs`, `gcp`) are opt-in — see the grove file below.

Build from source (needs Rust):

```sh
cargo build --release                    # → ./target/release/grove
```

## Commands & aliases

| Alias | Command            | What it does                                                        |
|-------|--------------------|---------------------------------------------------------------------|
| `gs`  | `grove status`     | `git status` (forwards extra args)                                  |
| `ga`  | `grove add`        | `git add` — stages `.` by default, or the paths you pass            |
| `gc`  | `grove commit`     | `git commit -m <msg>`                                               |
| `gcp` | `grove commit --all --push` | `git commit -a -m <msg>` then `git push`                   |
| `gp`  | `grove pull`       | `git pull`                                                          |
| `gpp` | `grove push`       | `git push`                                                          |
| `lg`  | `grove overview`   | Dashboard of every repo: branch, ahead/behind, dirty — clean repos dim so the ones needing work pop, with a summary + next-step hint |
| `lgp` | `grove sync`       | Auto pull/push clean in-sync repos, then show the overview          |
| `lt`  | `grove tree`       | Tree view (2 levels, icons); git repos get a git icon               |

Everything is a `grove` subcommand — uniform, and the passthroughs `exec` git so
color, pager, signals, and exit codes are unchanged.

## Notes

- `overview`/`sync` fetch repos in parallel and flag any `origin` still on HTTPS
  (so you can switch it to SSH). Dirty or diverged repos are never auto-synced.
- The dashboard ends with a severity-colored roll-up (`N repos · X clean · Y
  dirty · Z to push …`) and a `→` hint naming the command that clears each kind
  of pending work — so `lg`/`lgp`/`lgpp` give you the triage and the next step.
- `tree` has no external dependencies (no eza) and hides dotfiles by default.
- Nerd Font icons — use a Nerd Font for `lt` to render correctly.

## AI disclosure

Parts of this codebase were written with the assistance of AI coding agents
(Claude Code, opencode, and others). All changes were reviewed by the maintainer.
