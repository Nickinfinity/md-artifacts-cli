//! Config files (spec §12): config dir resolution, machine file, vault file. Rejections assert the
//! sink (bytes on disk unchanged, nothing written outside), not only the returned error.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a panic is the failure signal

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use mda_core::registry::ArtifactType;
use mda_vault::config::{
    MachineConfig, VaultConfig, config_dir, read_machine, read_vault_config, write_machine,
    write_vault_config,
};
use mda_vault::{Root, VaultError, content_hash};

/// Fresh temp dir, removed on drop (std only).
struct Tmp(PathBuf);

impl Tmp {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("mda-cfgt-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn p(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A Unix-looking absolute literal made absolute on the running OS (`/x` -> `C:/x` on Windows,
/// where `/x` has no drive and is rightly refused). Component compare treats `/` and `\` alike.
fn abs(unix: &str) -> String {
    if cfg!(windows) {
        format!("C:{unix}")
    } else {
        unix.to_string()
    }
}

fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
    move |k| {
        pairs
            .iter()
            .find(|(n, _)| *n == k)
            .map(|(_, v)| OsString::from(v))
    }
}

#[test]
fn config_dir_table() {
    let (win_appdata, win_want) = if cfg!(windows) {
        ("C:\\a", PathBuf::from("C:\\a").join("mda"))
    } else {
        ("/a", PathBuf::from("/a/mda"))
    };
    type Case<'a> = (&'a str, Vec<(&'a str, &'a str)>, Option<PathBuf>);
    let cases: Vec<Case> = vec![
        ("linux", vec![("MDA_CONFIG_DIR", "/x")], Some("/x".into())),
        ("macos", vec![("MDA_CONFIG_DIR", "/x")], Some("/x".into())),
        (
            "linux",
            vec![("MDA_CONFIG_DIR", "rel"), ("HOME", "/h")],
            None,
        ),
        (
            "linux",
            vec![("MDA_CONFIG_DIR", ""), ("HOME", "/h")],
            Some("/h/.config/mda".into()),
        ),
        (
            "macos",
            vec![("HOME", "/h")],
            Some("/h/Library/Application Support/mda".into()),
        ),
        ("windows", vec![("APPDATA", win_appdata)], Some(win_want)),
        (
            "linux",
            vec![("XDG_CONFIG_HOME", "/x")],
            Some("/x/mda".into()),
        ),
        (
            "linux",
            vec![("XDG_CONFIG_HOME", "rel"), ("HOME", "/h")],
            Some("/h/.config/mda".into()),
        ),
        ("linux", vec![], None),
        ("macos", vec![("HOME", "rel")], None),
    ];
    for (os, env, want) in cases {
        // Only the Unix-style literals (leading `/`) are rewritten; `rel`, `""` and Windows ones stay.
        let env: Vec<(&str, String)> = env
            .iter()
            .map(|(k, v)| {
                (
                    *k,
                    if v.starts_with('/') {
                        abs(v)
                    } else {
                        (*v).into()
                    },
                )
            })
            .collect();
        let want = want.map(|w| match w.to_str() {
            Some(u) if u.starts_with('/') => PathBuf::from(abs(u)),
            _ => w,
        });
        let pairs: Vec<(&str, &str)> = env.iter().map(|(k, v)| (*k, v.as_str())).collect();
        assert_eq!(config_dir(os, env_of(&pairs)), want, "{os} {env:?}");
    }
}

fn cfg_path(dir: &Path) -> PathBuf {
    dir.join("config.toml")
}

fn invalid_at(r: Result<MachineConfig, VaultError>, dir: &Path) {
    let want = cfg_path(dir);
    assert!(
        matches!(&r, Err(VaultError::ConfigInvalid { path }) if *path == want),
        "want ConfigInvalid at {want:?}, got {r:?}"
    );
}

#[test]
fn read_machine_missing_and_valid() {
    let t = Tmp::new("rm-ok");
    let missing = t.p("nope");
    assert_eq!(read_machine(&missing).unwrap(), MachineConfig::default());
    let dir = t.p("c");
    fs::create_dir_all(&dir).unwrap();
    assert_eq!(read_machine(&dir).unwrap(), MachineConfig::default()); // dir, no file
    // `/` is valid in a TOML basic string and absolute on both OSes once given a drive.
    let v = abs("/v");
    let bytes = format!("default = \"{v}\"\nvaults = [\"{v}\"]\nx = 1\n").into_bytes();
    fs::write(cfg_path(&dir), &bytes).unwrap();
    let c = read_machine(&dir).unwrap();
    assert_eq!(c.default.as_deref(), Some(v.as_str()));
    assert_eq!(c.vaults, vec![v.clone()]);
    assert_eq!(c.hash, Some(content_hash(&bytes)));
    assert_eq!(c.rest["x"].as_integer(), Some(1));
}

