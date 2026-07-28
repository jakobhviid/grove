# grove workflows

How grove is set up and used — for humans and for coding agents / provisioning
scripts. This file is also emitted verbatim by `grove --llm`.

## The model in one paragraph

grove ships a handful of binaries: **`grove`** (which bundles the everyday git
verbs as subcommands — `status add commit pull push` — plus setup) and the
standalone multi-repo/tree tools **`lg lgp lgpp lt`**. The short names you type
day-to-day (`gs ga gc gcp gp gpp`) are **shell aliases**, not binaries: nothing
short lands on `PATH`, so nothing collides with other tools, and you can rename
any that clash (e.g. `gc`) by editing one file. The aliases are provisioned by
`grove setup` (or wired up by hand with `grove init`).

## Commands (always work, no setup)

The full commands work the moment the binaries are on `PATH` — in any shell,
interactive or not, including scripts and CI:

```sh
grove status              # git status (forwards extra args)
grove add [paths]         # git add (stages "." by default)
grove commit <msg>        # git commit -m <msg>   (-a stage tracked, -p push after)
grove pull                # git pull
grove push                # git push
grove ssh [dir] [-y]      # switch a folder's HTTPS remotes to SSH (previews & asks; -y skips)

lg  [dir]                 # dashboard of every repo in a folder (incl. its web URL)
lgp [dir]                 # pull/push the clean, in-sync repos, then show lg
lgpp [dir]                # push every repo with unpushed commits (no pull)
lt  [dir] [-a] [-l N]     # tree view; git repos get a git icon
```

Because these are real commands (not aliases), **scripts and automation should
call them directly** — `grove commit -a -p "msg"`, `lg ~/src`, etc. Aliases are
purely an interactive-prompt convenience and are not expanded in scripts.

## Interactive setup: the short aliases

The short names are the interactive nicety. Provision them once:

```sh
grove setup               # auto-detects your shell from $SHELL
grove setup zsh           # or name it explicitly (zsh | bash | fish)
```

`grove setup`:

1. Writes `~/.config/grove/aliases` (the *grove file*) from a starter template
   if it doesn't exist yet.
2. Appends one idempotent, marker-delimited block to your shell rc
   (`~/.zshrc`, `~/.bashrc`, or `~/.config/fish/config.fish`) that loads the
   aliases on every startup. Re-running never adds a second block.
3. Prints exactly what it changed.

Then open a new shell (or `source ~/.zshrc`). Now `gs`, `gc`, `gcp`, … work.

### Manual / explicit setup

`grove init <shell>` is the plain emitter — it **prints** the alias lines and
changes nothing. It's what the `grove setup` block calls, and what to use if you
manage your own dotfiles:

```sh
eval "$(grove init zsh)"      # zsh / bash — add to your rc yourself
grove init fish | source      # fish
```

## The grove file

`~/.config/grove/aliases` is shell-agnostic `name = command` lines; `grove init`
translates them per shell. Edit it to rename, add, or drop aliases — this is
where you resolve a collision like `gc`:

```
# rename gc if it clashes with another tool:
gk  = grove commit
gcp = grove commit --all --push
# your own shortcuts:
gl  = lg
```

Run `grove example` to print the starter file.

## Unattended / scripted provisioning

`grove setup` is safe for non-interactive use: it prompts for nothing, is
idempotent, and exits non-zero only on real errors (e.g. `$HOME` unset).

**Provision a shell in a container / image** (so an eventual interactive shell
has the aliases):

```dockerfile
RUN curl -fsSL https://raw.githubusercontent.com/jakobhviid/grove/main/install.sh | sh
ENV PATH="/root/.local/bin:${PATH}"
RUN grove setup zsh                    # writes the grove file + ~/.zshrc block
```

**Provision for a specific user in a bootstrap script:**

```sh
export HOME=/home/dev SHELL=/bin/zsh
grove setup                            # detects zsh from $SHELL, writes ~dev's files
```

**Don't provision at all — just use the commands.** For CI or any script that
only *runs* git operations, skip setup entirely and call the full commands,
which need no shell state:

```sh
grove add
grove commit -a -p "ci: regenerate"    # commit everything and push
lgpp ~/repos                           # push every ahead repo under ~/repos
```

**Wire the rc line yourself** (if you don't want grove editing files):

```sh
mkdir -p ~/.config/grove && grove example > ~/.config/grove/aliases
echo 'command -v grove >/dev/null 2>&1 && eval "$(grove init zsh)"' >> ~/.zshrc
```

## Completions

Homebrew and the `curl` installer place a zsh completion file (`_grove`) that
covers `grove` (with its subcommands) and `lg lgp lgpp lt`. The short aliases
inherit grove's completion automatically (zsh resolves `alias gc='grove commit'`
and completes it as `grove commit`). The git verbs delegate to zsh's own git
completion, so `grove status`/`gs` complete exactly like `git status`.

## Everyday workflows

**Single repo:**

```sh
gs                         # what changed?
ga                         # stage everything (or: ga path/to/file)
gc fix the parser          # commit (message = the trailing words)
gcp fix the parser         # commit -a and push in one step
gp ; gpp                   # pull ; push
```

**A folder full of repos** (e.g. `~/src`):

```sh
lg ~/src                   # dashboard: branch, ahead/behind, dirty, web URL, HTTPS flags
lgp ~/src                  # fast-forward the behind ones, push the ahead ones
lgpp ~/src                 # just push everything with unpushed commits
grove ssh ~/src            # rewrite any HTTPS remotes to SSH (previews & asks; -y to skip)
```

`lg` ends with a severity roll-up (`N repos · X clean · Y dirty · Z to push …`)
and `→` hints naming the exact command that clears each kind of pending work.
