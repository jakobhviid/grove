//! The grove file: `~/.config/grove/aliases`, shell-agnostic `name = command`
//! lines. `grove init <shell>` translates it into that shell's alias syntax. If
//! no file exists, a built-in default set (gs/ga/gc/gcp/gp/gpp → the git verbs)
//! is emitted, so it works out of the box.
//!
//! `grove setup` is the provisioner on top: it writes that file, adds the load
//! line to your rc, and then — since no process can add aliases to the shell that
//! launched it — offers to hand the terminal to a fresh shell that re-reads the
//! rc, or emits the alias lines for `eval "$(grove setup)"` when stdout is piped.
//! See [`activate`].
use clap::ValueEnum;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, ValueEnum)]
pub enum Shell {
    Zsh,
    Bash,
    Fish,
}

/// Built-in defaults emitted when there's no grove file yet: short names for the
/// git verbs and the multi-repo/tree tools, mapped to `grove` subcommands. These
/// are aliases, not binaries, so nothing short lands on PATH to collide with
/// other tools (notably `lg` vs lazygit) — and each only shadows at your
/// interactive prompt, never in scripts. Rename any that clash on your system
/// (e.g. `gc` or `lg`) by editing the grove file; that's the whole point.
const DEFAULTS: &[(&str, &str)] = &[
    ("gs", "grove status"),
    ("ga", "grove add"),
    ("gc", "grove commit"),
    ("gcp", "grove commit --all --push"),
    ("gp", "grove pull"),
    ("gpp", "grove push"),
    ("lg", "grove overview"),
    ("lgs", "grove sync"),
    ("lgp", "grove pull-all"),
    ("lgpp", "grove push-all"),
    ("lt", "grove tree"),
];

/// Read an environment variable as a path, treating unset **and empty** the
/// same (an empty `XDG_CONFIG_HOME`/`ZDOTDIR` must not become a relative path).
/// Shared with `settings`/`cache`, which key off the same env conventions.
pub(crate) fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// `~/.config/grove` (honoring `XDG_CONFIG_HOME`) — the directory holding both the
/// grove file (`aliases`) and the settings file (`config`).
pub(crate) fn config_dir() -> PathBuf {
    let base = env_path("XDG_CONFIG_HOME")
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"));
    base.join("grove")
}

fn config_path() -> PathBuf {
    config_dir().join("aliases")
}

fn aliases() -> Vec<(String, String)> {
    match std::fs::read_to_string(config_path()) {
        Ok(text) => parse(&text),
        Err(_) => DEFAULTS.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
    }
}

/// The aliases the user has actually configured, or `None` when there's no grove
/// file yet. Unlike [`aliases`], this does **not** fall back to the built-in
/// defaults: a missing file means the short aliases aren't active in any shell,
/// which is exactly what the listings/hints need to know to nudge `grove setup`.
fn configured_aliases() -> Option<Vec<(String, String)>> {
    std::fs::read_to_string(config_path()).ok().map(|t| parse(&t))
}

/// Whether a grove file exists — i.e. the short aliases have been provisioned.
pub(crate) fn is_configured() -> bool {
    config_path().exists()
}

/// The alias name the user bound to `command` (e.g. `grove push-all`) in their
/// grove file, or `None` if there's no file or nothing maps to it. Honors renames
/// (`gv = grove overview` resolves `grove overview` to `gv`), so the listings and
/// hints always show the name the user actually types.
pub(crate) fn alias_for(command: &str) -> Option<String> {
    configured_aliases()?.into_iter().find(|(_, c)| c == command).map(|(n, _)| n)
}

/// Default aliases whose *name* is absent from an existing grove file — the set
/// `setup` appends when topping up. A name already present (even remapped to a
/// different command) is left alone, so we never rewrite a line you've edited.
fn missing_defaults(text: &str) -> Vec<(&'static str, &'static str)> {
    let have: std::collections::HashSet<String> = parse(text).into_iter().map(|(n, _)| n).collect();
    DEFAULTS.iter().copied().filter(|(n, _)| !have.contains(*n)).collect()
}

