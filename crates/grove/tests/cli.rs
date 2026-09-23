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
fn default_dir_fallback_applies_inside_a_repo_too() {
    // Inside a repo there is no fleet to list, so `overview` with no argument shows
    // the configured default_dir instead of an empty table.
    let home = tempdir().unwrap();
    let cache = tempdir().unwrap();
    let dest = tempdir().unwrap();
    let cwd = tempdir().unwrap();
    Command::new("git").arg("init").current_dir(cwd.path()).assert().success();
    grove(home.path()).args(["configure", "default_dir"]).arg(dest.path()).assert().success();
    grove(home.path())
        .env("XDG_CACHE_HOME", cache.path())
        .current_dir(cwd.path())
        .arg("overview")
        .assert()
        .success()
        .stderr(predicate::str::contains("showing"))
        .stderr(predicate::str::contains(dest.path().to_string_lossy().into_owned()));
}

/// A repo whose fetch is refused, built without touching the network: a stand-in
/// `ssh` that answers the way a real host does when your key isn't allowed. Returns
/// the folder holding the fleet.
#[cfg(unix)]
fn fleet_with_a_denied_repo(dir: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let ssh = dir.join("refusing-ssh");
    fs::write(&ssh, "#!/bin/sh\necho 'git@example.invalid: Permission denied (publickey).' >&2\nexit 255\n").unwrap();
    fs::set_permissions(&ssh, fs::Permissions::from_mode(0o755)).unwrap();

    let repo = dir.join("locked");
    fs::create_dir(&repo).unwrap();
    let git = |args: &[&str]| {
        Command::new("git").current_dir(&repo).args(args).assert().success();
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "t@example.invalid"]);
    git(&["config", "user.name", "t"]);
    git(&["commit", "-q", "--allow-empty", "-m", "one"]);
    git(&["remote", "add", "origin", "git@example.invalid:owner/locked.git"]);
    git(&["config", "core.sshCommand", ssh.to_str().unwrap()]);
}

#[cfg(unix)]
#[test]
fn a_refused_fetch_is_marked_denied_in_the_table_the_legend_and_the_json() {
    // A refused fetch reaches the reader four ways: the row carries a ⊘, the legend
    // spells it out, the roll-up counts it, and one line quotes git verbatim.
    let home = tempdir().unwrap();
    let cache = tempdir().unwrap();
    let fleet = tempdir().unwrap();
    fleet_with_a_denied_repo(fleet.path());

    grove(home.path())
        .env("XDG_CACHE_HOME", cache.path())
        .args(["overview"])
        .arg(fleet.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("⊘ locked"))
        .stdout(predicate::str::contains("legend: ⊘ no access to origin"))
        .stdout(predicate::str::contains("⊘ 1 denied"))
        .stdout(predicate::str::contains("Permission denied (publickey)"));

    // Same knowledge on the machine surface, so an agent or script can act on it.
    let out = grove(home.path())
        .env("XDG_CACHE_HOME", cache.path())
        .args(["overview", "--json"])
        .arg(fleet.path())
        .assert()
        .success();
    let json: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(json["repos"][0]["trouble"], "denied");
    assert_eq!(json["summary"]["denied"], 1);
    assert_eq!(json["repos"][0]["stale"], true);
    assert!(json["repos"][0]["trouble_detail"].as_str().unwrap().contains("Permission denied"));
}

