//! End-to-end tests of the binary, without network.
//!
//! The cache is seeded by hand: the CoinGecko client is therefore never called,
//! which makes these tests hermetic and fast.

#[cfg(test)]
mod support {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    use assert_cmd::prelude::*;
    use serde_json::{Value, json};
    use tempfile::TempDir;

    pub(crate) struct Env {
        pub _dir: TempDir,
        pub config: PathBuf,
        pub cache: PathBuf,
        pub runtime: PathBuf,
    }

    pub(crate) fn setup() -> Env {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.toml");
        let cache = dir.path().join("cache");
        let runtime = dir.path().join("run");
        fs::create_dir_all(&cache).unwrap();
        fs::create_dir_all(&runtime).unwrap();
        fs::write(
            &config,
            "coins = [\"bitcoin\", \"ethereum\"]\ncurrency = \"eur\"\nrefresh_interval = 3600\n",
        )
        .unwrap();
        Env {
            _dir: dir,
            config,
            cache,
            runtime,
        }
    }

    pub(crate) fn with_runtime_under_a_file() -> Env {
        let env = setup();
        let blocker = env.runtime.with_file_name("blocker");
        fs::write(&blocker, b"").unwrap();
        Env {
            runtime: blocker.join("run"),
            ..env
        }
    }

    pub(crate) fn seed_cache(cache: &Path) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let entry = json!({
            "fetched_at": now,
            "value": [
                {
                    "id": "bitcoin", "symbol": "btc", "name": "Bitcoin", "price": 42000.5,
                    "change_1h": 0.2, "change_24h": 1.5, "change_7d": -3.0,
                    "sparkline": [1.0, 2.0, 3.0, 2.5]
                },
                {
                    "id": "ethereum", "symbol": "eth", "name": "Ethereum", "price": 2500.0,
                    "change_1h": null, "change_24h": -0.75, "change_7d": 4.0,
                    "sparkline": [3.0, 2.0, 1.0]
                }
            ]
        });
        fs::write(
            cache.join("markets-eur-bitcoin_ethereum.json"),
            serde_json::to_vec(&entry).unwrap(),
        )
        .unwrap();
    }

    pub(crate) fn cryptobar(env: &Env) -> Command {
        let mut cmd = Command::cargo_bin("cryptobar").unwrap();
        cmd.env("CRYPTOBAR_CACHE_DIR", &env.cache)
            .env("CRYPTOBAR_RUNTIME_DIR", &env.runtime)
            .env("CRYPTOBAR_API_BASE", "http://127.0.0.1:1/api")
            .arg("--config")
            .arg(&env.config);
        cmd
    }

    pub(crate) fn run(env: &Env, args: &[&str]) -> Output {
        cryptobar(env).args(args).output().unwrap()
    }

    pub(crate) fn bar_json(env: &Env) -> Value {
        let output = run(env, &["bar"]);
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert_eq!(stdout.lines().count(), 1, "Waybar expects exactly one line");
        serde_json::from_str(stdout.trim()).unwrap()
    }
}

use std::fs::{self, File};
use std::path::Path;
use std::process::Stdio;
use std::thread;
use std::time::{Duration, SystemTime};

use assert_cmd::assert::OutputAssertExt;
use support::{bar_json, cryptobar, run, seed_cache, setup};

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
    let log = env.config.with_file_name("watch.log");

    let mut child = cryptobar(&env)
        .args(["bar", "--watch"])
        .stdout(Stdio::null())
        .stderr(File::create(&log).unwrap())
        .spawn()
        .unwrap();
    thread::sleep(Duration::from_millis(1000));
    for elapsed in 1..=2 {
        let touched = SystemTime::now() + Duration::from_secs(elapsed);
        File::open(&state).unwrap().set_modified(touched).unwrap();
        thread::sleep(Duration::from_millis(1000));
    }
    fs::remove_dir(&state).unwrap();
    fs::write(&state, b"{}").unwrap();
    thread::sleep(Duration::from_millis(1000));
    fs::remove_file(&state).unwrap();
    fs::create_dir(&state).unwrap();
    thread::sleep(Duration::from_millis(1000));
    child.kill().unwrap();
    child.wait().unwrap();

    let stderr = fs::read_to_string(&log).unwrap();
    assert_eq!(
        stderr
            .lines()
            .filter(|line| line.contains("could not read the state file"))
            .count(),
        2,
        "stderr: {stderr}"
    );
    assert_eq!(
        stderr
            .lines()
            .filter(|line| line.contains("alerts are paused until the state file"))
            .count(),
        2,
        "stderr: {stderr}"
    );
}
