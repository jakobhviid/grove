//! Shell completions for the grove suite, one hand-written script per shell —
//! zsh (`ZSH`), bash (`BASH`) and fish (`FISH`) — each covering every
//! subcommand the same way: the git verbs hand off to the shell's own git
//! completion, the multi-repo/tree verbs complete a folder, and
//! setup/init/configure complete their values. Other shells fall back to
//! clap-generated completions for `grove` itself.
//!
//! The short aliases (gs/ga/gc/gcp/gp/gpp and lg/lgs/lgp/lgpp/lt) complete as the
//! command they stand for. zsh and fish do that on their own (zsh resolves
//! `alias lg='grove overview'`; fish's `alias` wraps the command). bash doesn't,
//! so `grove init bash` registers the aliases with [`BASH_ALIASES`], which loads
//! the bash script on first use and expands the alias before completing.
use clap::CommandFactory;

pub fn emit(shell: clap_complete::Shell) {
    match shell {
        clap_complete::Shell::Zsh => print!("{ZSH}"),
        clap_complete::Shell::Bash => print!("{BASH}"),
        clap_complete::Shell::Fish => print!("{FISH}"),
        other => clap_complete::generate(other, &mut crate::Cli::command(), "grove", &mut std::io::stdout()),
    }
}

const ZSH: &str = r#"#compdef grove
# grove suite completions — one file covers every command (see `grove completions`).
# The short aliases (gs/ga/gc/gcp/gp/gpp and lg/lgs/lgp/lgpp/lt) inherit these
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
  'sync:pull/push the clean, in-sync repos, then overview (alias lgs)'
  'pull-all:fast-forward every repo that is behind, then overview (alias lgp)'
  'push-all:push every repo with unpushed commits, then overview (alias lgpp)'
  'tree:tree view; git repos get a git icon (alias lt)'
  'setup:provision your shell (grove file + rc line)'
  'init:print shell aliases from your grove file (for eval)'
  'example:print a starter grove file'
  'configure:get or set grove settings (cache, cache_ttl, default_dir, depth)'
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
          '--json[emit the planned rewrites as JSON (a dry run unless -y)]' \
          '--depth[levels down to look for repos]:levels:' \
          '--no-color[no colour]' \
          '(-q --quiet)'{-q,--quiet}'[no progress bars or notes]' \
          '1:folder of repos:_files -/' ;;
      overview|sync|pull-all|push-all)
        _arguments \
          '--json[emit JSON instead of the human view]' \
          '(-f --force)'{-f,--force}'[re-fetch every repo, bypassing the cache]' \
          '--depth[levels down to look for repos]:levels:' \
          '--no-color[no colour]' \
          '(-q --quiet)'{-q,--quiet}'[no progress bars or notes]' \
          '1:folder of repos:_files -/' ;;
      configure)
        _arguments \
          '--no-color[no colour]' \
          '1:setting:(cache cache_ttl default_dir depth)' \
          '2:value:->value'
        case "$state" in
          value)
            case "${line[1]}" in
              cache) _values 'cache' on off ;;
              default_dir) _files -/ ;;
            esac ;;
        esac ;;
      tree)
        _arguments \
          '(-a --all)'{-a,--all}'[show hidden entries (dotfiles)]' \
          '(-l --level)'{-l,--level}'[how many levels deep to descend]:levels:' \
          '(-g --git-ignore)'{-g,--git-ignore}'[leave out what git ignores]' \
          '--json[emit JSON instead of the human view]' \
          '--no-color[no colour]' \
          '1:directory:_files -/' ;;
      setup)
        _arguments \
          '--force[reconcile divergent aliases without prompting]' \
          '(--no-reload)--reload[start the fresh shell without asking]' \
          '(--reload)--no-reload[never start a shell]' \
          '--no-color[no colour]' \
          '1:shell:(zsh bash fish)' ;;
      init)
        _arguments \
          '--no-color[no colour]' \
          '1:shell:(zsh bash fish)' ;;
    esac ;;
esac
"#;

const BASH: &str = r#"# grove bash completion — `grove completions bash`; one file covers every command.
# The git verbs hand off to git's own bash completion (when it is installed), the
# multi-repo and tree verbs complete a folder, and setup/init/configure complete
# their values. bash doesn't complete an alias as the command it stands for, so
# `grove init bash` registers the short aliases (lg, gs, …) with _grove_alias.
# Written for bash 3.2 as well (the macOS system bash): compopt is optional.

# Offer the words in $1 that match the current word, each with its own trailing
# space — the registration uses -o nospace, as git's does, so folders stay open.
_grove_reply() {
  local w
  COMPREPLY=()
  for w in $(compgen -W "$1" -- "$cur"); do COMPREPLY+=("$w "); done
}

