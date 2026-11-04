//! Location keys in the machine file (spec §15): read, write, refuse; the table survives failures.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a panic is the failure signal

use mda_core::registry::ArtifactType::{Snippet, Variables};
use mda_vault::config::MachineConfig;
use mda_vault::{Location, VaultError, Where, location, set_location};

fn abs() -> String {
    std::env::temp_dir()
        .join("mda-loc-vault")
        .to_string_lossy()
        .into_owned()
}

fn cfg(toml: &str) -> MachineConfig {
    MachineConfig {
        rest: toml.parse().unwrap(),
        ..Default::default()
    }
}

fn reason(r: Result<impl std::fmt::Debug, VaultError>) -> &'static str {
    let Err(VaultError::LocationInvalid { reason, .. }) = r else {
        unreachable!("expected LocationInvalid")
    };
    reason
}

#[test]
fn reads_dir_and_root() {
    let v = abs();
    let c = cfg(&format!(
        "[types.Snippet]\nvault = {v:?}\ndir = \"Lib\"\n[types.Variables]\nroot = true\n"
    ));
    assert_eq!(
        location(&c, Snippet).unwrap(),
        Some(Location {
            vault: Some(v),
            place: Where::Dir("Lib".into())
        })
    );
    assert_eq!(location(&c, Variables).unwrap().unwrap().place, Where::Root);
}

#[test]
fn vault_only_is_default_and_enabled_only_is_none() {
    let v = abs();
    let c = cfg(&format!(
        "[types.Snippet]\nvault = {v:?}\n[types.Variables]\nenabled = true\n"
    ));
    assert_eq!(
        location(&c, Snippet).unwrap().unwrap().place,
        Where::Default
    );
    assert_eq!(location(&c, Variables).unwrap(), None);
}

#[test]
fn hostile_shapes_refused() {
    let v = abs();
    let cases: Vec<(String, &str)> = vec![
        ("[types.Snippet]\nvault = \"rel/path\"".into(), "vault"),
        ("[types.Snippet]\nvault = 3".into(), "vault"),
        ("[types.Snippet]\ndir = \".obsidian\"".into(), "dir"),
        ("[types.Snippet]\ndir = \"a/b\"".into(), "dir"),
        ("[types.Snippet]\ndir = \"a\\\\b\"".into(), "dir"),
        ("[types.Snippet]\ndir = \"..\"".into(), "dir"),
        ("[types.Snippet]\ndir = \"\"".into(), "dir"),
        ("[types.Snippet]\ndir = \"x\"\nroot = true".into(), "both"),
        ("[types.Snippet]\nroot = false".into(), "neither"),
        (
            format!("[types.Snippet]\nvault = {v:?}\nroot = false"),
            "neither",
        ),
        ("[types.Snippet]\nroot = \"yes\"".into(), "shape"),
        ("[types]\nSnippet = 1".into(), "shape"),
        ("types = 1".into(), "shape"),
    ];
    for (toml, want) in cases {
        assert_eq!(reason(location(&cfg(&toml), Snippet)), want, "{toml}");
    }
}

#[test]
fn set_round_trips_keeps_siblings_and_clears() {
    let v = abs();
    let mut c = cfg("other = 1\n[types.Snippet]\nenabled = true\nextra = \"k\"\n");
    let l = Location {
        vault: Some(v),
        place: Where::Dir("Lib".into()),
    };
    set_location(&mut c, Snippet, Some(l.clone())).unwrap();
    assert_eq!(location(&c, Snippet).unwrap(), Some(l));
    set_location(
        &mut c,
        Snippet,
        Some(Location {
            vault: None,
            place: Where::Root,
        }),
    )
    .unwrap();
    let t = c.rest["types"]["Snippet"].as_table().unwrap();
    assert_eq!(t.len(), 3); // enabled, extra, root
    assert!(t["root"].as_bool().unwrap());
    set_location(&mut c, Snippet, None).unwrap();
    assert_eq!(location(&c, Snippet).unwrap(), None);
    assert_eq!(
        c.rest,
        cfg("other = 1\n[types.Snippet]\nenabled = true\nextra = \"k\"\n").rest
    );
    // empty entry and `types` vanish
    let mut e = cfg("");
    set_location(
        &mut e,
        Snippet,
        Some(Location {
            vault: None,
            place: Where::Root,
        }),
    )
    .unwrap();
    set_location(&mut e, Snippet, None).unwrap();
    assert!(e.rest.is_empty());
}

#[test]
fn failed_set_leaves_table_untouched() {
    let base = "[types.Snippet]\ndir = \"Lib\"\n";
    let mut c = cfg(base);
    for loc in [
        Location {
            vault: Some("rel".into()),
            place: Where::Root,
        },
        Location {
            vault: None,
            place: Where::Dir(".git".into()),
        },
        Location {
            vault: None,
            place: Where::Dir("a/b".into()),
        },
    ] {
        reason(set_location(&mut c, Snippet, Some(loc)));
        assert_eq!(c.rest, cfg(base).rest);
    }
    let mut bad = cfg("[types]\nSnippet = 1");
    reason(set_location(&mut bad, Snippet, None));
    assert_eq!(bad.rest, cfg("[types]\nSnippet = 1").rest);
}

#[test]
fn survives_write_machine_round_trip() {
    let d = std::env::temp_dir().join(format!("mda-loc-rt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let mut c = MachineConfig::default();
    let l = Location {
        vault: Some(abs()),
        place: Where::Dir("Lib".into()),
    };
    set_location(&mut c, Snippet, Some(l.clone())).unwrap();
    mda_vault::config::write_machine(&d, &c).unwrap();
    let back = mda_vault::config::read_machine(&d).unwrap();
    assert_eq!(location(&back, Snippet).unwrap(), Some(l));
    let _ = std::fs::remove_dir_all(&d);
}
