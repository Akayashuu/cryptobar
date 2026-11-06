use super::*;
use crate::report::Report;

#[test]
fn a_minimal_config_uses_the_defaults() {
    let cfg = Config::parse(r#"coins = ["bitcoin"]"#).unwrap();
    assert_eq!(cfg.currency.as_str(), "eur");
    assert_eq!(cfg.refresh_interval, 60);
    assert_eq!(cfg.cycle_interval, 0);
    assert!(cfg.sparkline);
    assert_eq!(cfg.sparkline_width, 8);
    assert_eq!(cfg.waybar_signal, 8);
}

#[test]
fn an_empty_coin_list_is_rejected() {
    let err = Config::parse("coins = []").unwrap_err().to_string();
    assert!(err.contains("at least one"), "unclear message: {err}");
}

#[test]
fn a_duplicate_coin_is_rejected() {
    let err = Config::parse(r#"coins = ["bitcoin", "Bitcoin"]"#)
        .unwrap_err()
        .to_string();
    assert!(err.contains("bitcoin"), "unclear message: {err}");
}

#[test]
fn too_short_an_interval_is_rejected_to_protect_the_quota() {
    let err = Config::parse("coins = [\"bitcoin\"]\nrefresh_interval = 5")
        .unwrap_err()
        .to_string();
    assert!(err.contains("15"), "unclear message: {err}");
}

#[test]
fn an_out_of_range_signal_is_rejected() {
    assert!(Config::parse("coins = [\"bitcoin\"]\nwaybar_signal = 0").is_err());
    assert!(Config::parse("coins = [\"bitcoin\"]\nwaybar_signal = 31").is_err());
    assert!(Config::parse("coins = [\"bitcoin\"]\nwaybar_signal = 30").is_ok());
}

#[test]
fn an_invalid_currency_is_rejected_naming_the_field() {
    let failure = Config::parse("coins = [\"bitcoin\"]\ncurrency = \"xyz\"").unwrap_err();
    let err = Report(&failure).to_string();
    assert!(err.contains("currency"), "unclear message: {err}");
}

#[test]
fn decimals_are_auto_or_fixed() {
    assert_eq!(
        Config::parse("coins = [\"bitcoin\"]").unwrap().decimals(),
        Decimals::Auto
    );
    assert_eq!(
        Config::parse("coins = [\"bitcoin\"]\ndecimals = 3")
            .unwrap()
            .decimals(),
        Decimals::Fixed(3)
    );
}

#[test]
fn an_unknown_field_is_rejected_to_catch_typos() {
    let failure = Config::parse("coins = [\"bitcoin\"]\nrefresh_intervall = 60").unwrap_err();
    let err = Report(&failure).to_string();
    assert!(err.contains("refresh_intervall"), "unclear message: {err}");
}

#[test]
fn the_default_config_written_then_reread_is_valid() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    Config::write_default(&path, false).unwrap();
    let cfg = Config::load_from(&path).unwrap();
    assert_ne!(cfg.coins.len(), 0);
}

#[test]
fn write_default_refuses_to_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    Config::write_default(&path, false).unwrap();
    assert!(Config::write_default(&path, false).is_err());
}
