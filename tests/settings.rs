//! `aequalis --settings` and `aequalis --set`: one place for every setting, nothing from the environment.

mod common;

#[test]
fn set_show_and_reset() {
    let dir = common::config("set", &[]);
    let o = common::aequalis(&dir, &["--set", "precision=2"]);
    assert_eq!(common::out(&o), "precision = 2");
    assert_eq!(common::out(&common::aequalis(&dir, &["1/3"])), "0.33");
    // `--set key`: back to the default.
    assert_eq!(common::out(&common::aequalis(&dir, &["--set", "precision"])), "precision = 4");
    assert_eq!(common::out(&common::aequalis(&dir, &["1/3"])), "0.3333");
    // Normalised.
    assert_eq!(common::out(&common::aequalis(&dir, &["--set=currency.local=swiss franc"])), "currency.local = CHF");
    let shown = common::out(&common::aequalis(&dir, &["--settings"]));
    assert!(shown.contains("currency.local   CHF  (default: auto)") && shown.contains("cities.refresh   manual"), "{shown}");
    let json = common::out(&common::aequalis(&dir, &["--settings", "--json"]));
    assert!(json.contains(r#"{"key":"currency.local","value":"CHF","default":"auto","kind":"text","options":[]"#), "{json}");
    assert!(json.contains(r#"{"key":"cities.refresh","value":"manual","default":"manual","kind":"choice","options":["silent","notify","manual"]"#), "{json}");
    assert!(json.contains(r#""key":"offline","value":"true","default":"false","kind":"bool""#), "{json}");
    // What `auto` stands for here, and the currencies to pick the local one from.
    assert!(json.contains(r#""key":"distance.unit","value":"auto""#) && json.contains(r#""auto":"km""#) || json.contains(r#""auto":"mi""#), "{json}");
    assert!(json.contains(r#""currencies":[{"code":"USD","name":"US dollar"}"#), "{json}");
    // `--reset-settings`: every default back.
    assert_eq!(common::out(&common::aequalis(&dir, &["--reset-settings"])), "every setting is back to its default");
    let shown = common::out(&common::aequalis(&dir, &["--settings"]));
    assert!(shown.contains("currency.local   auto\n") && shown.contains("offline          false\n"), "{shown}");
}

#[test]
fn wrong_keys_and_values() {
    let dir = common::config("wrong", &[]);
    let o = common::aequalis(&dir, &["--set", "colour=blue"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(common::err(&o).starts_with("aequalis: error: unknown setting `colour`"), "{}", common::err(&o));
    let o = common::aequalis(&dir, &["--set", "cities.refresh=often"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(common::err(&o).contains("`cities.refresh` must be one of silent, notify, manual"));
    // A broken file: an error naming it, but --set still fixes it.
    std::fs::write(dir.join("aequalis/settings.json"), r#"{"precision": "lots"}"#).unwrap();
    let o = common::aequalis(&dir, &["1/3"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(common::err(&o).contains("`precision` must be an integer from 0 to 15"), "{}", common::err(&o));
    std::fs::write(dir.join("aequalis/settings.json"), "{}").unwrap();
    assert!(common::aequalis(&dir, &["--set", "precision=3"]).status.success());
}

#[test]
fn the_environment_is_not_read() {
    let dir = common::config("env", &[]);
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_aequalis"))
        .args(["1/3"])
        .env("XDG_CONFIG_HOME", &dir)
        .env("AEQUALIS_SCALE", "1")
        .env("AEQUALIS_MODE", "bogus")
        .output()
        .unwrap();
    assert_eq!(common::out(&o), "0.3333");
}