/// Default aliases whose name is present in the file but bound to a *different*
/// command than grove's default — candidates for `setup` to reconcile. Returns
/// `(name, current command, default command)`.
fn divergent_defaults(text: &str) -> Vec<(&'static str, String, &'static str)> {
    let have = parse(text);
    DEFAULTS
        .iter()
        .filter_map(|(name, default)| {
            let cur = have.iter().find(|(n, _)| n == name)?;
            (cur.1 != *default).then(|| (*name, cur.1.clone(), *default))
        })
        .collect()
}

/// Rewrite the single `name = …` line to grove's default command, preserving the
/// original left-hand side (name + spacing) so the file's formatting survives.
fn override_alias(text: &str, name: &str, default: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        match line.split_once('=') {
            Some((lhs, _)) if lhs.trim() == name => out.push(format!("{lhs}= {default}")),
            _ => out.push(line.to_string()),
        }
    }
    let mut joined = out.join("\n");
    if text.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

/// True when `setup`'s stdout is **not** a terminal — the `eval "$(grove setup)"`
/// form. Then stdout carries only shell code (the same discipline [`init`]
/// follows) and every human line moves to stderr, so the caller's shell can
/// evaluate the result while the person still reads the report.
fn eval_mode() -> bool {
    !io::stdout().is_terminal()
}

/// One line of `setup`'s human report: stdout normally, stderr in [`eval_mode`].
/// (`ui::paint` keys colour off stdout, so the eval-mode report comes out plain —
/// the right call anyway when stdout is being captured by a shell.)
fn say(line: &str) {
    if eval_mode() {
        eprintln!("{line}");
    } else {
        println!("{line}");
    }
}

/// A question (no newline, flushed) on the same stream [`say`] writes to, so the
/// answer is typed on the same line as the prompt.
fn prompt(text: &str) {
    use std::io::Write;
    if eval_mode() {
        eprint!("{text}");
        let _ = io::stderr().flush();
    } else {
        print!("{text}");
        let _ = io::stdout().flush();
    }
}

/// Whether there's a human to ask: stdin to read the answer from, and a terminal
/// on whichever stream we'd ask on (stdout normally, stderr when stdout carries
/// shell code). Piped/scripted runs are never prompted — there `--force` and
/// `--no-reload` are the controls.
fn interactive() -> bool {
    io::stdin().is_terminal() && (!eval_mode() || io::stderr().is_terminal())
}

/// Ask a yes/no question on the terminal; `default_yes` is what a bare Enter
/// means. Returns `false` without prompting when there's no human ([`interactive`]),
/// and on EOF/Ctrl-D — the safe answer for anything that edits files or replaces
/// the process.
fn confirm(question: &str, default_yes: bool) -> bool {
    if !interactive() {
        return false;
    }
    prompt(&format!("  {question} {} ", if default_yes { "[Y/n]" } else { "[y/N]" }));
    let mut answer = String::new();
    match io::stdin().read_line(&mut answer) {
        Ok(0) | Err(_) => false,
        Ok(_) => match answer.trim().to_ascii_lowercase().as_str() {
            "" => default_yes,
            a => matches!(a, "y" | "yes"),
        },
    }
}

/// Parse `name = command` lines; ignore blanks and `#` comments. Shared with the
/// settings file (`settings.rs`), which uses the same `key = value` shape.
pub(crate) fn parse(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (name, cmd) = line.split_once('=')?;
            let (name, cmd) = (name.trim(), cmd.trim());
            (!name.is_empty() && !cmd.is_empty()).then(|| (name.to_string(), cmd.to_string()))
        })
        .collect()
}

fn name_of(shell: Shell) -> &'static str {
    match shell {
        Shell::Zsh => "zsh",
        Shell::Bash => "bash",
        Shell::Fish => "fish",
    }
}

fn alias_line(shell: Shell, name: &str, cmd: &str) -> String {
    match shell {
        Shell::Fish => format!("alias {name} '{cmd}'"),
        Shell::Zsh | Shell::Bash => format!("alias {name}='{cmd}'"),
    }
}

fn activate_hint(shell: Shell) -> String {
    match shell {
        Shell::Fish => "grove init fish | source".to_string(),
        s => format!("eval \"$(grove init {})\"", name_of(s)),
    }
}

