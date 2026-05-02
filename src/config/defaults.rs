//! Built-in defaults for categorization rules and special folder names.
//!
//! These are the starting point emitted by `filo init`. Every value is
//! overridable by editing `config.toml` directly — nothing here is enforced
//! at runtime.

use std::collections::BTreeMap;

pub const DEFAULT_OTHER_CATEGORY: &str = "Other";
pub const DEFAULT_DUPLICATES_FOLDER: &str = "Duplicates";
pub const DEFAULT_DEBOUNCE_MS: u64 = 2000;

/// Default category → extension mapping.
///
/// `BTreeMap` gives a stable on-disk ordering, which keeps diffs of an
/// edited `config.toml` clean.
pub fn default_rules() -> BTreeMap<String, Vec<String>> {
    let seed: &[(&str, &[&str])] = &[
        (
            "Documents",
            &["pdf", "doc", "docx", "odt", "rtf", "txt", "md", "epub"],
        ),
        (
            "Images",
            &[
                "jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "tiff", "heic", "avif",
            ],
        ),
        (
            "Videos",
            &["mp4", "mkv", "mov", "avi", "webm", "flv", "wmv", "m4v"],
        ),
        (
            "Archives",
            &["zip", "tar", "gz", "bz2", "xz", "rar", "7z", "tgz"],
        ),
        (
            "Audio",
            &["mp3", "flac", "wav", "ogg", "m4a", "aac", "opus"],
        ),
        (
            "Code",
            &[
                "rs", "py", "js", "ts", "go", "c", "cpp", "h", "hpp", "java", "rb", "sh", "toml",
                "json", "yaml", "yml",
            ],
        ),
    ];

    seed.iter()
        .map(|(cat, exts)| {
            (
                (*cat).to_string(),
                exts.iter().map(|e| (*e).to_string()).collect(),
            )
        })
        .collect()
}
