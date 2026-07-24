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

```sh
brew install jakobhviid/tap/grove       # macOS & Linux
```

Then add one line to your shell rc:

```sh
eval "$(grove init zsh)"                 # zsh
eval "$(grove init bash)"                # bash
grove init fish | source                 # fish
```

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
| `lg`  | `grove overview`   | Dashboard of every repo in a folder: branch, ahead/behind, dirty    |
| `lgp` | `grove sync`       | Auto pull/push clean in-sync repos, then show the overview          |
| `lt`  | `grove tree`       | Tree view (2 levels, icons); git repos get a git icon               |

Everything is a `grove` subcommand — uniform, and the passthroughs `exec` git so
color, pager, signals, and exit codes are unchanged.

## Notes

- `overview`/`sync` fetch repos in parallel and flag any `origin` still on HTTPS
  (so you can switch it to SSH). Dirty or diverged repos are never auto-synced.
- `tree` has no external dependencies (no eza) and hides dotfiles by default.
- Nerd Font icons — use a Nerd Font for `lt` to render correctly.