# Offer folders. bash 4+ quotes the names and marks them itself (compopt);
# bash 3.2 gets the trailing slash added here.
_grove_dirs() {
  local d i
  COMPREPLY=()
  while IFS= read -r d; do COMPREPLY+=("$d"); done < <(compgen -d -- "$cur")
  if type compopt >/dev/null 2>&1; then
    compopt -o filenames 2>/dev/null
  else
    for i in "${!COMPREPLY[@]}"; do COMPREPLY[i]="${COMPREPLY[i]}/"; done
  fi
}

# Offer nothing — not even the file-name fallback, where the shell allows it.
_grove_nothing() {
  COMPREPLY=()
  if type compopt >/dev/null 2>&1; then compopt +o default +o bashdefault 2>/dev/null; fi
}

# Swap the command word of the line being completed for the words given — `git`
# for a passthrough verb, an alias's expansion for a short name — keeping
# COMP_LINE and COMP_POINT in step, since git's completion reads the cursor there.
_grove_set_head() {
  local head=${COMP_WORDS[0]} new="$*"
  local before=${COMP_LINE%%"$head"*} after=${COMP_LINE#*"$head"}
  COMP_LINE=$before$new$after
  COMP_POINT=$((COMP_POINT + ${#new} - ${#head}))
  COMP_WORDS=("$@" "${COMP_WORDS[@]:1}")
  COMP_CWORD=$((COMP_CWORD + $# - 1))
}

_grove_have_git() {
  declare -F __git_wrap__git_main >/dev/null || declare -F _git >/dev/null
}

# Hand `grove status …` to git's own completion, as the same line with `git` in
# grove's place. bash-completion loads git's on demand, so ask it to first;
# without it, bash falls back to file names.
_grove_git() {
  local loader
  if ! _grove_have_git; then
    for loader in _comp_load __load_completion _completion_loader; do
      if declare -F "$loader" >/dev/null; then "$loader" git >/dev/null 2>&1; break; fi
    done
    _grove_have_git || return 0
  fi
  _grove_set_head git
  if declare -F __git_wrap__git_main >/dev/null; then __git_wrap__git_main; else _git; fi
}

_grove() {
  local cur=${COMP_WORDS[COMP_CWORD]} prev=${COMP_WORDS[COMP_CWORD-1]}
  local i sub="" subi=0
  for ((i = 1; i < COMP_CWORD; i++)); do
    case ${COMP_WORDS[i]} in
      -*) ;;
      *) sub=${COMP_WORDS[i]}; subi=$i; break ;;
    esac
  done
  if [[ -z $sub ]]; then
    if [[ $cur == -* ]]; then
      _grove_reply "--llm --help -h --version -V"
    else
      _grove_reply "status add commit pull push ssh overview sync pull-all push-all tree setup init example configure"
    fi
    return 0
  fi

  # The words after the subcommand that aren't flags: configure's key, setup's shell.
  local -a args=()
  for ((i = subi + 1; i < COMP_CWORD; i++)); do
    case ${COMP_WORDS[i]} in -*) ;; *) args+=("${COMP_WORDS[i]}") ;; esac
  done

  case $sub in
    status|add|pull|push) _grove_git ;;
    commit)
      if [[ $cur == -* ]]; then _grove_reply "--all -a --push -p --help -h"; fi ;;
    ssh|overview|sync|pull-all|push-all)
      if [[ $prev == --depth ]]; then _grove_nothing; return 0; fi
      if [[ $cur == -* ]]; then
        case $sub in
          ssh) _grove_reply "--yes -y --json --depth --no-color --quiet -q --help -h" ;;
          *) _grove_reply "--json --force -f --depth --no-color --quiet -q --help -h" ;;
        esac
      elif ((${#args[@]} == 0)); then
        _grove_dirs
      else
        _grove_nothing
      fi ;;
    tree)
      case $prev in -l|--level) _grove_nothing; return 0 ;; esac
      if [[ $cur == -* ]]; then
        _grove_reply "--all -a --level -l --git-ignore -g --json --no-color --help -h"
      elif ((${#args[@]} == 0)); then
        _grove_dirs
      else
        _grove_nothing
      fi ;;
    setup|init)
      if [[ $cur == -* ]]; then
        case $sub in
          setup) _grove_reply "--force --reload --no-reload --no-color --help -h" ;;
          *) _grove_reply "--no-color --help -h" ;;
        esac
      elif ((${#args[@]} == 0)); then
        _grove_reply "zsh bash fish"
      else
        _grove_nothing
      fi ;;
    configure)
      if [[ $cur == -* ]]; then
        _grove_reply "--no-color --help -h"
      elif ((${#args[@]} == 0)); then
        _grove_reply "cache cache_ttl default_dir depth"
      elif ((${#args[@]} == 1)) && [[ ${args[0]} == cache ]]; then
        _grove_reply "on off"
      elif ((${#args[@]} == 1)) && [[ ${args[0]} == default_dir ]]; then
        _grove_dirs
      else
        _grove_nothing
      fi ;;
    *) _grove_nothing ;;
  esac
  return 0
}

# A short alias (`lg` → `grove overview`), completed as the grove command it
# stands for. Anything that isn't an alias for grove completes as grove itself.
_grove_alias() {
  local def
  local -a words
  if def=$(alias -- "${COMP_WORDS[0]}" 2>/dev/null); then
    def=${def#*=}
    eval "def=$def"
    read -r -a words <<<"$def"
    if [[ ${words[0]} == grove ]]; then _grove_set_head "${words[@]}"; fi
  fi
  _grove
}

complete -o bashdefault -o default -o nospace -F _grove grove
"#;

/// What `grove init bash` appends after the alias lines, with `@NAMES@` replaced
/// by the aliases that run grove (and `grove` itself, so it completes without an
/// installed completion file). The completion script is loaded from the binary on
/// the first TAB, so a shell that never completes pays nothing for it.
pub const BASH_ALIASES: &str = r#"_grove_lazy() { declare -F _grove_alias >/dev/null || eval "$(command grove completions bash 2>/dev/null)"; declare -F _grove_alias >/dev/null && _grove_alias; }
complete -o bashdefault -o default -o nospace -F _grove_lazy @NAMES@"#;

const FISH: &str = r#"# grove fish completions — `grove completions fish`; one file covers every command.
# The git verbs hand off to fish's own git completion, the multi-repo and tree
# verbs complete a folder, and setup/init/configure complete their values. The
# short aliases (lg, gs, …) need nothing of their own: fish's `alias` wraps the
# command it stands for, so `lg` completes as `grove overview`.

# The subcommand on the line, once one is typed: the first word after `grove`
# that isn't a flag.
function __grove_subcommand
    set -l tokens (commandline -opc)
    set -e tokens[1]
    for t in $tokens
        if not string match -q -- '-*' $t
            echo $t
            return 0
        end
    end
    return 1
end

function __grove_using
    set -l sub (__grove_subcommand); or return 1
    contains -- $sub $argv
end

# The words after the subcommand that aren't flags: configure's key, setup's shell.
function __grove_args
    set -l tokens (commandline -opc)
    set -e tokens[1]
    set -l past 0
    for t in $tokens
        string match -q -- '-*' $t; and continue
        if test $past -eq 1
            echo $t
        end
        set past 1
    end
end

function __grove_nargs
    test (count (__grove_args)) -eq $argv[1]
end

function __grove_configuring
    set -l args (__grove_args)
    __grove_using configure; and test (count $args) -eq 1; and test "$args[1]" = $argv[1]
end

# Hand `grove status …` to fish's own git completion, as the same line with `git`
# in grove's place.
function __grove_git
    set -l tokens (commandline -opc)
    set -e tokens[1]
    complete -C (string join -- ' ' git (string escape -- $tokens) (commandline -ct))
end

set -l __grove_fleet overview sync pull-all push-all

complete -c grove -f
complete -c grove -n 'not __grove_using status add pull push' -s h -l help -d 'Print help'
complete -c grove -n 'not __grove_subcommand' -s V -l version -d 'Print version'
complete -c grove -n 'not __grove_subcommand' -l llm -d 'Print the full LLM-readable guide'

complete -c grove -n 'not __grove_subcommand' -a status -d 'git status'
complete -c grove -n 'not __grove_subcommand' -a add -d 'git add (stages . by default)'
complete -c grove -n 'not __grove_subcommand' -a commit -d 'git commit -m'
complete -c grove -n 'not __grove_subcommand' -a pull -d 'git pull'
complete -c grove -n 'not __grove_subcommand' -a push -d 'git push'
complete -c grove -n 'not __grove_subcommand' -a ssh -d 'Switch a folder of repos from HTTPS remotes to SSH'
complete -c grove -n 'not __grove_subcommand' -a overview -d 'Multi-repo dashboard for a folder (alias lg)'
complete -c grove -n 'not __grove_subcommand' -a sync -d 'Pull/push the clean, in-sync repos (alias lgs)'
complete -c grove -n 'not __grove_subcommand' -a pull-all -d 'Pull every repo that is behind (alias lgp)'
complete -c grove -n 'not __grove_subcommand' -a push-all -d 'Push every repo with unpushed commits (alias lgpp)'
complete -c grove -n 'not __grove_subcommand' -a tree -d 'Tree view; git repos get a git icon (alias lt)'
complete -c grove -n 'not __grove_subcommand' -a setup -d 'Provision your shell (grove file + rc line)'
complete -c grove -n 'not __grove_subcommand' -a init -d 'Print shell aliases from your grove file'
complete -c grove -n 'not __grove_subcommand' -a example -d 'Print a starter grove file'
complete -c grove -n 'not __grove_subcommand' -a configure -d 'Get or set grove settings'

complete -c grove -n '__grove_using status add pull push' -a '(__grove_git)'

complete -c grove -n '__grove_using commit' -s a -l all -d 'Stage tracked changes first'
complete -c grove -n '__grove_using commit' -s p -l push -d 'Push after a successful commit'

complete -c grove -n "__grove_using $__grove_fleet ssh tree; and __grove_nargs 0" -a '(__fish_complete_directories (commandline -ct))'
complete -c grove -n "__grove_using $__grove_fleet tree" -l json -d 'Emit JSON instead of the human view'
complete -c grove -n "__grove_using $__grove_fleet" -s f -l force -d 'Re-fetch every repo, bypassing the cache'
complete -c grove -n "__grove_using $__grove_fleet ssh" -l depth -x -d 'Levels down to look for repos'
complete -c grove -n "__grove_using $__grove_fleet ssh" -s q -l quiet -d 'No progress bars or notes'
complete -c grove -n "__grove_using $__grove_fleet ssh tree setup init configure" -l no-color -d 'No colour'

complete -c grove -n '__grove_using ssh' -s y -l yes -d 'Apply without the confirmation prompt'
complete -c grove -n '__grove_using ssh' -l json -d 'The planned rewrites as JSON (a dry run unless -y)'

complete -c grove -n '__grove_using tree' -s a -l all -d 'Show dotfiles too'
complete -c grove -n '__grove_using tree' -s l -l level -x -d 'Levels deep to descend'
complete -c grove -n '__grove_using tree' -s g -l git-ignore -d 'Leave out what git ignores'

complete -c grove -n '__grove_using setup init; and __grove_nargs 0' -a 'zsh bash fish'
complete -c grove -n '__grove_using setup' -l force -d 'Reconcile divergent aliases without prompting'
complete -c grove -n '__grove_using setup' -l reload -d 'Start the fresh shell without asking'
complete -c grove -n '__grove_using setup' -l no-reload -d 'Never start a shell'

complete -c grove -n '__grove_using configure; and __grove_nargs 0' -a 'cache cache_ttl default_dir depth'
complete -c grove -n '__grove_configuring cache' -a 'on off'
complete -c grove -n '__grove_configuring default_dir' -a '(__fish_complete_directories (commandline -ct))'
"#;

#[cfg(test)]
mod tests {
    use super::{BASH, FISH, ZSH};
    use clap::CommandFactory;

    /// Every visible subcommand and every flag it declares, as the scripts spell them.
    fn surface() -> Vec<(String, Vec<String>)> {
        let cmd = crate::Cli::command();
        cmd.get_subcommands()
            .filter(|sub| !sub.is_hide_set())
            .map(|sub| {
                let flags = sub
                    .get_arguments()
                    .filter(|a| !a.is_positional())
                    .flat_map(|a| {
                        let long = a.get_long().map(|l| format!("--{l}"));
                        let short = a.get_short().map(|s| format!("-{s}"));
                        long.into_iter().chain(short)
                    })
                    .collect();
                (sub.get_name().to_string(), flags)
            })
            .collect()
    }

    /// The hand-written scripts can't drift from the clap definition unnoticed: a
    /// new subcommand or flag fails here until every shell offers it. (The git
    /// verbs' arguments are git's, and complete through git.)
    #[test]
    fn every_script_offers_every_subcommand_and_flag() {
        for (shell, script) in [("zsh", ZSH), ("bash", BASH), ("fish", FISH)] {
            for (sub, flags) in surface() {
                assert!(script.contains(&sub), "{shell} completion is missing the `{sub}` subcommand");
                for flag in flags {
                    // fish spells flags as `-l name` / `-s x`, the others literally.
                    let spelled = match (shell, flag.strip_prefix("--")) {
                        ("fish", Some(long)) => format!("-l {long}"),
                        ("fish", None) => format!("-s {}", &flag[1..]),
                        _ => flag.clone(),
                    };
                    assert!(script.contains(&spelled), "{shell} completion is missing `{sub} {flag}`");
                }
            }
        }
    }
}
