//! `tally --settings` and `tally --set`: one place for every setting, nothing from the environment.

mod common;

#[test]
fn set_show_and_reset() {
    let dir = common::config("set", &[]);
    let o = common::tally(&dir, &["--set", "precision=2"]);
    assert_eq!(common::out(&o), "precision = 2");
    assert_eq!(common::out(&common::tally(&dir, &["1/3"])), "0.33");
    // `--set key`: back to the default.
    assert_eq!(common::out(&common::tally(&dir, &["--set", "precision"])), "precision = 4");
    assert_eq!(common::out(&common::tally(&dir, &["1/3"])), "0.3333");
    // Normalised.
    assert_eq!(common::out(&common::tally(&dir, &["--set=currency.local=swiss franc"])), "currency.local = CHF");
    let shown = common::out(&common::tally(&dir, &["--settings"]));
    assert!(shown.contains("currency.local   CHF  (default: auto)") && shown.contains("cities.refresh   manual"), "{shown}");
    let json = common::out(&common::tally(&dir, &["--settings", "--json"]));
    assert!(json.contains(r#"{"key":"currency.local","value":"CHF","default":"auto","kind":"text","options":[]"#), "{json}");
    assert!(json.contains(r#"{"key":"cities.refresh","value":"manual","default":"manual","kind":"choice","options":["silent","notify","manual"]"#), "{json}");
    assert!(json.contains(r#""key":"offline","value":"true","default":"false","kind":"bool""#), "{json}");
}

#[test]
fn wrong_keys_and_values() {
    let dir = common::config("wrong", &[]);
    let o = common::tally(&dir, &["--set", "colour=blue"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(common::err(&o).starts_with("tally: error: unknown setting `colour`"), "{}", common::err(&o));
    let o = common::tally(&dir, &["--set", "cities.refresh=often"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(common::err(&o).contains("`cities.refresh` must be one of silent, notify, manual"));
    // A broken file: an error naming it, but --set still fixes it.
    std::fs::write(dir.join("tally/settings.json"), r#"{"precision": "lots"}"#).unwrap();
    let o = common::tally(&dir, &["1/3"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(common::err(&o).contains("`precision` must be an integer from 0 to 15"), "{}", common::err(&o));
    std::fs::write(dir.join("tally/settings.json"), "{}").unwrap();
    assert!(common::tally(&dir, &["--set", "precision=3"]).status.success());
}

#[test]
fn the_environment_is_not_read() {
    let dir = common::config("env", &[]);
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_tally"))
        .args(["1/3"])
        .env("XDG_CONFIG_HOME", &dir)
        .env("TALLY_SCALE", "1")
        .env("TALLY_MODE", "bogus")
        .output()
        .unwrap();
    assert_eq!(common::out(&o), "0.3333");
}
