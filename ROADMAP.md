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
