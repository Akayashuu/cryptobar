//! End-to-end tests of the binary, without network.
//!
//! The cache is seeded by hand: the CoinGecko client is therefore never called,
//! which makes these tests hermetic and fast.

#[cfg(test)]
mod support;

use std::fs;
use std::path::Path;

use assert_cmd::assert::OutputAssertExt;
use support::{
    Watch, bar_json, cryptobar, fake_notifier, path_with, run, seed_cache, seed_cache_priced,
    setup, touch,
};

fn lines_with(text: &str, needle: &str) -> usize {
    text.lines().filter(|line| line.contains(needle)).count()
}

#[test]
fn bar_emits_valid_waybar_json_from_the_cache() {
    let env = setup();
    seed_cache(&env.cache);
    let value = bar_json(&env);
    assert!(value["text"].as_str().unwrap().contains("BTC"));
    assert_eq!(value["class"], "up");
    assert!(value["tooltip"].as_str().unwrap().contains("ETH"));
}

#[test]
fn next_switches_the_displayed_quote() {
    let env = setup();
    seed_cache(&env.cache);
    assert!(bar_json(&env)["text"].as_str().unwrap().contains("BTC"));

    run(&env, &["next"]);
    assert!(bar_json(&env)["text"].as_str().unwrap().contains("ETH"));

    run(&env, &["prev"]);
    assert!(bar_json(&env)["text"].as_str().unwrap().contains("BTC"));
}

#[test]
fn without_cache_or_network_the_module_stays_valid() {
    let env = setup();
    let output = run(&env, &["bar"]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let value: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(value["text"].is_string());
}

#[test]
fn config_check_validates_the_file() {
    let env = setup();
    cryptobar(&env).args(["config", "check"]).assert().success();
}

#[test]
fn config_check_rejects_an_invalid_file() {
    let env = setup();
    fs::write(&env.config, "coins = []\n").unwrap();
    cryptobar(&env).args(["config", "check"]).assert().failure();
}

#[test]
fn config_init_refuses_to_overwrite_without_force() {
    let env = setup();
    cryptobar(&env).args(["config", "init"]).assert().failure();
    cryptobar(&env)
        .args(["config", "init", "--force"])
        .assert()
        .success();
}

fn corrupt_state_warning(state: &Path) -> String {
    format!(
        "cryptobar: the state file {} is corrupt, starting from the default state: expected value at line 1 column 1\n",
        state.display()
    )
}

#[test]
fn an_unreadable_state_still_renders_the_bar() {
    let env = support::with_runtime_under_a_file();
    seed_cache(&env.cache);
    let output = run(&env, &["bar"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1);
    assert!(stdout.contains("BTC"));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1);
    assert!(stderr.starts_with("cryptobar: could not read the state file "));
    assert!(stderr.contains(", starting from the default state: "));
}

#[test]
fn next_warns_once_and_leaves_an_unreadable_state_alone() {
    let env = setup();
    seed_cache(&env.cache);
    fs::create_dir(env.runtime.join("state.json")).unwrap();
    let output = run(&env, &["next"]);
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1);
    assert!(stderr.starts_with("cryptobar: could not read the state file "));
    assert!(env.runtime.join("state.json").is_dir());
}

#[test]
fn an_unreadable_backoff_is_reported_once_per_run() {
    let env = setup();
    fs::create_dir(env.cache.join("backoff.state")).unwrap();
    let output = run(&env, &["bar"]);
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        stderr
            .lines()
            .filter(|line| line.contains("local cache"))
            .count(),
        1
    );
}

#[test]
fn a_corrupt_state_is_reported_once_and_next_repairs_it() {
    let env = setup();
    seed_cache(&env.cache);
    let state = env.runtime.join("state.json");
    fs::write(&state, b"garbage").unwrap();

    let first = run(&env, &["next"]);
    assert!(first.status.success());
    assert_eq!(
        String::from_utf8(first.stderr).unwrap(),
        corrupt_state_warning(&state)
    );
    serde_json::from_slice::<serde_json::Value>(&fs::read(&state).unwrap()).unwrap();

    let second = run(&env, &["next"]);
    assert!(second.status.success());
    assert_eq!(String::from_utf8(second.stderr).unwrap(), "");
}

#[test]
fn bar_repairs_a_corrupt_state_and_stays_quiet_afterwards() {
    let env = setup();
    seed_cache(&env.cache);
    let state = env.runtime.join("state.json");
    fs::write(&state, b"garbage").unwrap();

    let first = run(&env, &["bar"]);
    assert!(first.status.success());
    assert_eq!(
        String::from_utf8(first.stderr).unwrap(),
        corrupt_state_warning(&state)
    );

    let second = run(&env, &["bar"]);
    assert!(second.status.success());
    assert_eq!(String::from_utf8(second.stderr).unwrap(), "");
    serde_json::from_slice::<serde_json::Value>(&fs::read(&state).unwrap()).unwrap();
}

