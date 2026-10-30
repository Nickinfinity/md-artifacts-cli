//! D-29: every stem the naming rule accepts is created, read back and renamed on the real
//! filesystem of each CI OS (the per-OS proof).
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a panic is the failure signal

use std::fs;
use std::path::Path;

use mda_core::naming::ACCEPTED_STEMS;
use mda_vault::{Root, content_hash, create_new, read_bounded, rename_new};

#[test]
fn accepted_stems_round_trip_on_disk() {
    let dir = std::env::temp_dir().join(format!("mda-names-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("Snippets")).unwrap();
    fs::create_dir_all(dir.join("Moved")).unwrap();
    let root = Root::new(&dir).unwrap();
    for stem in ACCEPTED_STEMS {
        let rel = format!("Snippets/{stem}.md");
        let bytes = format!("body of {stem}\n").into_bytes();
        create_new(&root, Path::new(&rel), &bytes).unwrap();
        assert_eq!(
            read_bounded(&root, Path::new(&rel), 1 << 20).unwrap(),
            bytes
        );
        let to = format!("Moved/{stem}.md");
        rename_new(
            &root,
            Path::new(&rel),
            Path::new(&to),
            &content_hash(&bytes),
            None,
            1 << 20,
        )
        .unwrap();
        assert!(read_bounded(&root, Path::new(&rel), 1 << 20).is_err());
        assert_eq!(read_bounded(&root, Path::new(&to), 1 << 20).unwrap(), bytes);
    }
    let _ = fs::remove_dir_all(&dir);
}