pub fn init(shell: Shell) {
    let items = aliases();
    let lines: Vec<String> = items.iter().map(|(n, c)| alias_line(shell, n, c)).collect();

    // Being eval'd / piped / redirected (not a TTY): emit ONLY shell code, and
    // nothing else — this runs on every shell startup, so it must stay silent
    // and pure. This is the path `eval "$(grove init zsh)"` takes.
    if !io::stdout().is_terminal() {
        for l in &lines {
            println!("{l}");
        }
        return;
    }

    // A human ran it in a terminal: explain what it does rather than dumping raw
    // `alias` lines with no context.
    use grove_core::ui::paint;
    let path = config_path();
    let source = if path.exists() {
        path.display().to_string()
    } else {
        "built-in defaults — no grove file yet (run `grove example > ~/.config/grove/aliases`)".to_string()
    };

    println!("{} would set up {} {} for {}:", paint("1;32", "grove init"), paint("1", &items.len().to_string()), if items.len() == 1 { "alias" } else { "aliases" }, paint("1", name_of(shell)));
    println!("{}", paint("90", &format!("  source: {source}")));
    println!();
    if items.is_empty() {
        println!("  (none defined)");
    } else {
        let w = items.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
        for (n, c) in &items {
            println!("  {}  →  {}", paint("36", &format!("{n:<w$}")), c);
        }
    }
    println!();
    println!("It doesn't change anything on its own — it prints shell code to evaluate.");
    println!("To activate, add this to your shell startup file:");
    println!("  {}", paint("1", &activate_hint(shell)));
    println!("{}", paint("90", &format!("Or let grove wire it up for you (writes the grove file + this line): grove setup {}", name_of(shell))));
    println!("{}", paint("90", "(when run non-interactively, e.g. via eval, it prints only the alias lines.)"));
}

/// Marker that identifies grove's managed block in a shell rc file. `setup`
/// checks for it to stay idempotent — re-running never adds a second block.
const MARKER: &str = "# grove — shell integration";

/// Set in the environment of the shell `setup` hands off to, so a `grove setup`
/// run *inside* that shell never offers to nest another one.
const RELOADED: &str = "GROVE_RELOADED";

/// What `setup` should do about the shell you ran it from, once the files are
/// written. See [`activate`] for why this is a choice at all.
#[derive(Clone, Copy, PartialEq)]
pub enum Reload {
    /// The default: offer the handoff, but only on a terminal and only when
    /// something actually changed.
    Ask,
    /// `--reload`: hand off without asking (still terminal-gated).
    Always,
    /// `--no-reload`, `GROVE_NO_RELOAD`, or `--force`: never — just print the
    /// line to run.
    Never,
}

impl Reload {
    /// Resolve the two flags plus the `GROVE_NO_RELOAD` escape hatch. An explicit
    /// flag always wins over the environment; `--force` (scripted, unattended)
    /// implies "don't touch my shell" unless `--reload` says otherwise.
    pub fn from_flags(reload: bool, no_reload: bool, force: bool) -> Self {
        if reload {
            Reload::Always
        } else if no_reload || force || std::env::var_os("GROVE_NO_RELOAD").is_some_and(|v| !v.is_empty()) {
            Reload::Never
        } else {
            Reload::Ask
        }
    }
}