#[cfg(unix)]
#[test]
fn a_denied_repo_keeps_its_mark_through_the_action_verbs() {
    // pull-all/push-all re-read state without fetching, so the trouble the fetch
    // found has to be carried onto the dashboard they print — a repo that was locked
    // during the fetch must not look healthy one line later.
    let home = tempdir().unwrap();
    let cache = tempdir().unwrap();
    let fleet = tempdir().unwrap();
    fleet_with_a_denied_repo(fleet.path());

    for verb in ["pull-all", "push-all", "sync"] {
        grove(home.path())
            .env("XDG_CACHE_HOME", cache.path())
            .args([verb])
            .arg(fleet.path())
            .assert()
            .success()
            .stdout(predicate::str::contains("⊘ locked"))
            .stdout(predicate::str::contains("⊘ 1 denied"));
    }
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

/// A fleet whose repos live in organizing subfolders (`work/api`, `private/notes`)
/// alongside one sitting loose in the folder itself. Each is a real clone of a
/// local bare origin, so fetches succeed offline and every row reads clean.
fn nested_fleet(remote: &std::path::Path, fleet: &std::path::Path) {
    let git = |dir: &std::path::Path, args: &[&str]| {
        Command::new("git").current_dir(dir).args(args).assert().success();
    };
    let origin = remote.join("origin.git");
    Command::new("git").args(["init", "-q", "--bare", "-b", "main"]).arg(&origin).assert().success();

    // One commit in the origin, so every clone lands on a branch with an upstream.
    let seed = remote.join("seed");
    fs::create_dir(&seed).unwrap();
    git(&seed, &["init", "-q", "-b", "main"]);
    git(&seed, &["config", "user.email", "t@example.invalid"]);
    git(&seed, &["config", "user.name", "t"]);
    git(&seed, &["commit", "-q", "--allow-empty", "-m", "one"]);
    git(&seed, &["remote", "add", "origin", origin.to_str().unwrap()]);
    git(&seed, &["push", "-q", "origin", "main"]);

    for rel in ["loose", "work/api", "work/web", "private/notes"] {
        let dest = fleet.join(rel);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        Command::new("git").args(["clone", "-q"]).arg(&origin).arg(&dest).assert().success();
    }
}

#[test]
fn nested_repos_reach_the_dashboard_ordered_by_their_folder() {
    // Repos sorted into `work/` and `private/` are found by default, wear the folder
    // that holds them, and are ordered by it — so a group reads as one block rather
    // than interleaving with the rest by bare name.
    let home = tempdir().unwrap();
    let cache = tempdir().unwrap();
    let remote = tempdir().unwrap();
    let fleet = tempdir().unwrap();
    nested_fleet(remote.path(), fleet.path());

    let out = grove(home.path())
        .env("XDG_CACHE_HOME", cache.path())
        .arg("overview")
        .arg(fleet.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("4 repos"));
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();

    let at = |needle: &str| stdout.find(needle).unwrap_or_else(|| panic!("{needle} missing from:\n{stdout}"));
    // Ungrouped first, then each organizing folder in turn.
    assert!(at("loose") < at("private/notes"), "ungrouped repos lead:\n{stdout}");
    assert!(at("private/notes") < at("work/api"), "groups follow in name order:\n{stdout}");
    assert!(at("work/api") < at("work/web"), "within a group, by repo name:\n{stdout}");
}

#[test]
fn the_json_document_splits_the_group_from_the_repo_name() {
    let home = tempdir().unwrap();
    let cache = tempdir().unwrap();
    let remote = tempdir().unwrap();
    let fleet = tempdir().unwrap();
    nested_fleet(remote.path(), fleet.path());

    let out = grove(home.path())
        .env("XDG_CACHE_HOME", cache.path())
        .args(["overview", "--json"])
        .arg(fleet.path())
        .assert()
        .success();
    let json: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    let repos = json["repos"].as_array().unwrap();
    assert_eq!(repos.len(), 4);
    // Same order as the table, and the group is its own field — a consumer never
    // has to split a name to learn which folder a repo sits in.
    assert_eq!(repos[0]["name"], "loose");
    assert_eq!(repos[0]["group"], serde_json::Value::Null);
    assert_eq!(repos[1]["name"], "notes");
    assert_eq!(repos[1]["group"], "private");
    assert_eq!(repos[2]["name"], "api");
    assert_eq!(repos[2]["group"], "work");
}

#[test]
fn every_fleet_verb_covers_the_nested_repos() {
    // sync/pull-all/push-all act off the same scan the dashboard uses, so a repo in
    // a subfolder is theirs to move too — not just one they can list.
    let home = tempdir().unwrap();
    let remote = tempdir().unwrap();
    let fleet = tempdir().unwrap();
    nested_fleet(remote.path(), fleet.path());

    for verb in ["sync", "pull-all", "push-all", "ssh"] {
        let cache = tempdir().unwrap();
        grove(home.path())
            .env("XDG_CACHE_HOME", cache.path())
            .arg(verb)
            .arg(fleet.path())
            .assert()
            .success()
            .stdout(predicate::str::contains("work/api"))
            .stdout(predicate::str::contains("private/notes"))
            .stdout(predicate::str::contains("4 repos"));
    }
}

#[test]
fn depth_one_scans_the_folder_flat() {
    // The flat layout is one setting (or one flag) away: at depth 1 only the repo
    // sitting directly in the folder is a fleet member.
    let home = tempdir().unwrap();
    let remote = tempdir().unwrap();
    let fleet = tempdir().unwrap();
    nested_fleet(remote.path(), fleet.path());

    let flat = |cmd: &mut Command| {
        let cache = tempdir().unwrap();
        cmd.env("XDG_CACHE_HOME", cache.path())
            .assert()
            .success()
            .stdout(predicate::str::contains("1 repos"))
            .stdout(predicate::str::contains("work/api").not());
    };
    flat(grove(home.path()).args(["overview", "--depth", "1"]).arg(fleet.path()));

    grove(home.path()).args(["configure", "depth", "1"]).assert().success();
    flat(grove(home.path()).arg("overview").arg(fleet.path()));
    // The flag still wins over the setting, in the other direction too.
    let cache = tempdir().unwrap();
    grove(home.path())
        .env("XDG_CACHE_HOME", cache.path())
        .args(["overview", "--depth", "2"])
        .arg(fleet.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("4 repos"));
}

#[test]
fn a_folder_above_the_repo_home_defers_to_it() {
    // Standing in the parent of your repo home — your `$HOME` when the fleet is
    // `~/Developer`. The scan would reach the fleet from up there, but only by
    // sweeping every sibling folder too, so the folder you named wins.
    let home = tempdir().unwrap();
    let cache = tempdir().unwrap();
    let remote = tempdir().unwrap();
    let above = tempdir().unwrap();

    let fleet = above.path().join("Developer");
    fs::create_dir(&fleet).unwrap();
    nested_fleet(remote.path(), &fleet);
    // A sibling of the fleet holding a repo of its own — what the sweep would drag in.
    let sibling = above.path().join("Documents");
    fs::create_dir(&sibling).unwrap();
    Command::new("git").args(["clone", "-q"]).arg(remote.path().join("origin.git")).arg(sibling.join("Playground")).assert().success();

    grove(home.path()).args(["configure", "default_dir"]).arg(&fleet).assert().success();
    grove(home.path())
        .env("XDG_CACHE_HOME", cache.path())
        .current_dir(above.path())
        .arg("overview")
        .assert()
        .success()
        .stderr(predicate::str::contains("your repos live in"))
        .stdout(predicate::str::contains("work/api"))
        .stdout(predicate::str::contains("4 repos"))
        .stdout(predicate::str::contains("Documents/Playground").not());
}

#[test]
fn naming_a_folder_as_the_repo_home_lets_it_be_scanned() {
    // Nothing is above itself, so pointing default_dir at a folder makes that
    // folder the root — someone who wants their `$HOME` scanned just says so.
    let home = tempdir().unwrap();
    let cache = tempdir().unwrap();
    let remote = tempdir().unwrap();
    let root = tempdir().unwrap();
    nested_fleet(remote.path(), root.path());

    grove(home.path()).args(["configure", "default_dir"]).arg(root.path()).assert().success();
    grove(home.path())
        .env("XDG_CACHE_HOME", cache.path())
        .current_dir(root.path())
        .arg("overview")
        .assert()
        .success()
        .stdout(predicate::str::contains("work/api"))
        .stdout(predicate::str::contains("4 repos"));
}

#[test]
fn a_folder_holding_repos_only_deeper_down_defers_to_the_repo_home() {
    // Choosing where to run is a shallow question: repos buried below this folder
    // don't make it the fleet you meant to be standing in, so the folder you named
    // wins. Pass it explicitly (or set it as default_dir) to scan it.
    let home = tempdir().unwrap();
    let cache = tempdir().unwrap();
    let remote = tempdir().unwrap();
    let elsewhere = tempdir().unwrap();
    let cwd = tempdir().unwrap();

    nested_fleet(remote.path(), elsewhere.path());
    // The current folder holds no repo of its own — only one a level further down.
    fs::create_dir(cwd.path().join("clientA")).unwrap();
    Command::new("git").args(["clone", "-q"]).arg(remote.path().join("origin.git")).arg(cwd.path().join("clientA/site")).assert().success();

    grove(home.path()).args(["configure", "default_dir"]).arg(elsewhere.path()).assert().success();
    grove(home.path())
        .env("XDG_CACHE_HOME", cache.path())
        .current_dir(cwd.path())
        .arg("overview")
        .assert()
        .success()
        .stderr(predicate::str::contains("no repos to list here"))
        .stdout(predicate::str::contains("work/api"))
        .stdout(predicate::str::contains("clientA/site").not());
}

#[test]
fn a_repo_home_is_kept_and_scanned_to_the_full_depth() {
    // The shallow probe only picks the folder. Standing in a repo home that holds
    // repos directly *and* in subfolders lists both — it is not a depth-1 scan.
    let home = tempdir().unwrap();
    let cache = tempdir().unwrap();
    let remote = tempdir().unwrap();
    let elsewhere = tempdir().unwrap();
    let fleet = tempdir().unwrap();
    nested_fleet(remote.path(), fleet.path());

    grove(home.path()).args(["configure", "default_dir"]).arg(elsewhere.path()).assert().success();
    grove(home.path())
        .env("XDG_CACHE_HOME", cache.path())
        .current_dir(fleet.path())
        .arg("overview")
        .assert()
        .success()
        .stderr(predicate::str::contains("default_dir").not())
        .stdout(predicate::str::contains("work/api"))
        .stdout(predicate::str::contains("4 repos"));
}
