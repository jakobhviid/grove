# grove workflows

How grove is set up and used — for humans and for coding agents / provisioning
scripts. This file is also emitted verbatim by `grove --llm`.

## The model in one paragraph

grove is **one binary**. It bundles the everyday git verbs (`status add commit
pull push`, each exec-ing git), the multi-repo tools (`overview sync pull-all
push-all` over a folder of repos), a `tree` view, shell-alias setup, and its own
settings (`grove configure`). The short names you type day-to-day — `gs ga gc gcp
gp gpp` for the git verbs and `lg lgs lgp lgpp lt` for the multi-repo/tree tools —
are **shell aliases**, not binaries: nothing short lands on `PATH`, so nothing
collides with other tools (notably `lg` vs lazygit), and you can rename any that
clash by editing one file. The aliases are provisioned by `grove setup` (or wired
up by hand with `grove init`).

## Commands (always work, no setup)

The full subcommands work the moment `grove` is on `PATH` — in any shell,
interactive or not, including scripts and CI:

```sh
grove status              # git status (forwards extra args)
grove add [paths]         # git add (stages "." by default)
grove commit <msg>        # git commit -m <msg>   (-a stage tracked, -p push after)
grove pull                # git pull
grove push                # git push

grove overview [dir]      # dashboard of every repo in a folder (alias: lg)
grove sync    [dir]       # pull the behind + push the ahead clean repos, then overview (alias: lgs)
grove pull-all [dir]      # fast-forward every repo that is behind, then overview (alias: lgp)
grove push-all [dir]      # push every repo with unpushed commits, then overview (alias: lgpp)
grove tree    [dir] [-a] [-l N]   # tree view; git repos get a git icon (alias: lt)
grove ssh     [dir] [-y]  # switch a folder's HTTPS remotes to SSH (previews & asks; -y skips)
grove configure [key] [value]     # get/set settings: cache, cache_ttl, default_dir
```

The multi-repo verbs are a symmetric trio: **`sync` (`lgs`)** does both directions
for the clean, in-sync repos and is the everyday one; **`pull-all` (`lgp`)** and
**`push-all` (`lgpp`)** each do a single direction for when you want manual control.
Each repo name in the dashboard is a clickable `file://` link that opens the folder
(where the terminal supports OSC 8), alongside the forge glyph that opens its web page.

Because the subcommands are real commands (not aliases), **scripts and
automation should call them directly** — `grove commit -a -p "msg"`, `grove
overview ~/src`, etc. The short aliases are purely an interactive-prompt
convenience and are not expanded in scripts.

## Machine-readable output (for agents)

`overview`, `sync`, `push-all`, and `tree` take `--json` — one document to
stdout, progress to stderr, so the pipe stays clean:

```sh
grove overview ~/src --json     # {dir, repos:[{name,path,branch,https,web_url,ahead,behind,staged,modified,untracked}], summary:{...}}
grove sync ~/src --json         # {synced:[{name,op}], overview:{...}}   ← act + re-check in one call
grove pull-all ~/src --json     # {pulled:[name], overview:{...}}
grove push-all ~/src --json     # {pushed:[name], overview:{...}}
grove tree ~/src -l 1 --json    # {root, entries:[{name,type,is_repo,children:[...]}]}
```

Convention for agents: run a verb with `--json`, gate on the counts (`summary`,
`synced`, `pushed`), then act. The passthrough git verbs have no `--json` — they
exec git, which owns their output; drive them by exit code as usual.

## Interactive setup: the short aliases

The short names are the interactive nicety. Provision them once:

```sh
grove setup               # auto-detects your shell from $SHELL
grove setup zsh           # or name it explicitly (zsh | bash | fish)
```

`grove setup`:

1. Writes `~/.config/grove/aliases` (the *grove file*) from a starter template
   if it doesn't exist yet, or — on an existing file — tops up any default alias
   it's missing (this is how an older file gains `lg`/`lgs`/`lgp`/`lgpp`/`lt`) and
   offers to reconcile any default whose name is bound to a different command.