/// `grove setup [shell]`: the one-stop provisioner. Writes the grove file if
/// it's missing and appends an idempotent, marker-delimited block to the shell's
/// rc that loads the aliases via `grove init` on every startup. Re-running is a
/// no-op once the block is present. `init` stays the pure emitter this block
/// calls; `setup` is the only thing that edits your files.
pub fn setup(shell: Option<Shell>, force: bool, reload: Reload) -> anyhow::Result<()> {
    use grove_core::ui::paint;
    let shell = shell
        .or_else(detect_shell)
        .ok_or_else(|| anyhow::anyhow!("couldn't detect your shell from $SHELL — run `grove setup zsh` (or bash/fish)"))?;
    let sh = name_of(shell);

    // 1) Materialize the editable grove file, then reconcile it. A brand-new file gets
    //    the full annotated template. An existing one is (a) topped up with any default
    //    whose *name* is missing (so a file predating a default — e.g. `gp` — self-heals),
    //    and (b) offered a fix for any default whose name is present but bound to a
    //    different command. We never silently rewrite an edited line: each divergence is
    //    confirmed interactively, or applied wholesale with `--force` for scripts.
    let cfg = config_path();
    let mut added: Vec<&'static str> = Vec::new();
    let mut overridden: Vec<&'static str> = Vec::new();
    let mut kept: Vec<&'static str> = Vec::new();
    let file_status = if cfg.exists() {
        let mut text = std::fs::read_to_string(&cfg)?;
        let mut changed = false;

        // (b) Reconcile divergent aliases first, so top-up sees the final name set.
        for (name, current, default) in divergent_defaults(&text) {
            let apply = force
                || confirm(
                    &format!("{} is `{}` — reset to grove's default `{}`?", paint("36", name), paint("1", &current), paint("1", default)),
                    false,
                );
            if apply {
                text = override_alias(&text, name, default);
                overridden.push(name);
                changed = true;
            } else {
                kept.push(name);
            }
        }

        // (a) Top up the aliases this file never defined.
        let missing = missing_defaults(&text);
        if !missing.is_empty() {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str("\n# Added by `grove setup` — default git-verb aliases this file was missing:\n");
            let w = missing.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
            for (n, c) in &missing {
                text.push_str(&format!("{n:<w$} = {c}\n"));
                added.push(n);
            }
            changed = true;
        }

        if changed {
            std::fs::write(&cfg, text)?;
            "updated"
        } else {
            "exists"
        }
    } else {
        if let Some(dir) = cfg.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&cfg, EXAMPLE)?;
        "created"
    };

    // 2) Append the managed block to the rc file, unless our marker is already there.
    let rc = rc_path(shell)?;
    let existing = std::fs::read_to_string(&rc).unwrap_or_default();
    let rc_status = if existing.contains(MARKER) {
        "present"
    } else {
        if let Some(dir) = rc.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut content = existing;
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push('\n');
        content.push_str(&rc_block(shell));
        std::fs::write(&rc, content)?;
        "added"
    };

    let changed = file_status != "exists" || rc_status == "added";
    say(&format!("{} — {}", paint("1;32", "grove setup"), paint("1", sh)));
    say("");
    say(&format!("  {} {} {}", paint("36", "grove file"), paint("1", &format!("{file_status:<8}")), cfg.display()));
    let note = |label: &str, names: &[&str]| {
        if !names.is_empty() {
            say(&format!("  {} {} {label}: {}", paint("36", &format!("{:<10}", "")), paint("1", &format!("{:<8}", "")), paint("1", &names.join(" "))));
        }
    };
    note("topped up", &added);
    note("reset to default", &overridden);
    note("left as-is", &kept);
    let rc_desc = if rc_status == "present" {
        "already configured — no change".to_string()
    } else {
        format!("added the `grove init {sh}` line")
    };
    say(&format!("  {} {} {}", paint("36", &format!("{:<10}", rc_name(shell))), paint("1", &format!("{rc_status:<8}")), rc_desc));
    if changed {
        let names: Vec<String> = aliases().into_iter().map(|(n, _)| n).collect();
        say(&format!("  {} {} {}", paint("36", &format!("{:<10}", "aliases")), paint("1", &format!("{:<8}", "")), paint("90", &names.join(" "))));
    }

    // 3) The rc line is guarded by `command -v grove`, so an unreachable binary
    //    makes the whole integration a silent no-op. Say so now rather than let it
    //    surface later as "the aliases don't work".
    check_path();

    // 4) Offer to autodetect a `default_dir` (the folder the multi-repo verbs fall
    //    back to). The fallback is inert without one, and most people have a single
    //    repo folder we can find. Interactive only, and never when a default is set.
    suggest_default_dir(force);

    // 5) Leave the shell you ran this from actually able to use the aliases. Last,
    //    because a handoff replaces this process and nothing after it would run.
    activate(shell, reload, changed, &rc);
    Ok(())
}

