//! Tests bout en bout du binaire, sans réseau.
//!
//! Le cache est pré-rempli à la main : le client CoinGecko n'est donc jamais
//! sollicité, ce qui rend ces tests hermétiques et rapides.

// L'indexation d'un `serde_json::Value` retourne `Null` au lieu de paniquer :
// le lint ne s'applique pas ici, et un test doit rester lisible.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::path::Path;
use std::process::Command;

use assert_cmd::prelude::*;
use serde_json::json;
use tempfile::TempDir;

struct Env {
    _dir: TempDir,
    config: std::path::PathBuf,
    cache: std::path::PathBuf,
    runtime: std::path::PathBuf,
}

fn setup() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.toml");
    let cache = dir.path().join("cache");
    let runtime = dir.path().join("run");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::create_dir_all(&runtime).unwrap();
    std::fs::write(
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

/// Écrit une entrée de cache correspondant à la clé attendue par `markets_cached`.
fn seed_cache(cache: &Path) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
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
    std::fs::write(
        cache.join("markets-eur-bitcoin_ethereum.json"),
        serde_json::to_vec(&entry).unwrap(),
    )
    .unwrap();
}

fn cryptobar(env: &Env) -> Command {
    let mut cmd = Command::cargo_bin("cryptobar").unwrap();
    cmd.env("CRYPTOBAR_CACHE_DIR", &env.cache)
        .env("CRYPTOBAR_RUNTIME_DIR", &env.runtime)
        // Base injoignable : aucun test ne doit dépendre du réseau.
        .env("CRYPTOBAR_API_BASE", "http://127.0.0.1:1/api")
        .arg("--config")
        .arg(&env.config);
    cmd
}

fn bar_json(env: &Env) -> serde_json::Value {
    let output = cryptobar(env).arg("bar").output().unwrap();
    assert!(
        output.status.success(),
        "stderr : {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.lines().count(),
        1,
        "Waybar attend exactement une ligne"
    );
    serde_json::from_str(stdout.trim()).unwrap()
}

#[test]
fn bar_emet_un_json_waybar_valide_depuis_le_cache() {
    let env = setup();
    seed_cache(&env.cache);
    let value = bar_json(&env);
    assert!(value["text"].as_str().unwrap().contains("BTC"));
    assert_eq!(value["class"], "up");
    assert!(value["tooltip"].as_str().unwrap().contains("ETH"));
}

#[test]
fn next_fait_basculer_la_cotation_affichee() {
    let env = setup();
    seed_cache(&env.cache);
    assert!(bar_json(&env)["text"].as_str().unwrap().contains("BTC"));

    cryptobar(&env).arg("next").output().unwrap();
    assert!(bar_json(&env)["text"].as_str().unwrap().contains("ETH"));

    cryptobar(&env).arg("prev").output().unwrap();
    assert!(bar_json(&env)["text"].as_str().unwrap().contains("BTC"));
}

#[test]
fn sans_cache_ni_reseau_le_module_reste_valide() {
    let env = setup();
    // Aucune entrée de cache et pas de réseau autorisé : la sortie doit rester
    // du JSON exploitable par Waybar plutôt qu'une erreur.
    let output = cryptobar(&env).arg("bar").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let value: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(value["text"].is_string());
}

#[test]
fn config_check_valide_le_fichier() {
    let env = setup();
    cryptobar(&env).args(["config", "check"]).assert().success();
}

#[test]
fn config_check_rejette_un_fichier_invalide() {
    let env = setup();
    std::fs::write(&env.config, "coins = []\n").unwrap();
    cryptobar(&env).args(["config", "check"]).assert().failure();
}

#[test]
fn config_init_refuse_decraser_sans_force() {
    let env = setup();
    cryptobar(&env).args(["config", "init"]).assert().failure();
    cryptobar(&env)
        .args(["config", "init", "--force"])
        .assert()
        .success();
}