#[test]
fn read_machine_rejects_bad_files() {
    let long = format!("/{}", "a".repeat(4096));
    let many: String = (0..65).map(|i| format!("\"/v{i}\",")).collect();
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("corrupt", b"default = [".to_vec()),
        ("utf8", b"a = \"\xff\"\n".to_vec()),
        ("big", vec![b'#'; 65_537]),
        ("notin", b"default = \"/a\"\nvaults = [\"/b\"]\n".to_vec()),
        ("rel", b"vaults = [\"rel\"]\n".to_vec()),
        ("sixtyfive", format!("vaults = [{many}]\n").into_bytes()),
        ("longentry", format!("vaults = [\"{long}\"]\n").into_bytes()),
        ("str", b"vaults = \"x\"\n".to_vec()),
        ("defnum", b"default = 1\nvaults = []\n".to_vec()),
        ("defonly", b"default = \"/a\"\n".to_vec()),
    ];
    for (name, bytes) in cases {
        let t = Tmp::new(&format!("rm-{name}"));
        fs::write(cfg_path(&t.0), &bytes).unwrap();
        invalid_at(read_machine(&t.0), &t.0);
    }
}

#[test]
fn write_machine_round_trip() {
    let t = Tmp::new("wm");
    let dir = t.p("new/cfg"); // missing dir is created
    let v = abs("/v");
    let cfg = MachineConfig {
        default: Some(v.clone()),
        vaults: vec![v.clone()],
        ..Default::default()
    };
    write_machine(&dir, &cfg).unwrap();
    assert_eq!(
        fs::read(cfg_path(&dir)).unwrap(),
        format!("default = \"{v}\"\nvaults = [\"{v}\"]\n").into_bytes()
    );
    let mut back = read_machine(&dir).unwrap();
    assert_eq!(back.default, cfg.default);
    back.rest.insert("x".into(), 1.into());
    write_machine(&dir, &back).unwrap();
    assert_eq!(
        fs::read(cfg_path(&dir)).unwrap(),
        format!("default = \"{v}\"\nvaults = [\"{v}\"]\nx = 1\n").into_bytes()
    );
}

#[test]
fn write_machine_conflicts_leave_other_writer_bytes() {
    let t = Tmp::new("wm-conf");
    let dir = t.p("c");
    let cfg = MachineConfig::default();
    write_machine(&dir, &cfg).unwrap();
    let read = read_machine(&dir).unwrap();
    fs::write(cfg_path(&dir), b"other = 1\n").unwrap(); // another writer
    assert!(matches!(
        write_machine(&dir, &read),
        Err(VaultError::Conflict { .. })
    ));
    assert_eq!(fs::read(cfg_path(&dir)).unwrap(), b"other = 1\n");
    // hash None with a file present
    assert!(matches!(
        write_machine(&dir, &MachineConfig::default()),
        Err(VaultError::Conflict { .. })
    ));
    assert_eq!(fs::read(cfg_path(&dir)).unwrap(), b"other = 1\n");
}

fn vault_with(t: &Tmp, mda: Option<&str>) -> Root {
    let v = t.p("vault");
    fs::create_dir_all(v.join(".obsidian")).unwrap();
    if let Some(c) = mda {
        fs::create_dir_all(v.join(".mda")).unwrap();
        fs::write(v.join(".mda/config.toml"), c).unwrap();
    }
    Root::new(&v).unwrap()
}

const CFG: &str =
    "[future]\nkeep = 1\n\n[types.Command]\nenabled = true\n\n[types.Snippet]\nenabled = false\n";

#[test]
fn vault_config_reads_and_preserves() {
    let t = Tmp::new("vc");
    let none = vault_with(&t, None);
    assert_eq!(read_vault_config(&none).unwrap(), VaultConfig::default());
    let t2 = Tmp::new("vc2");
    let root = vault_with(&t2, Some(CFG));
    let mut c = read_vault_config(&root).unwrap();
    assert_eq!(c.enabled(ArtifactType::Snippet), Some(false));
    assert_eq!(c.enabled(ArtifactType::Command), Some(true));
    assert_eq!(c.enabled(ArtifactType::Template), None);
    c.set_enabled(ArtifactType::Template, true);
    write_vault_config(&root, &c).unwrap();
    let out = fs::read_to_string(t2.p("vault/.mda/config.toml")).unwrap();
    assert!(out.contains("keep = 1"), "{out}");
    let back = read_vault_config(&root).unwrap();
    assert_eq!(back.enabled(ArtifactType::Template), Some(true));
    assert_eq!(back.enabled(ArtifactType::Snippet), Some(false));
    assert_eq!(back.enabled(ArtifactType::Command), Some(true));
}

