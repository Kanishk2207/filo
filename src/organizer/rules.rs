//! Keyword/extension → destination lookup.
//!
//! Matching and path resolution are deliberately separate. [`RuleSet`] only
//! decides *which* rule wins and returns the raw `to` value plus how to read
//! it; [`resolve_destination`] turns that into a concrete directory. Keeping
//! them apart means matching stays pure and testable, and every caller
//! resolves paths the same way.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::config::{DestinationKind, KeywordRule};

/// The winning rule's destination, before it is turned into a real directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Route<'a> {
    /// Folder name, or path when `kind` is [`DestinationKind::Path`].
    pub to: &'a str,
    pub kind: DestinationKind,
}

impl<'a> Route<'a> {
    fn folder(to: &'a str) -> Self {
        Self {
            to,
            kind: DestinationKind::Folder,
        }
    }
}

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

    /// Resolve the destination for a given path.
    ///
    /// Matching is case-insensitive on both filename keywords and extension.
    /// Keywords are checked first; if none match, extension rules are used.
    /// If no rule matches, the configured fallback category is returned.
    ///
    /// Extension rules and the fallback always produce a folder route —
    /// only keyword rules can name a path.
    pub fn route_for(&self, path: &Path) -> Route<'a> {
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
                return Route {
                    to: target,
                    kind: rule.to_type,
                };
            }
        }

        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e.to_lowercase(),
            None => return Route::folder(self.fallback),
        };
        for (category, exts) in self.rules {
            if exts.iter().any(|e| e.eq_ignore_ascii_case(&ext)) {
                return Route::folder(category);
            }
        }
        Route::folder(self.fallback)
    }
}

/// Turn a [`Route`] into the directory a matched file should land in.
///
/// - Folder routes become `<root>/<to>`, exactly as before.
/// - Path routes expand a leading `~` to the home directory, and anchor
///   anything still relative to `root`.
///
/// The directory does not have to exist; it is created at move time.
pub fn resolve_destination(route: &Route, root: &Path) -> PathBuf {
    match route.kind {
        DestinationKind::Folder => root.join(route.to),
        DestinationKind::Path => resolve_path(route.to, root, dirs::home_dir().as_deref()),
    }
}

/// `resolve_destination`'s path arm, with the home directory injected so it
/// can be tested without touching the real one.
fn resolve_path(raw: &str, root: &Path, home: Option<&Path>) -> PathBuf {
    let expanded = expand_home(raw.trim(), home);
    if expanded.is_absolute() {
        expanded
    } else {
        root.join(expanded)
    }
}

fn expand_home(raw: &str, home: Option<&Path>) -> PathBuf {
    let rest = if raw == "~" {
        Some("")
    } else {
        raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\"))
    };

    let Some(rest) = rest else {
        return PathBuf::from(raw);
    };

    match home {
        Some(home) if rest.is_empty() => home.to_path_buf(),
        Some(home) => home.join(rest),
        None => {
            // No home directory to expand against. Leave the `~` literal
            // rather than guessing, and say so — a folder actually named
            // `~` is a visible symptom the user can act on.
            log::warn!(
                "cannot expand '~' in rule destination '{}': no home directory found",
                raw
            );
            PathBuf::from(raw)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{resolve_destination, resolve_path, Route, RuleSet};
    use crate::config::{DestinationKind, KeywordRule};

    fn home() -> PathBuf {
        PathBuf::from("/home/tester")
    }

    #[test]
    fn folder_route_joins_the_watch_root() {
        let route = Route {
            to: "kanishk-itr",
            kind: DestinationKind::Folder,
        };
        assert_eq!(
            resolve_destination(&route, Path::new("/downloads")),
            Path::new("/downloads/kanishk-itr")
        );
    }

    #[test]
    fn path_route_expands_leading_tilde() {
        assert_eq!(
            resolve_path(
                "~/personal/kanishk-itr",
                Path::new("/downloads"),
                Some(&home())
            ),
            home().join("personal/kanishk-itr")
        );
    }

    #[test]
    fn path_route_keeps_absolute_paths_untouched() {
        assert_eq!(
            resolve_path("/archive/tax/2026", Path::new("/downloads"), Some(&home())),
            Path::new("/archive/tax/2026")
        );
    }

    #[test]
    fn path_route_anchors_relative_paths_to_the_watch_root() {
        assert_eq!(
            resolve_path("tax/2026", Path::new("/downloads"), Some(&home())),
            Path::new("/downloads/tax/2026")
        );
    }

    #[test]
    fn path_route_leaves_tilde_literal_without_a_home_dir() {
        assert_eq!(
            resolve_path("~/personal", Path::new("/downloads"), None),
            Path::new("/downloads/~/personal")
        );
    }

    #[test]
    fn tilde_alone_resolves_to_the_home_dir() {
        assert_eq!(
            resolve_path("~", Path::new("/downloads"), Some(&home())),
            home()
        );
    }

    #[test]
    fn tilde_inside_a_path_is_not_expanded() {
        assert_eq!(
            resolve_path("/archive/~backup", Path::new("/downloads"), Some(&home())),
            Path::new("/archive/~backup")
        );
    }

    #[test]
    fn keyword_rule_carries_its_destination_kind() {
        let rules = crate::config::defaults::default_rules();
        let keyword_rules = vec![KeywordRule::path(
            "~/personal/kanishk-itr",
            vec!["itr".into()],
        )];
        let set = RuleSet::new(&rules, &keyword_rules, "Other");

        let route = set.route_for(Path::new("/downloads/ITR-2026.pdf"));
        assert_eq!(route.kind, DestinationKind::Path);
        assert_eq!(route.to, "~/personal/kanishk-itr");
    }

    #[test]
    fn extension_match_is_always_a_folder_route() {
        let rules = crate::config::defaults::default_rules();
        let keyword_rules: Vec<KeywordRule> = Vec::new();
        let set = RuleSet::new(&rules, &keyword_rules, "Other");

        let route = set.route_for(Path::new("/downloads/report.pdf"));
        assert_eq!(route.kind, DestinationKind::Folder);
        assert_eq!(route.to, "Documents");
    }
}
