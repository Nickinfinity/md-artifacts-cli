//! THE language tables: editor-language aliases, fence names and file extensions (ports the
//! extension's `types/constants.ts` `LANG_ALIAS` / `LANG_FENCE` / `LANG_EXT` and
//! `language-map.service.ts`). Resolving against an editor's known-language list stays client-side.

// Tables are slices searched with `find`, so JS prototype keys (`constructor`, `__proto__`) miss.

/// Shorthand → canonical id (26 rows).
pub const LANG_ALIAS: &[(&str, &str)] = &[
    ("js", "javascript"),
    ("node", "javascript"),
    ("mjs", "javascript"),
    ("cjs", "javascript"),
    ("jsx", "javascriptreact"),
    ("ts", "typescript"),
    ("tsx", "typescriptreact"),
    ("py", "python"),
    ("py3", "python"),
    ("rb", "ruby"),
    ("rs", "rust"),
    ("golang", "go"),
    ("sh", "shellscript"),
    ("shell", "shellscript"),
    ("bash", "shellscript"),
    ("zsh", "shellscript"),
    ("yml", "yaml"),
    ("md", "markdown"),
    ("c++", "cpp"),
    ("c#", "csharp"),
    ("cs", "csharp"),
    ("kt", "kotlin"),
    ("objc", "objective-c"),
    ("objcpp", "objective-cpp"),
    ("ps1", "powershell"),
    ("htm", "html"),
];

/// Canonical id → fence string, only where it differs (6 rows).
pub const LANG_FENCE: &[(&str, &str)] = &[
    ("typescriptreact", "tsx"),
    ("javascriptreact", "jsx"),
    ("shellscript", "bash"),
    ("dockerfile", "dockerfile"),
    ("objective-c", "objc"),
    ("objective-cpp", "objcpp"),
];

/// Canonical id → file extension (29 rows).
pub const LANG_EXT: &[(&str, &str)] = &[
    ("javascript", "js"),
    ("javascriptreact", "jsx"),
    ("typescript", "ts"),
    ("typescriptreact", "tsx"),
    ("python", "py"),
    ("ruby", "rb"),
    ("rust", "rs"),
    ("go", "go"),
    ("java", "java"),
    ("csharp", "cs"),
    ("cpp", "cpp"),
    ("c", "c"),
    ("kotlin", "kt"),
    ("swift", "swift"),
    ("php", "php"),
    ("shellscript", "sh"),
    ("powershell", "ps1"),
    ("yaml", "yml"),
    ("json", "json"),
    ("html", "html"),
    ("css", "css"),
    ("scss", "scss"),
    ("sql", "sql"),
    ("markdown", "md"),
    ("xml", "xml"),
    ("objective-c", "m"),
    ("objective-cpp", "mm"),
    ("dockerfile", "dockerfile"),
    ("plaintext", "txt"),
];

fn lookup(t: &'static [(&'static str, &'static str)], k: &str) -> Option<&'static str> {
    t.iter().find(|(key, _)| *key == k).map(|(_, v)| *v)
}

/// Map an editor language id to the artifact language (`typescriptreact` → `tsx`); unknown ids pass
/// through.
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::language::map_language_id("typescriptreact"), "tsx");
/// assert_eq!(mda_core::language::map_language_id("rust"), "rust");
/// ```
pub fn map_language_id(id: &str) -> &str {
    lookup(LANG_FENCE, id).unwrap_or(id)
}

/// Normalise a raw language name (Unicode lowercase, then alias) to its canonical id.
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::language::normalize_lang_id("JS"), "javascript");
/// assert_eq!(mda_core::language::normalize_lang_id("Zig"), "zig");
/// ```
pub fn normalize_lang_id(raw: &str) -> String {
    let lc = raw.to_lowercase();
    lookup(LANG_ALIAS, &lc).map_or(lc, str::to_owned)
}

/// The file extension (no dot) for a canonical language id: the table, else the id when it is
/// non-empty `[a-z0-9]`, else `txt`.
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::language::ext_for_lang("javascript"), "js");
/// assert_eq!(mda_core::language::ext_for_lang("zig"), "zig");
/// assert_eq!(mda_core::language::ext_for_lang("weird-id!"), "txt");
/// ```
pub fn ext_for_lang(id: &str) -> &str {
    if let Some(e) = lookup(LANG_EXT, id) {
        return e;
    }
    let safe = !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    if safe { id } else { "txt" }
}
