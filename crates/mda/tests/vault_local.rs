//! Local-only parity against the user's real vault (Q-E declined, decision #27): vault files never
//! enter this public repo, so they are read where they live, through the real binary.
//!
//! Ignored by the gate. Run it explicitly after setting the vault:
//!
//! ```text
//! MDA_TEST_VAULT=/path/to/vault cargo test -p mda --test vault_local -- --ignored
//! ```
//!
//! Expectations are the TS parser's output for each file, generated locally (gitignored) by
//! `docs/plans/engine-port/conformance-gen/gen-parse.mjs --vault …`; `MDA_TEST_VAULT_EXPECTED`
//! overrides their default location `docs/plans/engine-port/vault-expected/`.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)] // reason: test fails loudly

mod common;

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Manifest {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    path: String,
    expected: String,
}

fn expected_dir() -> PathBuf {
    std::env::var_os("MDA_TEST_VAULT_EXPECTED").map_or_else(
        || {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../docs/plans/engine-port/vault-expected")
        },
        PathBuf::from,
    )
}

#[test]
#[ignore = "local only: needs MDA_TEST_VAULT (the real vault) and the locally generated expectations"]
fn real_vault_matches_ts() {
    let vault = std::env::var("MDA_TEST_VAULT")
        .expect("set MDA_TEST_VAULT to the vault root before running this local test");
    let dir = expected_dir();
    let text = std::fs::read_to_string(dir.join("manifest.parse.toml")).unwrap_or_else(|e| {
        panic!(
            "{}: {e} (generate it with conformance-gen/gen-parse.mjs --vault)",
            dir.display()
        )
    });
    let m: Manifest = toml::from_str(&text).unwrap();
    assert!(!m.cases.is_empty(), "no vault cases in {}", dir.display());
    let mut fails = Vec::new();
    for c in &m.cases {
        let out = common::run(
            &["--vault", &vault, "artifact", "show", &c.path, "--json"],
            b"",
        );
        let mut got = common::json_lines(&out.stdout)
            .into_iter()
            .next()
            .unwrap_or(Value::Null);
        // W2 adds `hash` to the read response; the TS expectations predate it.
        if let Some(o) = got.as_object_mut() {
            o.remove("hash");
        }
        let want: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join(&c.expected)).unwrap()).unwrap();
        if out.status.code() != Some(0) || got != want {
            fails.push(format!(
                "{} ({}): exit {:?}\n  got  {got}\n  want {want}",
                c.id,
                c.path,
                out.status.code()
            ));
        }
    }
    assert!(
        fails.is_empty(),
        "{} of {} differ:\n{}",
        fails.len(),
        m.cases.len(),
        fails.join("\n")
    );
}
