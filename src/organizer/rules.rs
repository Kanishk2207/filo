//! Keyword/extension → category lookup.

use std::collections::BTreeMap;
use std::path::Path;

use crate::config::KeywordRule;

pub struct RuleSet<'a> {
    rules: &'a BTreeMap<String, Vec<String>>,
    keyword_rules: &'a [KeywordRule],
    fallback: &'a str,
}

impl<'a> RuleSet<'a> {
    pub fn new(
        rules: &'a BTreeMap<String, Vec<String>>,
        keyword_rules: &'a [KeywordRule],
        fallback: &'a str,
    ) -> Self {
        Self {
            rules,
            keyword_rules,
            fallback,
        }
    }

    /// Resolve the category for a given path.
    ///
    /// Matching is case-insensitive on both filename keywords and extension.
    /// Keywords are checked first; if none match, extension rules are used.
    /// If no rule matches, the configured fallback category is returned.
    pub fn category_for(&self, path: &Path) -> &str {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        for rule in self.keyword_rules {
            let target = rule.to.trim();
            if target.is_empty() {
                continue;
            }
            if rule
                .keywords
                .iter()
                .map(|k| k.trim())
                .filter(|k| !k.is_empty())
                .any(|k| name.contains(&k.to_ascii_lowercase()))
            {
                return target;
            }
        }

        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e.to_lowercase(),
            None => return self.fallback,
        };
        for (category, exts) in self.rules {
            if exts.iter().any(|e| e.eq_ignore_ascii_case(&ext)) {
                return category;
            }
        }
        self.fallback
    }
}
