//! CLI integration tests: drive the real `grove` binary in an isolated process
//! with a controlled env (temp HOME/XDG so nothing touches the developer's real
//! grove file), and assert on stdout/stderr/exit + on-disk side effects.
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

/// A `grove` invocation with color off and a temp config home, so `init`/`setup`
/// read the built-in defaults (no grove file) rather than the developer's.
fn grove(config_home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("grove").unwrap();
    cmd.env("NO_COLOR", "1").env("XDG_CONFIG_HOME", config_home);
    cmd
}

#[test]
fn bare_grove_prints_the_suite_overview() {
    let home = tempdir().unwrap();
    grove(home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("git shortcuts + multi-repo tools"))
        .stdout(predicate::str::contains("grove overview"))
        .stdout(predicate::str::contains("grove pull-all"))
        .stdout(predicate::str::contains("grove push-all"))
        .stdout(predicate::str::contains("grove configure"));
}

#[test]
fn example_defines_every_default_alias() {
    let home = tempdir().unwrap();
    grove(home.path())
        .arg("example")
        .assert()
        .success()
        .stdout(predicate::str::contains("lg   = grove overview"))
        .stdout(predicate::str::contains("lgs  = grove sync"))
        .stdout(predicate::str::contains("lgp  = grove pull-all"))
        .stdout(predicate::str::contains("lgpp = grove push-all"))
        .stdout(predicate::str::contains("gs  = grove status"));
}

#[test]
fn init_emits_only_alias_lines_when_piped() {
    // No grove file in the temp config home, so `init` falls back to the built-in
    // defaults — which must now include the multi-repo aliases.
    let home = tempdir().unwrap();
    let out = grove(home.path()).args(["init", "zsh"]).assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("alias lg='grove overview'"), "missing lg alias:\n{stdout}");
    assert!(stdout.contains("alias lt='grove tree'"), "missing lt alias:\n{stdout}");
    // Piped (non-TTY) init must be pure shell code — every non-empty line an alias.
    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        assert!(line.starts_with("alias "), "non-alias line leaked into piped init: {line:?}");
    }
}

#[test]
fn overview_json_on_an_empty_folder_is_valid() {
    let home = tempdir().unwrap();
    let repos = tempdir().unwrap();
    grove(home.path())
        .args(["overview", "--json"])
        .arg(repos.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"repos\": []"))
        .stdout(predicate::str::contains("\"summary\""));
}

#[test]
fn pull_all_json_on_an_empty_folder_is_valid() {
    let home = tempdir().unwrap();
    let repos = tempdir().unwrap();
    grove(home.path())
        .args(["pull-all", "--json"])
        .arg(repos.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"pulled\": []"))
        .stdout(predicate::str::contains("\"overview\""));
}

