use std::env::var;
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::UNIX_EPOCH;
use std::time::{Duration, SystemTime};

use assert_cmd::prelude::*;
use serde_json::{Value, json};
use tempfile::TempDir;

const TICK_TIMEOUT: Duration = Duration::from_secs(20);

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
    seed_cache_priced(cache, 42000.5);
}

pub(crate) fn seed_cache_priced(cache: &Path, bitcoin_price: f64) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let entry = json!({
        "fetched_at": now,
        "value": [
            {
                "id": "bitcoin", "symbol": "btc", "name": "Bitcoin", "price": bitcoin_price,
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

pub(crate) fn fake_notifier(env: &Env) -> (PathBuf, PathBuf) {
    let bin = env.config.with_file_name("bin");
    fs::create_dir(&bin).unwrap();
    let sent = env.config.with_file_name("notified.log");
    let script = bin.join("notify-send");
    fs::write(
        &script,
        format!("#!/bin/sh\necho notified >> '{}'\n", sent.display()),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    (bin, sent)
}

pub(crate) fn path_with(bin: &Path) -> String {
    format!("{}:{}", bin.display(), var("PATH").unwrap_or_default())
}

pub(crate) fn touch(path: &Path, offset_secs: u64) {
    let touched = SystemTime::now()
        .checked_add(Duration::from_secs(offset_secs))
        .unwrap();
    fs::File::open(path).unwrap().set_modified(touched).unwrap();
}

pub(crate) struct Watch {
    child: Child,
    ticks: Receiver<String>,
    log: PathBuf,
    tick_number: usize,
}

impl Watch {
    pub(crate) fn spawn(env: &Env, path: Option<String>) -> Self {
        let log = env.config.with_file_name("watch.log");
        let mut cmd = cryptobar(env);
        cmd.args(["bar", "--watch"])
            .stdout(Stdio::piped())
            .stderr(fs::File::create(&log).unwrap());
        if let Some(path) = path {
            cmd.env("PATH", path);
        }
        let mut child = cmd.spawn().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, ticks) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            ticks,
            log,
            tick_number: 0,
        }
    }

    pub(crate) fn tick(&mut self) {
        self.tick_number = self.tick_number.saturating_add(1);
        if let Err(error) = self.ticks.recv_timeout(TICK_TIMEOUT) {
            panic!(
                "tick {} got no line within {TICK_TIMEOUT:?} ({error})\nchild stderr:\n{}",
                self.tick_number,
                self.stderr()
            );
        }
    }

    pub(crate) fn stop(&mut self) -> String {
        self.kill();
        self.stderr()
    }

    fn stderr(&self) -> String {
        fs::read_to_string(&self.log)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", self.log.display()))
    }

    fn kill(&mut self) {
        drop(self.child.kill());
        drop(self.child.wait());
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.kill();
    }
}
