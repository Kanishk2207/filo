//! Extension → category lookup.

use std::collections::BTreeMap;
use std::path::Path;

pub struct RuleSet<'a> {
    rules: &'a BTreeMap<String, Vec<String>>,
    fallback: &'a str,
}

impl<'a> RuleSet<'a> {
    pub fn new(rules: &'a BTreeMap<String, Vec<String>>, fallback: &'a str) -> Self {
        Self { rules, fallback }
    }

    /// Resolve the category for a given path.
    ///
    /// Matching is case-insensitive on the extension. If the file has no
    /// extension or no rule matches, the configured fallback category is
    /// returned.
    pub fn category_for(&self, path: &Path) -> &str {
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