/// Step 5 of [`setup`]: make the aliases usable in the session you just ran it
/// from, not only in the next one. A process can never modify its parent shell,
/// so there are exactly two honest ways to do it, and we take whichever applies:
///
/// - **`eval "$(grove setup)"`** — stdout isn't a terminal, so we print the alias
///   lines and the caller's own shell evaluates them. Nothing is nested, nothing
///   is replaced; this is the form for dotfiles and provisioning scripts.
/// - **a handoff** — we `exec` a fresh interactive shell in place of grove, so it
///   re-reads the rc we just wrote and the aliases are live. That shell is *new*,
///   nested in the one you started from (`exit` returns to it), which is why we
///   ask first and say so plainly.
///
/// Everything else — no terminal, `--no-reload`, a shell that isn't the one you
/// are running, or already inside a handoff — falls back to printing the one line
/// to run. We never silently replace the caller's shell.
fn activate(shell: Shell, mode: Reload, changed: bool, rc: &Path) {
    use grove_core::ui::paint;
    say("");
    if eval_mode() {
        // stdout is a pipe: emit what `grove init` would, so `eval "$(grove setup)"`
        // provisions *and* activates in one step. Piped-but-not-eval'd (a redirect
        // to a file) is harmless — the lines just land there.
        for (name, cmd) in aliases() {
            println!("{}", alias_line(shell, &name, &cmd));
        }
        say(&paint("90", "Alias lines written to stdout — run via `eval \"$(grove setup)\"` and they are live in this shell now."));
        return;
    }
    let hint = || say(&format!("Reload your shell to activate:  {}", paint("1", &reload_hint(shell, rc))));
    if !changed && mode != Reload::Always {
        say(&paint("90", "Already set up — open a new shell if you haven't reloaded, or run `grove setup --reload`."));
        return;
    }
    // Only ever hand off into the shell the user actually runs, on a real
    // terminal, and never a second level deep.
    if mode == Reload::Never || !interactive() || !is_current_shell(shell) || std::env::var_os(RELOADED).is_some() {
        hint();
        return;
    }
    if mode == Reload::Ask && !confirm("Start a fresh shell now, with the aliases loaded?", true) {
        hint();
        return;
    }
    say(&format!("{} starting a new {} with the aliases loaded", paint("1;32", "→"), name_of(shell)));
    say(&paint("90", "(it is nested in the shell you started from — `exit` returns there)"));
    let e = exec_shell(shell); // only returns if the exec failed
    grove_core::ui::err(&format!("couldn't start {}: {e}", name_of(shell)));
    hint();
}

/// Replace this process with a fresh interactive shell, which re-reads the rc we
/// just wrote — the only way a child process can leave the terminal in a shell
/// that has the new aliases. Interactive but **not** a login shell: the login
/// files already ran for the session we inherit our environment from, and re-running
/// them would duplicate `PATH` entries; the rc (`.zshrc`/`.bashrc`/`config.fish`)
/// is where our block lives. Returns only if the exec failed.
fn exec_shell(shell: Shell) -> io::Error {
    let mut cmd = std::process::Command::new(shell_program(shell));
    cmd.arg("-i").env(RELOADED, "1");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.exec()
    }
    #[cfg(not(unix))]
    {
        io::Error::other(format!("{cmd:?}: shell handoff needs Unix exec"))
    }
}

/// The shell binary to hand off to: `$SHELL` (so a custom build like
/// `/opt/homebrew/bin/zsh` is honored), falling back to the bare name for a PATH
/// lookup. Only called once [`is_current_shell`] agrees the two are the same shell.
fn shell_program(shell: Shell) -> PathBuf {
    env_path("SHELL").filter(|_| is_current_shell(shell)).unwrap_or_else(|| PathBuf::from(name_of(shell)))
}

/// Whether the shell we just provisioned is the one the user is actually running.
/// `grove setup bash` from a zsh session prints the hint instead of dropping you
/// into a shell you didn't ask for.
fn is_current_shell(shell: Shell) -> bool {
    detect_shell().is_some_and(|s| name_of(s) == name_of(shell))
}

/// Where the shell will find `grove`, if anywhere — an executable named `grove`
/// on `PATH`.
fn grove_on_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|dir| dir.join("grove")).find(|p| is_executable(p))
}

fn is_executable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}

