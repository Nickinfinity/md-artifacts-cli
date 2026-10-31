//! W7d (spec §9): the defaults fence is ```` ```yaml VKs ```` only. A pre-W7d ```` ```vks ```` fence or an
//! untagged ```` ```yaml ```` fence holding `VK-` keys is not read as defaults, and `vault.validate`
//! says so; a real YAML code snippet is neither read as defaults nor warned about.
#![allow(clippy::unwrap_used)] // reason: setup helpers outside #[test] fns

use std::fs;
use std::path::PathBuf;

use mda_ops::artifact::{ReadRequest, read};
use mda_ops::validate::{ValidateRequest, validate};
use mda_ops::{Ctx, Root};

fn vault(files: &[(&str, &str)]) -> (PathBuf, Ctx) {
    let d = std::env::temp_dir().join(format!("mda-defaults-fence-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join(".obsidian")).unwrap();
    for (rel, text) in files {
        let p = d.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    let d = d.canonicalize().unwrap();
    let ctx = Ctx::new(Some(Root::new(&d).unwrap()));
    (d, ctx)
}

const OLD: &str =
    "---\nartifactType: Snippet\n---\n\n```js\nf(<VK-a>)\n```\n\nvars:\n```vks\nVK-a: x\n```\n";
const UNTAGGED: &str =
    "---\nartifactType: Snippet\n---\n\n```js\nf(<VK-a>)\n```\n\nvars:\n```yaml\nVK-a: x\n```\n";
const TAGGED: &str = "---\nartifactType: Snippet\n---\n\n```js\nf(<VK-a>)\n```\n\nvars:\n```yaml VKs\nVK-a: x\n```\n";
const COMPOSE: &str =
    "---\nartifactType: Snippet\n---\n\n```yaml\nservices:\n  app:\n    image: <VK-image>\n```\n";

#[test]
fn only_yaml_vks_supplies_defaults_and_validate_names_the_rest() {
    let (d, ctx) = vault(&[
        ("Snippets/old.md", OLD),
        ("Snippets/untagged.md", UNTAGGED),
        ("Snippets/tagged.md", TAGGED),
        ("Snippets/compose.md", COMPOSE),
    ]);
    let vars = |p: &str| {
        let r = read(&ctx, ReadRequest { path: p.into() }).unwrap();
        serde_json::to_value(&r.artifact.vars).unwrap()
    };
    let x = serde_json::json!([{"name": "VK-a", "defaultValue": "x"}]);
    // A fenced file's top-level vars are its declared defaults only (R-25(1)): none here.
    let empty = serde_json::json!([]);
    assert_eq!(vars("Snippets/tagged.md"), x);
    assert_eq!(vars("Snippets/old.md"), empty);
    assert_eq!(vars("Snippets/untagged.md"), empty);
    let compose = read(
        &ctx,
        ReadRequest {
            path: "Snippets/compose.md".into(),
        },
    )
    .unwrap();
    assert!(compose.artifact.code.starts_with("services:"));

    let r = validate(&ctx, ValidateRequest {}).unwrap();
    let mut got: Vec<_> = r
        .issues
        .iter()
        .filter(|i| i.code == "vars.untagged_defaults")
        .map(|i| (i.path.as_str(), i.params["info"].as_str()))
        .collect();
    got.sort_unstable();
    assert_eq!(
        got,
        [("Snippets/old.md", "vks"), ("Snippets/untagged.md", "yaml")]
    );
    let _ = fs::remove_dir_all(d);
}
