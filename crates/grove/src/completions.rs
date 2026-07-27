//! Shell completions for the grove suite. zsh gets a single hand-written file
//! (`ZSH`) tagged for every real binary — grove (with its git-verb subcommands)
//! plus lg/lgp/lgpp/lt. The short aliases (gs/ga/gc/gcp/gp/gpp) need no entry of
//! their own: zsh resolves `alias gc='grove commit'` and completes it through
//! grove's `commit` subcommand. The git verbs delegate to zsh's own git
//! completion, so `grove status` (and `gs`) complete exactly like `git status`.
//! Other shells fall back to clap-generated completions for `grove` itself.
use clap::CommandFactory;

pub fn emit(shell: clap_complete::Shell) {
    match shell {
        clap_complete::Shell::Zsh => print!("{ZSH}"),
        other => clap_complete::generate(other, &mut crate::Cli::command(), "grove", &mut std::io::stdout()),
    }
}

const ZSH: &str = r#"#compdef grove lg lgp lgpp lt
# grove suite completions — one file covers every command (see `grove completions`).
# Short aliases (gs/ga/gc/gcp/gp/gpp) inherit these automatically: zsh resolves the
# alias to `grove <verb>` and completes via grove's subcommand handling below.
case "$service" in
  lg|lgp|lgpp)
    _arguments \
      '(- *)--man[print the man page and exit]' \
      '(- *)'{-V,--version}'[print version and exit]' \
      '1:folder of repos:_files -/'
    ;;
  lt)
    _arguments \
      '(-a --all)'{-a,--all}'[show hidden entries (dotfiles)]' \
      '(-l --level)'{-l,--level}'[how many levels deep to descend]:levels:' \
      '(- *)--man[print the man page and exit]' \
      '(- *)'{-V,--version}'[print version and exit]' \
      '1:directory:_files -/'
    ;;
  grove)
    local curcontext="$curcontext" state line
    local -a cmds=(
      'status:git status'
      'add:git add (stages . by default)'
      'commit:git commit -m'
      'pull:git pull'
      'push:git push'
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
          setup|init) _values 'shell' zsh bash fish ;;
        esac ;;
    esac
    ;;
esac
"#;