/// Warn when the shell integration can't reach `grove` at all (the rc line is
/// guarded by `command -v grove`, so an off-PATH binary silently does nothing),
/// or when it would reach a *different* grove than the one you just ran — the
/// usual "I set up a dev build and the aliases point elsewhere" surprise.
fn check_path() {
    use grove_core::ui::paint;
    let me = std::env::current_exe().ok();
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    match grove_on_path() {
        None => {
            say("");
            say(&paint("1;33", "! `grove` is not on your PATH — the line we added to your rc is guarded, so it will do nothing."));
            if let Some(dir) = me.as_deref().and_then(Path::parent) {
                say(&paint("90", &format!("  add its folder, e.g.:  export PATH=\"{}:$PATH\"", dir.display())));
            }
        }
        Some(found) => {
            if me.as_deref().is_some_and(|m| canon(m) != canon(&found)) {
                say(&paint("90", &format!("  note: your shell will run {} (you ran {})", found.display(), me.unwrap_or_default().display())));
            }
        }
    }
}

/// After provisioning aliases, offer to set `default_dir` — the folder the
/// multi-repo verbs fall back to. We autodetect the folders full of git repos
/// under `$HOME` and present them as a numbered menu to pick from (or type your
/// own path, or skip). Skipped when a default is already set, under `--force`,
/// and whenever stdin/stdout isn't a TTY — so scripts and unattended provisioning
/// are never prompted or silently reconfigured.
fn suggest_default_dir(force: bool) {
    use grove_core::ui::paint;
    if force || crate::settings::default_dir_configured() || !interactive() {
        return;
    }
    let candidates = crate::settings::detect_candidates();
    if candidates.is_empty() {
        return;
    }

    say("");
    say(&paint("1", "Pick a default folder for the multi-repo commands (lg/lgs/lgp/lgpp):"));
    let w = candidates.len().to_string().len();
    for (i, (path, repos)) in candidates.iter().enumerate() {
        let count = format!("{repos} git {}", if *repos == 1 { "repo" } else { "repos" });
        // Show `~/Developer`, not the noisy `/Users/you/Developer`.
        say(&format!("  {} {}  {}", paint("36", &format!("{:>w$})", i + 1)), crate::settings::tildify(path), paint("90", &count)));
    }
    prompt(&format!("  choose [1-{}], type a path, or Enter to skip: ", candidates.len()));

    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err() {
        return;
    }
    let choice = line.trim();
    // A number picks from the menu; a non-empty non-number is treated as a path
    // the user typed (so they can point at a folder we didn't detect); empty skips.
    let picked: Option<PathBuf> = if choice.is_empty() {
        None
    } else if let Ok(n) = choice.parse::<usize>() {
        candidates.get(n.wrapping_sub(1)).map(|(p, _)| p.clone())
    } else {
        let typed = crate::settings::expand_tilde(choice);
        typed.is_dir().then_some(typed)
    };

    match picked {
        // Store the tilde form (`~/Developer`) — readable in the settings file, and
        // `settings::load` expands it back on use.
        Some(dir) => {
            let stored = crate::settings::tildify(&dir);
            match crate::settings::put("default_dir", &stored) {
                Ok(()) => say(&format!("  {} default_dir = {}", paint("1;32", "set"), paint("1", &stored))),
                Err(e) => grove_core::ui::err(&format!("{e:#}")),
            }
        }
        None => say(&format!("  {}", paint("90", "skipped — set one later with `grove configure default_dir <path>`"))),
    }
}

/// Guess the shell from `$SHELL` (used when `grove setup` is run without an arg).
fn detect_shell() -> Option<Shell> {
    let sh = std::env::var("SHELL").ok()?;
    let base = Path::new(&sh).file_name()?.to_string_lossy().into_owned();
    if base.contains("zsh") {
        Some(Shell::Zsh)
    } else if base.contains("fish") {
        Some(Shell::Fish)
    } else if base.contains("bash") {
        Some(Shell::Bash)
    } else {
        None
    }
}

/// The startup file `setup` writes to for each shell.
fn rc_path(shell: Shell) -> anyhow::Result<PathBuf> {
    let home = env_path("HOME").ok_or_else(|| anyhow::anyhow!("HOME is not set"))?;
    Ok(match shell {
        Shell::Zsh => env_path("ZDOTDIR").unwrap_or(home).join(".zshrc"),
        Shell::Bash => home.join(".bashrc"),
        Shell::Fish => env_path("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config")).join("fish").join("config.fish"),
    })
}

