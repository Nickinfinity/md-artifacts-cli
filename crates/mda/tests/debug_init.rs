//! One test only: the `log` facade is process-global.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code

use mda::debug::{DebugLevel, init};

#[test]
fn logger_writes_warning_then_lines_and_bad_path_errs() {
    let dir = std::env::temp_dir().join(format!("mda-{}-debug_init", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("no-such-dir").join("x.log");

    // Unopenable file: Err before anything is installed.
    assert!(init(DebugLevel::Debug, Some(&bad)).is_err());
    // Off never opens the file.
    assert!(init(DebugLevel::Off, Some(&bad)).is_ok());
    assert!(!bad.exists());

    let file = dir.join("out.log");
    init(DebugLevel::Trace, Some(&file)).unwrap();
    log::debug!(target: "mda::t", "hello world");

    let text = std::fs::read_to_string(&file).unwrap();
    let mut lines = text.lines();
    let first = lines.next().unwrap();
    assert!(first.to_lowercase().contains("secret"), "first: {first}");
    let second = lines.next().unwrap();
    let parts: Vec<&str> = second.splitn(4, ' ').collect();
    assert_eq!(parts.len(), 4, "second: {second}");
    assert!(
        parts[0].ends_with('Z') && parts[0].len() == 20,
        "ts: {}",
        parts[0]
    );
    assert_eq!(&parts[1..], ["DEBUG", "mda::t", "hello world"]);

    // Control chars in the message are escaped: one line, no ESC byte.
    log::debug!(target: "mda::t", "a\nb\x1bc\r");
    let bytes = std::fs::read(&file).unwrap();
    assert!(!bytes.contains(&0x1b), "raw ESC leaked");
    assert!(!bytes.contains(&b'\r'), "raw CR leaked");
    assert_eq!(bytes.iter().filter(|&&b| b == b'\n').count(), 3);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
