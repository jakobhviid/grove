//! Shell completions for the grove suite. zsh gets a single hand-written file
//! (`ZSH`) that covers every subcommand — the git verbs delegate to zsh's own
//! git completion, and the multi-repo/tree verbs complete a folder. The short
//! aliases (gs/ga/gc/gcp/gp/gpp and lg/lgp/lgpp/lt) need no entry of their own:
//! zsh resolves e.g. `alias lg='grove overview'` and completes it through
//! grove's `overview` subcommand. Other shells fall back to clap-generated
//! completions for `grove` itself.
use clap::CommandFactory;

pub fn emit(shell: clap_complete::Shell) {
    match shell {
        clap_complete::Shell::Zsh => print!("{ZSH}"),
        other => clap_complete::generate(other, &mut crate::Cli::command(), "grove", &mut std::io::stdout()),
    }
}

const ZSH: &str = r#"#compdef grove
# grove suite completions — one file covers every command (see `grove completions`).
# The short aliases (gs/ga/gc/gcp/gp/gpp and lg/lgp/lgpp/lt) inherit these
# automatically: zsh resolves e.g. `alias lg='grove overview'` and completes it via
# grove's `overview` subcommand handling below. The git verbs delegate to zsh's own
# git completion, so `grove status` (and `gs`) complete exactly like `git status`.
local curcontext="$curcontext" state line
local -a cmds=(
  'status:git status'
  'add:git add (stages . by default)'
  'commit:git commit -m'
  'pull:git pull'
  'push:git push'
  'ssh:switch a folder of repos from HTTPS remotes to SSH'
  'overview:multi-repo dashboard for a folder (alias lg)'
  'sync:pull/push the clean, in-sync repos, then overview (alias lgp)'
  'push-all:push every repo with unpushed commits (alias lgpp)'
  'tree:tree view; git repos get a git icon (alias lt)'
  'setup:provision your shell (grove file + rc line)'
  'init:print shell aliases from your grove file (for eval)'
  'example:print a starter grove file'
)
_arguments -C \
  '--llm[print the full LLM-readable guide and exit]' \
  '(- *)'{-h,--help}'[show help]' \
  '(- *)'{-V,--version}'[print version and exit]' \
  '1:command:->cmd' \
  '*::arg:->arg'
case "$state" in
  cmd) _describe 'grove command' cmds ;;
  arg)
    case "${line[1]}" in
      status) service=git-status _git ;;
      add)    service=git-add    _git ;;
      pull)   service=git-pull   _git ;;
      push)   service=git-push   _git ;;
      commit)
        _arguments \
          '(-a --all)'{-a,--all}'[stage tracked changes first]' \
          '(-p --push)'{-p,--push}'[push after a successful commit]' \
          '*:message:' ;;
      ssh)
        _arguments \
          '(-y --yes)'{-y,--yes}'[apply without the confirmation prompt]' \
          '1:folder of repos:_files -/' ;;
      overview|sync|push-all)
        _arguments \
          '--json[emit JSON instead of the human view]' \
          '1:folder of repos:_files -/' ;;
      tree)
        _arguments \
          '(-a --all)'{-a,--all}'[show hidden entries (dotfiles)]' \
          '(-l --level)'{-l,--level}'[how many levels deep to descend]:levels:' \
          '--json[emit JSON instead of the human view]' \
          '1:directory:_files -/' ;;
      setup|init) _values 'shell' zsh bash fish ;;
    esac ;;
esac
"#;