#[test]
fn configure_sets_gets_and_lists_settings() {
    let home = tempdir().unwrap();
    // A brand-new config home: listing shows the defaults, and unknown keys error.
    grove(home.path())
        .args(["configure"])
        .assert()
        .success()
        .stdout(predicate::str::contains("cache"))
        .stdout(predicate::str::contains("default_dir"));
    grove(home.path())
        .args(["configure", "nonsense", "x"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown setting"));
    // cache validates its value.
    grove(home.path())
        .args(["configure", "cache", "maybe"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("on or off"));
    // A set round-trips through the file and back out via a plain get.
    grove(home.path()).args(["configure", "cache_ttl", "30"]).assert().success();
    grove(home.path())
        .args(["configure", "cache_ttl"])
        .assert()
        .success()
        .stdout(predicate::str::contains("30"));
    let cfg = home.path().join("grove").join("config");
    assert!(cfg.exists(), "settings file not written");
    assert!(fs::read_to_string(&cfg).unwrap().contains("cache_ttl = 30"));
}

#[test]
fn default_dir_fallback_runs_in_the_configured_folder_with_a_note() {
    // A git-irrelevant working dir + a configured (empty) default_dir: `overview`
    // with no argument should fall back to default_dir and say so on stderr.
    let home = tempdir().unwrap();
    let cache = tempdir().unwrap();
    let dest = tempdir().unwrap();
    let cwd = tempdir().unwrap();
    grove(home.path()).args(["configure", "default_dir"]).arg(dest.path()).assert().success();
    grove(home.path())
        .env("XDG_CACHE_HOME", cache.path())
        .current_dir(cwd.path())
        .arg("overview")
        .assert()
        .success()
        .stderr(predicate::str::contains("showing"))
        .stderr(predicate::str::contains(dest.path().to_string_lossy().into_owned()))
        .stdout(predicate::str::contains("No git repositories"));
}

#[test]
fn overview_force_and_default_cache_both_run() {
    // The per-repo cache is on by default; `--force` bypasses it. Both paths must
    // produce a valid dashboard. (Cache stamping is unit-tested in cache.rs — it
    // only marks fully-settled real repos, which an empty temp folder never has.)
    let home = tempdir().unwrap();
    let cache = tempdir().unwrap();
    let repos = tempdir().unwrap();
    for args in [vec!["overview"], vec!["overview", "--force"]] {
        grove(home.path())
            .env("XDG_CACHE_HOME", cache.path())
            .args(&args)
            .arg(repos.path())
            .assert()
            .success()
            .stdout(predicate::str::contains("No git repositories"));
    }
}

#[test]
fn listing_reflects_configured_aliases_and_drops_the_setup_nudge() {
    // A grove file with a renamed overview alias: the listing shows the rename and
    // the "configured" footer, not the "aren't installed yet" nudge.
    let home = tempdir().unwrap();
    let aliases = home.path().join("grove").join("aliases");
    fs::create_dir_all(aliases.parent().unwrap()).unwrap();
    fs::write(&aliases, "gv = grove overview\nlgp = grove pull-all\n").unwrap();
    grove(home.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("(gv)"))
        .stdout(predicate::str::contains("(lgp)"))
        .stdout(predicate::str::contains("Aliases are yours to edit"))
        .stdout(predicate::str::contains("aren't installed yet").not());
}

#[test]
fn overview_on_a_non_directory_fails_with_a_clear_error() {
    let home = tempdir().unwrap();
    grove(home.path())
        .args(["overview", "/no/such/path/hopefully"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a directory"));
}

#[test]
fn tree_json_reports_a_child_directory() {
    let home = tempdir().unwrap();
    let root = tempdir().unwrap();
    fs::create_dir(root.path().join("child")).unwrap();
    grove(home.path())
        .args(["tree", "--json", "-l", "1"])
        .arg(root.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\": \"child\""))
        .stdout(predicate::str::contains("\"type\": \"dir\""));
}

#[test]
fn llm_guide_is_self_contained() {
    let home = tempdir().unwrap();
    grove(home.path())
        .arg("--llm")
        .assert()
        .success()
        .stdout(predicate::str::contains("grove COMMAND REFERENCE"))
        .stdout(predicate::str::contains("grove overview"))
        .stdout(predicate::str::contains("ARCHITECTURE"))
        .stdout(predicate::str::contains("WORKFLOWS"));
}

#[test]
fn setup_writes_the_grove_file_and_rc_block_idempotently() {
    // Isolate HOME and XDG so setup writes into the temp tree, not the developer's.
    let home = tempdir().unwrap();
    let cfg = home.path().join(".config");
    let run = || {
        Command::cargo_bin("grove")
            .unwrap()
            .env("NO_COLOR", "1")
            .env("HOME", home.path())
            .env("XDG_CONFIG_HOME", &cfg)
            .env("SHELL", "/bin/zsh")
            .env_remove("ZDOTDIR") // don't let an inherited ZDOTDIR redirect the rc write
            .args(["setup", "zsh"])
            .assert()
            .success();
    };
    run();
    let aliases = cfg.join("grove").join("aliases");
    let rc = home.path().join(".zshrc");
    assert!(aliases.exists(), "grove file not written");
    assert!(fs::read_to_string(&aliases).unwrap().contains("lg   = grove overview"));
    let marker = "# grove — shell integration";
    let rc_after_first = fs::read_to_string(&rc).unwrap();
    assert_eq!(rc_after_first.matches(marker).count(), 1, "marker missing after first setup");

    // Second run must not add a second managed block.
    run();
    let rc_after_second = fs::read_to_string(&rc).unwrap();
    assert_eq!(rc_after_second.matches(marker).count(), 1, "setup added a duplicate rc block");

    // The default_dir autodetect offer is interactive-only: run non-interactively
    // (no TTY), it must never write a settings file behind the user's back.
    assert!(!cfg.join("grove").join("config").exists(), "setup wrote a settings file non-interactively");
}

/// A `grove setup` in an isolated HOME/XDG tree, as a zsh user.
fn setup_cmd(home: &std::path::Path, cfg: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("grove").unwrap();
    cmd.env("NO_COLOR", "1")
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", cfg)
        .env("SHELL", "/bin/zsh")
        .env_remove("ZDOTDIR")
        .env_remove("GROVE_NO_RELOAD");
    cmd
}

#[test]
fn setup_piped_emits_alias_lines_for_eval_and_reports_on_stderr() {
    // `eval "$(grove setup)"`: stdout must be pure shell code (so the caller's
    // shell can evaluate it and have the aliases live immediately), with the whole
    // human report moved to stderr — the same discipline `grove init` follows.
    let home = tempdir().unwrap();
    let cfg = home.path().join(".config");
    let out = setup_cmd(home.path(), &cfg).args(["setup", "zsh"]).assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(stdout.contains("alias lgs='grove sync'"), "missing alias line:\n{stdout}");
    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        assert!(line.starts_with("alias "), "non-alias line leaked into piped setup: {line:?}");
    }
    assert!(stderr.contains("grove setup"), "report missing from stderr:\n{stderr}");
    assert!(stderr.contains(".zshrc"), "report missing from stderr:\n{stderr}");
}

#[test]
fn setup_reload_without_a_terminal_never_starts_a_shell() {
    // `--reload` asks for the shell handoff, but with no terminal there is nobody
    // to hand off *to* — it must fall back to the printed hint (and, since stdout
    // is a pipe, the eval-able alias lines) rather than exec a shell into a pipe.
    let home = tempdir().unwrap();
    let cfg = home.path().join(".config");
    let out = setup_cmd(home.path(), &cfg).args(["setup", "zsh", "--reload"]).assert().success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        assert!(line.starts_with("alias "), "unexpected output from a non-interactive --reload: {line:?}");
    }
}

#[test]
fn setup_warns_when_grove_is_not_on_path() {
    // The rc line is guarded by `command -v grove`, so a grove the shell can't
    // find makes the whole integration a silent no-op. Setup must say so.
    let home = tempdir().unwrap();
    let cfg = home.path().join(".config");
    let empty = tempdir().unwrap();
    let out = setup_cmd(home.path(), &cfg).env("PATH", empty.path()).args(["setup", "zsh"]).assert().success();
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(stderr.contains("not on your PATH"), "missing PATH warning:\n{stderr}");
}

#[test]
fn setup_force_offers_no_default_dir_even_with_a_repo_folder_present() {
    // Even when $HOME clearly has a repo folder, `--force` (scripts) must stay
    // fully non-interactive and set no default_dir.
    let home = tempdir().unwrap();
    let cfg = home.path().join(".config");
    for r in ["a", "b", "c"] {
        fs::create_dir_all(home.path().join("Developer").join(r).join(".git")).unwrap();
    }
    Command::cargo_bin("grove")
        .unwrap()
        .env("NO_COLOR", "1")
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", &cfg)
        .env("SHELL", "/bin/zsh")
        .env_remove("ZDOTDIR")
        .args(["setup", "zsh", "--force"])
        .assert()
        .success();
    let config = cfg.join("grove").join("config");
    let has_default = config.exists() && fs::read_to_string(&config).unwrap().contains("default_dir");
    assert!(!has_default, "--force setup set a default_dir");
}