#[test]
fn vault_config_first_write_creates_mda() {
    let t = Tmp::new("vc-first");
    let root = vault_with(&t, None);
    let mut c = VaultConfig::default();
    c.set_enabled(ArtifactType::Command, true);
    write_vault_config(&root, &c).unwrap();
    assert_eq!(
        fs::read_to_string(t.p("vault/.mda/config.toml")).unwrap(),
        "[types.Command]\nenabled = true\n"
    );
}

#[test]
fn vault_config_mda_is_a_file() {
    let t = Tmp::new("vc-file");
    let root = vault_with(&t, None);
    fs::write(t.p("vault/.mda"), b"x").unwrap();
    assert!(matches!(
        read_vault_config(&root),
        Err(VaultError::NotDir { .. })
    ));
    assert!(matches!(
        write_vault_config(&root, &VaultConfig::default()),
        Err(VaultError::NotDir { .. })
    ));
    assert_eq!(fs::read(t.p("vault/.mda")).unwrap(), b"x");
}

#[cfg(unix)]
#[test]
fn vault_config_mda_symlink_out() {
    let t = Tmp::new("vc-link");
    let root = vault_with(&t, None);
    fs::create_dir_all(t.p("outside")).unwrap();
    std::os::unix::fs::symlink(t.p("outside"), t.p("vault/.mda")).unwrap();
    assert!(matches!(
        read_vault_config(&root),
        Err(VaultError::OutsideRoot { .. })
    ));
    let mut c = VaultConfig::default();
    c.set_enabled(ArtifactType::Command, true);
    assert!(matches!(
        write_vault_config(&root, &c),
        Err(VaultError::OutsideRoot { .. })
    ));
    assert_eq!(fs::read_dir(t.p("outside")).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn vault_config_fifo_never_blocks() {
    let t = Tmp::new("vc-fifo");
    let root = vault_with(&t, None);
    fs::create_dir_all(t.p("vault/.mda")).unwrap();
    let fifo = t.p("vault/.mda/config.toml");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(read_vault_config(&root));
    });
    let r = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("blocked on FIFO");
    assert!(matches!(r, Err(VaultError::NotRegular { .. })));
}

#[test]
fn vault_config_bad_shapes_and_corrupt_never_overwritten() {
    for (name, body) in [
        ("types-not-table", "[types]\nSnippet = false\n"),
        ("notable", "types = { Snippet = false }\n"),
        ("enabled-str", "[types.Snippet]\nenabled = \"yes\"\n"),
        ("corrupt", "[types\n"),
    ] {
        let t = Tmp::new(&format!("vc-bad-{name}"));
        let root = vault_with(&t, Some(body));
        let r = read_vault_config(&root);
        match r {
            Err(VaultError::ConfigInvalid { path }) => {
                assert_eq!(path, root.path().join(".mda/config.toml"));
            }
            other => panic!("{name}: {other:?}"),
        }
        // A blind write (no hash) must not clobber the corrupt file.
        let mut c = VaultConfig::default();
        c.set_enabled(ArtifactType::Command, true);
        assert!(write_vault_config(&root, &c).is_err());
        assert_eq!(
            fs::read_to_string(t.p("vault/.mda/config.toml")).unwrap(),
            body
        );
    }
    let t = Tmp::new("vc-types-scalar");
    let root = vault_with(&t, Some("types = 1\n"));
    assert!(matches!(
        read_vault_config(&root),
        Err(VaultError::ConfigInvalid { .. })
    ));
}

#[test]
fn hostile_toml_is_bounded() {
    let n = 60_000;
    let bodies = [
        format!("a={}", "[".repeat(n)),
        format!("a={}", "{a=".repeat(n / 3)),
        format!("[{}]\n", "a.".repeat(n / 2) + "a"),
        format!("{}=1\n", "a.".repeat(n / 2) + "a"),
    ];
    for (i, body) in bodies.iter().enumerate() {
        assert!(body.len() <= 65_536);
        let t = Tmp::new(&format!("hostile-{i}"));
        fs::write(cfg_path(&t.0), body).unwrap();
        let start = Instant::now();
        invalid_at(read_machine(&t.0), &t.0);
        assert!(start.elapsed() < Duration::from_secs(1), "case {i} slow");
    }
}
