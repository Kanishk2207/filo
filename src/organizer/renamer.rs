//! Smart renaming: deterministic normalization of file stems.
//!
//! The transform operates on the stem only (the extension is preserved
//! verbatim by the caller). Steps run in a fixed order so the result is
//! reproducible for any given input.

use crate::config::RenameConfig;

/// Apply the rename pipeline to `stem` according to `config`.
///
/// If the pipeline would produce an empty string, the original stem is
/// returned unchanged — we never generate nameless files.
pub fn smart_rename(stem: &str, config: &RenameConfig) -> String {
    let mut s = stem.to_string();

    if config.strip_redundant_words {
        s = strip_redundant(&s);
    }
    if config.lowercase_with_hyphens {
        s = normalize(&s);
    }
    if config.prepend_date {
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        s = format!("{}-{}", date, s);
    }

    if s.is_empty() {
        stem.to_string()
    } else {
        s
    }
}

/// Collapse consecutive repeated tokens (case-insensitive).
///
/// `FINAL_FINAL_report` → `FINAL-report`, `copy copy draft` → `copy-draft`.
/// Only _adjacent_ duplicates are removed — distant repetitions are kept so
/// we don't destroy legitimate patterns like `report-2025-report`.
fn strip_redundant(s: &str) -> String {
    let tokens: Vec<&str> = s.split(['_', ' ', '-']).filter(|t| !t.is_empty()).collect();

    let mut dedup: Vec<&str> = Vec::with_capacity(tokens.len());
    for tok in tokens {
        let is_repeat = dedup
            .last()
            .map(|last| last.eq_ignore_ascii_case(tok))
            .unwrap_or(false);
        if !is_repeat {
            dedup.push(tok);
        }
    }
    dedup.join("-")
}

/// Lowercase and join non-alphanumerics with a single hyphen.
///
/// Dots are preserved so double-extension patterns like `archive.tar` (stem
/// of `archive.tar.gz`) survive the pass.
fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_was_sep = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_was_sep = false;
        } else if c == '.' {
            out.push('.');
            last_was_sep = false;
        } else if !last_was_sep && !out.is_empty() {
            out.push('-');
            last_was_sep = true;
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> RenameConfig {
        RenameConfig {
            enabled: true,
            lowercase_with_hyphens: true,
            strip_redundant_words: true,
            prepend_date: false,
        }
    }

    #[test]
    fn collapses_repeated_tokens() {
        assert_eq!(smart_rename("FINAL_FINAL_report", &cfg()), "final-report");
    }

    #[test]
    fn normalizes_spaces_and_case() {
        assert_eq!(smart_rename("My  Cool File", &cfg()), "my-cool-file");
    }

    #[test]
    fn preserves_dots_in_stem() {
        assert_eq!(smart_rename("archive.tar", &cfg()), "archive.tar");
    }

    #[test]
    fn falls_back_to_original_when_empty() {
        assert_eq!(smart_rename("___", &cfg()), "___");
    }
}
