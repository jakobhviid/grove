# ROADMAP

Future work, known gaps, and anything deferred. This is the **one** place such
notes live — not a `TODO.md`, not scattered `// TODO` comments, not the README.

## Planned / deferred

- **`grove ssh --json`.** `ssh` is the one data-touching verb without a
  machine-readable mode: it previews changes and asks for confirmation, but has
  no `--json`. Add a `--json` document (the planned rewrites + what was applied),
  consistent with `overview`/`sync`/`pull-all`/`push-all`/`tree`, so an agent can
  drive the HTTPS→SSH migration.
- **Completion parity for bash & fish.** The hand-written zsh completion delegates
  the git verbs to zsh's own `_git` and offers folder completion for the
  multi-repo verbs. bash and fish only get the clap-generated fallback, which
  does neither. Either hand-write bash/fish equivalents or enrich the generated
  ones.
- **Deeper repo discovery.** `overview`/`sync`/`pull-all`/`push-all` scan only the
  *immediate* subdirectories of the folder. A `--depth N` (or recursive) mode
  would pick up nested layouts (e.g. `~/src/org/repo`).
- **`tree` polish.** Optional `.gitignore` awareness and file-type-aware icons,
  so it reads closer to `eza --tree` without taking on `eza`'s dependency weight.

## Known gaps

- Windows is unsupported by design (the passthrough verbs `exec`-replace with git
  on Unix; the release targets are macOS + Linux only).
- Progress bars and colour are TTY-gated but there is no `--no-color` / `--quiet`
  flag; suppress colour with `NO_COLOR`.

## Done

- **`grove setup` activates the aliases where you ran it.** Setup provisioned the
  files and then left you in a shell that knew nothing about them. A process can't
  add aliases to its parent shell, so setup now takes whichever honest route
  applies: piped stdout emits the alias lines for `eval "$(grove setup)"` (report
  moves to stderr, as in `init`); on a terminal it offers to `exec` a fresh
  interactive shell that re-reads the rc. Gated on a terminal, the shell `$SHELL`
  reports, an actual change, and not already inside a handoff (`GROVE_RELOADED`);
  `--reload` / `--no-reload` / `GROVE_NO_RELOAD` override the prompt, and anything
  ineligible falls back to printing the line to run. Setup also warns when `grove`
  isn't on `PATH`, which silently neuters the guarded rc line.
- **`pull-all` pulls diverged repos too.** It filtered to strictly-behind, so a
  diverged repo (`↑n ↓m`) was skipped even though a plain `git pull` (with the
  user's `pull.rebase`) resolves it. Now it runs `git pull` in every behind repo —
  ff for strictly-behind, rebase/merge for diverged per the user's config — and on
  a conflict (or dirty tracked changes) aborts the in-progress rebase/merge so no
  repo is left half-applied. Fleet pull/push now capture git's output (no conflict
  wall leaking) and report only the repos that actually moved.
- **Fleet fetch: per-repo cache + wide parallelism.** Fetching now runs on a
  network-sized pool (not the CPU-sized one), and a per-repo cache (on by default)
  skips re-fetching any repo a recent fetch left fully settled — dirty/ahead/behind/
  diverged repos always re-fetch, so actionable state stays live; cached rows are
  marked, `--force` bypasses. Measured on a 27-repo fleet: cold 6.6s → 4.3s, and a
  repeat run cached 24/27 and only fetched the 3 active repos. (Benchmarked SSH
  `ControlMaster` grouping too; the cache + wide pool made it redundant, so it was
  not adopted.) The `sync` family was refactored to act off an already-collected
  `overview::Report` (collect → act → re-read), so fetching lives in one place.
- **Symmetric multi-repo trio + settings.** `sync` took the honest `lgs` alias
  (it does both directions); `lgp` became the new `grove pull-all` (fast-forward
  every behind repo), the mirror of `push-all` (`lgpp`). The listings and `→`
  hints resolve to the user's actual aliases and nudge `grove setup` when none are
  provisioned. Added `grove configure` over a `~/.config/grove/config` settings
  file (`cache`, `cache_ttl`, `default_dir`), a fetch-freshness cache under
  `~/.cache/grove` (skips only the network fetch; local state stays fresh), a
  default-dir fallback for git-irrelevant folders (which `grove setup` offers as
  a pick-list of the repo folders detected under your home), and clickable
  `file://` repo/folder names in `overview` and `tree`.
- **2.0 — collapse to one binary.** `lg`/`lgp`/`lgpp`/`lt` became the `grove`
  subcommands `overview`/`sync`/`push-all`/`tree`, exposed as `grove setup`
  aliases; `--json` added to the data-producing verbs; the CI green gate,
  integration tests, `ARCHITECTURE.md`, and `.gitattributes` landed alongside.