2. Appends one idempotent, marker-delimited block to your shell rc
   (`~/.zshrc`, `~/.bashrc`, or `~/.config/fish/config.fish`) that loads the
   aliases on every startup. Re-running never adds a second block.
3. Prints exactly what it changed.

Then open a new shell (or `source ~/.zshrc`). Now `gs`, `gc`, `lg`, `lt`, … work.

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
where you resolve a collision like `gc` or `lg`:

```
# rename gc if it clashes with another tool:
gk  = grove commit
gcp = grove commit --all --push
# rename lg if you use lazygit:
gv  = grove overview
# your own shortcuts:
st  = grove status
```

Run `grove example` to print the starter file (it defines all the defaults).

## Unattended / scripted provisioning

`grove setup` is safe for non-interactive use: it prompts for nothing, is
idempotent, and exits non-zero only on real errors (e.g. `$HOME` unset). Pass
`--force` to reconcile a divergent alias without the interactive prompt.

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
only *runs* git operations, skip setup entirely and call the full subcommands,
which need no shell state:

```sh
grove add
grove commit -a -p "ci: regenerate"    # commit everything and push
grove push-all ~/repos                 # push every ahead repo under ~/repos
```

**Wire the rc line yourself** (if you don't want grove editing files):

```sh
mkdir -p ~/.config/grove && grove example > ~/.config/grove/aliases
echo 'command -v grove >/dev/null 2>&1 && eval "$(grove init zsh)"' >> ~/.zshrc
```

## Completions

Homebrew and the `curl` installer place a zsh completion file (`_grove`) that
covers `grove` and all its subcommands. The short aliases inherit grove's
completion automatically (zsh resolves `alias lg='grove overview'` and completes
it as `grove overview`). The git verbs delegate to zsh's own git completion, so
`grove status`/`gs` complete exactly like `git status`.

## Everyday workflows

**Single repo** (short aliases shown; the real commands are `grove status`, …):

```sh
gs                         # what changed?
ga                         # stage everything (or: ga path/to/file)
gc fix the parser          # commit (message = the trailing words)
gcp fix the parser         # commit -a and push in one step
gp ; gpp                   # pull ; push
```

**A folder full of repos** (e.g. `~/src`):

```sh
lg ~/src                   # dashboard: branch, ahead/behind, dirty, forge link, HTTPS flags
lgs ~/src                  # sync: fast-forward the behind ones, push the ahead ones
lgp ~/src                  # pull-all: just fast-forward everything that's behind
lgpp ~/src                 # push-all: just push everything with unpushed commits
grove ssh ~/src            # rewrite any HTTPS remotes to SSH (previews & asks; -y to skip)
```

`grove overview` ends with a severity roll-up (`N repos · X clean · Y dirty · Z
to push …`) and `→` hints naming the exact command that clears each kind of
pending work — in the short aliases you actually have (e.g. `lgpp`), or the long
`grove …` forms plus a `grove setup` nudge when you haven't provisioned them yet.

## Settings (`grove configure`)

Three optional knobs live in `~/.config/grove/config` (same `key = value` shape
as the grove file). `grove configure` lists them; `grove configure <key> <value>`
sets one:

```sh
grove configure                         # list every setting + its value
grove configure default_dir ~/Developer # where the multi-repo verbs run when the
                                        #   current folder has no repos of its own
grove configure cache off               # disable the fetch-freshness cache (default on)
grove configure cache_ttl 10            # seconds a fetch stays fresh (default 5)
```

- **`cache`** / **`cache_ttl`** — after a multi-repo verb fetches a folder, a
  follow-up on the same folder within `cache_ttl` seconds reuses that fetch instead
  of hitting the network again. Only the fetch is skipped; ahead/behind and dirty
  are always recomputed, so it never acts on stale local state.
- **`default_dir`** — when you run `lg`/`lgs`/`lgp`/`lgpp` in a folder that has
  nothing to do with git (not inside a repo, no repo subfolders), grove runs in
  this folder instead and prints a dim note saying so. An explicit `dir` argument
  always wins; unset, nothing changes.