#[test]
fn a_failed_repair_is_reported_once_by_the_save_that_follows() {
    let env = setup();
    seed_cache(&env.cache);
    let state = env.runtime.join("state.json");
    fs::write(&state, b"garbage").unwrap();
    fs::create_dir(env.runtime.join("state.json.tmp")).unwrap();

    let output = run(&env, &["next"]);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 2, "stderr: {stderr}");
    assert!(stderr.starts_with(&corrupt_state_warning(&state)));
    assert_eq!(
        stderr
            .lines()
            .filter(|line| line.contains("could not write the state file"))
            .count(),
        1
    );
    assert_eq!(output.status.code(), Some(74));
    assert_eq!(fs::read(&state).unwrap(), b"garbage");
}

#[test]
fn bar_watch_reports_an_unreadable_state_once_per_failure() {
    let env = setup();
    seed_cache(&env.cache);
    fs::write(
        &env.config,
        "coins = [\"bitcoin\", \"ethereum\"]\ncurrency = \"eur\"\nrefresh_interval = 15\n\n[[alerts]]\ncoin = \"bitcoin\"\nabove = 1e15\n",
    )
    .unwrap();
    let state = env.runtime.join("state.json");
    fs::create_dir(&state).unwrap();
    let mut watch = Watch::spawn(&env, None);

    watch.tick();
    touch(&state, 1);
    watch.tick();
    touch(&state, 2);
    watch.tick();
    fs::remove_dir(&state).unwrap();
    watch.tick();
    fs::write(&state, b"{}").unwrap();
    watch.tick();
    fs::remove_file(&state).unwrap();
    watch.tick();
    fs::create_dir(&state).unwrap();
    watch.tick();

    let stderr = watch.stop();
    assert_eq!(
        lines_with(&stderr, "could not read the state file"),
        2,
        "stderr: {stderr}"
    );
    assert_eq!(
        lines_with(&stderr, "alerts are paused until the state file"),
        2,
        "stderr: {stderr}"
    );
}

#[test]
fn bar_watch_reports_a_failed_repair_and_alert_save_once_and_fires_once() {
    let env = setup();
    seed_cache(&env.cache);
    fs::write(
        &env.config,
        "coins = [\"bitcoin\", \"ethereum\"]\ncurrency = \"eur\"\nrefresh_interval = 15\n\n[[alerts]]\ncoin = \"bitcoin\"\nabove = 1\n",
    )
    .unwrap();
    fs::write(env.runtime.join("state.json"), b"garbage").unwrap();
    fs::create_dir(env.runtime.join("state.json.tmp")).unwrap();
    let (bin, sent) = fake_notifier(&env);
    let mut watch = Watch::spawn(&env, Some(path_with(&bin)));

    watch.tick();
    touch(&env.config, 1);
    watch.tick();
    touch(&env.config, 2);
    watch.tick();

    let stderr = watch.stop();
    assert_eq!(lines_with(&stderr, "is corrupt"), 1, "stderr: {stderr}");
    assert_eq!(
        lines_with(&stderr, "could not write the state file"),
        1,
        "stderr: {stderr}"
    );
    let notified = fs::read_to_string(&sent).unwrap_or_default();
    assert_eq!(notified.lines().count(), 1, "notifications: {notified}");
}

#[test]
fn bar_watch_reports_a_failed_save_again_after_the_state_was_rewritten() {
    let env = setup();
    seed_cache(&env.cache);
    fs::write(
        &env.config,
        "coins = [\"bitcoin\", \"ethereum\"]\ncurrency = \"eur\"\nrefresh_interval = 15\n\n[[alerts]]\ncoin = \"bitcoin\"\nabove = 1\n",
    )
    .unwrap();
    let state = env.runtime.join("state.json");
    let tmp = env.runtime.join("state.json.tmp");
    fs::write(&state, b"{}").unwrap();
    fs::create_dir(&tmp).unwrap();
    let (bin, sent) = fake_notifier(&env);
    let mut watch = Watch::spawn(&env, Some(path_with(&bin)));

    watch.tick();
    fs::remove_dir(&tmp).unwrap();
    assert!(run(&env, &["next"]).status.success());
    watch.tick();
    fs::create_dir(&tmp).unwrap();
    seed_cache_priced(&env.cache, 0.5);
    touch(&state, 1);
    watch.tick();

    let stderr = watch.stop();
    assert_eq!(
        lines_with(&stderr, "could not write the state file"),
        2,
        "stderr: {stderr}"
    );
    let notified = fs::read_to_string(&sent).unwrap_or_default();
    assert_eq!(notified.lines().count(), 1, "notifications: {notified}");
}

#[test]
fn bar_watch_reports_an_offline_backoff_once() {
    let env = setup();
    fs::write(
        &env.config,
        "coins = [\"bitcoin\", \"ethereum\"]\ncurrency = \"eur\"\nrefresh_interval = 15\n",
    )
    .unwrap();
    let state = env.runtime.join("state.json");
    fs::write(&state, b"{}").unwrap();
    let mut watch = Watch::spawn(&env, None);

    watch.tick();
    touch(&state, 1);
    watch.tick();
    touch(&state, 2);
    watch.tick();

    let stderr = watch.stop();
    assert_eq!(lines_with(&stderr, "backoff active"), 1, "stderr: {stderr}");
    assert_eq!(stderr.lines().count(), 2, "stderr: {stderr}");
}