fn rc_name(shell: Shell) -> &'static str {
    match shell {
        Shell::Zsh => ".zshrc",
        Shell::Bash => ".bashrc",
        Shell::Fish => "config.fish",
    }
}

/// The marker comment + a guarded load line (a no-op if `grove` isn't on PATH,
/// so removing grove later doesn't spam errors at shell startup).
fn rc_block(shell: Shell) -> String {
    let load = match shell {
        Shell::Fish => "command -v grove >/dev/null 2>&1; and grove init fish | source".to_string(),
        s => format!("command -v grove >/dev/null 2>&1 && eval \"$(grove init {})\"", name_of(s)),
    };
    format!("{MARKER} (managed by `grove setup`; safe to delete this block)\n{load}\n")
}

fn reload_hint(shell: Shell, rc: &Path) -> String {
    match shell {
        Shell::Fish => "exec fish".to_string(),
        _ => format!("source {}", rc.display()),
    }
}

pub fn print_example() {
    print!("{EXAMPLE}");
}

const EXAMPLE: &str = "\
# ~/.config/grove/aliases
# Shell-agnostic aliases. `grove init <shell>` turns these into aliases for your
# shell. Left = the name you type, right = the command it runs.
#
# Short names for the everyday git verbs. They're aliases (not binaries), so they
# only apply at your interactive prompt — rename any that clash with another tool
# on your system (e.g. change `gc` if you already use it for something else).
gs  = grove status
ga  = grove add
gc  = grove commit
gcp = grove commit --all --push
gp  = grove pull
gpp = grove push

# The multi-repo / tree tools. `lgs` (sync) is the everyday one — it pulls the
# behind repos and pushes the ahead ones; `lgp`/`lgpp` are the one-direction
# escape hatches. `lg` in particular clashes with lazygit — rename it (or any of
# these) if you already use that name for something else.
lg   = grove overview
lgs  = grove sync
lgp  = grove pull-all
lgpp = grove push-all
lt   = grove tree

# Add your own, too:
# co = grove commit
# st = grove status
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_defaults_reports_absent_names() {
        // An old file that predates most defaults (only gs + gcp present): setup
        // tops up every absent default — including the multi-repo aliases a file
        // written before the 2.0 collapse never had — but leaves the two present
        // names, even remapped ones, alone.
        let old = "gs  = gst\ngcp = gc --all --push\n";
        let missing: Vec<&str> = missing_defaults(old).into_iter().map(|(n, _)| n).collect();
        assert_eq!(missing, vec!["ga", "gc", "gp", "gpp", "lg", "lgs", "lgp", "lgpp", "lt"]);
    }

    #[test]
    fn missing_defaults_empty_when_all_present() {
        assert!(missing_defaults(EXAMPLE).is_empty());
    }

    #[test]
    fn remapped_name_is_not_reported_missing() {
        // `gp` present but remapped to something else is still "present" — we must
        // not rewrite a line the user has edited.
        assert!(!missing_defaults("gp = git pull --rebase\n").iter().any(|(n, _)| *n == "gp"));
    }

    #[test]
    fn divergent_defaults_flags_only_remapped_names() {
        // gs is remapped (divergent); gp matches the default; ga is absent (not divergent).
        let text = "gs = gst\ngp = grove pull\n";
        let div = divergent_defaults(text);
        assert_eq!(div.len(), 1);
        assert_eq!(div[0], ("gs", "gst".to_string(), "grove status"));
    }

    #[test]
    fn override_alias_rewrites_rhs_and_keeps_formatting() {
        let text = "# a comment\ngs  = gst\ngcp = gc --all --push\n";
        let out = override_alias(text, "gs", "grove status");
        assert_eq!(out, "# a comment\ngs  = grove status\ngcp = gc --all --push\n");
        // Untouched names and comments are preserved verbatim.
        assert!(out.contains("# a comment"));
        assert!(out.contains("gcp = gc --all --push"));
    }

    #[test]
    fn override_alias_preserves_trailing_newline_absence() {
        assert_eq!(override_alias("gp = git pull", "gp", "grove pull"), "gp = grove pull");
    }

    #[test]
    fn example_template_defines_every_default() {
        // The starter template and the built-in defaults must not drift apart.
        assert!(missing_defaults(EXAMPLE).is_empty(), "EXAMPLE is missing a default alias");
    }
}
