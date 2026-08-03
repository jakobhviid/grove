# Agent guidelines

Instructions for any AI coding agent (Claude Code, opencode, Cursor, …) working
in this repository.

## Attribution — never attribute AI in the repo

- **Never** add AI/assistant attribution to commits or pull requests: no
  `Co-Authored-By: Claude` (or any other assistant) trailer, and no
  "🤖 Generated with …" line. Author every commit solely as the repository owner.
- AI assistance is disclosed **once**, in the README's "AI disclosure" section —
  that is the only place it belongs. Keep it out of the commit history entirely.

## Releases & versioning — auto-incremented from commit type

CI cuts a release on every push to `main`, and the version is **derived
automatically from the commit history** (Conventional Commits) — nobody bumps a
version by hand, so a forgotten manual release still versions correctly. The
commit **subject prefix** decides the bump:

- `feat: …` — a new feature → **minor** bump (1.2.0 → 1.3.0)
- `fix: …` — a bug fix / hotfix → **patch** bump (1.2.3 → 1.2.4)
- `feat!: …` (or any `type!:`, e.g. `fix!:`) — a breaking change → **major** bump
  (1.4.2 → 2.0.0)
- anything else (`docs:`, `chore:`, `refactor:`, …) or an un-prefixed subject →
  **patch** bump

So **pick the right commit-subject prefix for the change** and the release version
follows automatically. Never hand-edit `version` in `Cargo.toml` to release — CI
computes and stamps it.

Declare a breaking change with the **`!` in the subject** (`feat!:` / `fix!:`). A
`BREAKING CHANGE` *footer* is **not** scanned — the version awk reads commit
subjects only — so the subject bang is the one and only way to cut a major.

## The green gate — clippy is the release gate

CI won't publish a release unless the gate is green, so run it locally before
**every** push:

```sh
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Clippy warnings are treated as errors — a clean `cargo build`/`cargo test` is not
enough. There is **deliberately no `cargo fmt` gate**; formatting and readability
are a review concern, not a bot's.

## Docs are load-bearing

`grove --llm` embeds the repo docs (`ARCHITECTURE.md`, `WORKFLOWS.md`,
`README.md`) at **compile time**, so the guide ships inside the binary and an
agent can drive the whole suite from `--llm` alone. Two consequences:

- **A behaviour change ships with its doc change in the same commit.** If you
  change what a command does, update the doc that describes it, together.
- **When the code and a doc disagree, the code wins and the doc is the bug** —
  fix the doc.
