//! The organizer engine.
//!
//! A single file flows through three stages:
//!
//!   1. **Rule lookup** → pick a destination category (`rules`).
//!   2. **Plan** → compute where the file would go, including duplicate
//!      detection and collision handling (`mover`, `duplicates`, `renamer`).
//!   3. **Execute** → actually move it (`mover::safe_move`).
//!
//! The `plan` / `execute` split is what makes `filo preview` a zero-risk
//! dry run: the same planner runs but `execute` is never called.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::{Config, DuplicateAction};
use crate::errors::OrganizerError;

pub mod duplicates;
pub mod mover;
pub mod renamer;
pub mod rules;

/// A resolved intent to do something with a specific file.
#[derive(Debug, Clone)]
pub struct Plan {
    pub source: PathBuf,
    pub action: Action,
}

#[derive(Debug, Clone)]
pub enum Action {
    /// Move `source` into the resolved destination. Guaranteed non-colliding.
    Move { destination: PathBuf },
    /// `source` is a byte-identical duplicate of `of`; leave it where it is.
    SkipDuplicate { of: PathBuf },
    /// `source` is a duplicate of `of`; route it to the dedicated duplicates
    /// subfolder instead of the regular destination.
    MoveDuplicate { destination: PathBuf, of: PathBuf },
    /// The matched rule resolves to the directory `source` already sits in,
    /// so there is nothing to do. Only reachable via path destinations (a
    /// folder destination is always a subfolder of the watch root).
    AlreadyInPlace,
}

/// Compute a plan for `source`, whose containing watch folder is `root`.
///
/// By default destinations are subfolders of `root` named after the matched
/// category (e.g. `~/Downloads/Images/`). A keyword rule with
/// `to_type = "path"` instead sends matches to a path of its own, anywhere on
/// disk (e.g. `~/personal/kanishk-itr/`). Either way the source is never
/// touched until `execute`.
pub fn plan(source: &Path, root: &Path, config: &Config) -> Result<Plan, OrganizerError> {
    let ruleset = rules::RuleSet::new(
        &config.rules,
        &config.keyword_rules.rules,
        &config.other_category,
    );
    let route = ruleset.route_for(source);
    let dest_dir = rules::resolve_destination(&route, root);

    // A path rule can name the directory the file is already in. Moving a
    // file onto itself would otherwise read as a duplicate of itself and,
    // under `duplicates.action = "move"`, shunt it into a Duplicates
    // subfolder. Leave it alone instead.
    if is_same_dir(&dest_dir, source.parent()) {
        return Ok(Plan {
            source: source.to_path_buf(),
            action: Action::AlreadyInPlace,
        });
    }

    let (stem, ext) = split_stem_ext(source);
    let final_stem = if config.rename.enabled {
        renamer::smart_rename(&stem, &config.rename)
    } else {
        stem
    };
    let ext_ref = ext.as_deref();

    // Duplicate check, in order of specificity:
    //   (a) a file already exists at the exact target name → check its hash
    //   (b) otherwise, scan the destination directory for any file with the
    //       same size → hash-check only those candidates.
    let direct = mover::build_dest(&dest_dir, &final_stem, ext_ref);
    if direct.exists() && duplicates::are_duplicates(source, &direct)? {
        return Ok(Plan {
            source: source.to_path_buf(),
            action: resolve_duplicate(&direct, &dest_dir, &final_stem, ext_ref, config)?,
        });
    }
    if let Some(existing) = find_duplicate_in_dir(&dest_dir, source)? {
        return Ok(Plan {
            source: source.to_path_buf(),
            action: resolve_duplicate(&existing, &dest_dir, &final_stem, ext_ref, config)?,
        });
    }

    let destination = mover::unique_destination(&dest_dir, &final_stem, ext_ref)?;
    Ok(Plan {
        source: source.to_path_buf(),
        action: Action::Move { destination },
    })
}

pub fn execute(plan: &Plan) -> Result<(), OrganizerError> {
    match &plan.action {
        Action::Move { destination } => mover::safe_move(&plan.source, destination),
        Action::MoveDuplicate { destination, .. } => mover::safe_move(&plan.source, destination),
        Action::SkipDuplicate { .. } | Action::AlreadyInPlace => Ok(()),
    }
}

/// Whether `a` and `b` are the same directory on disk.
///
/// Compares resolved paths when both exist, so `~/Downloads` and a symlink
/// or `.`-laden spelling of it are recognized as one directory. Falls back to
/// a literal comparison when either side cannot be resolved (for example a
/// destination that has not been created yet).
fn is_same_dir(a: &Path, b: Option<&Path>) -> bool {
    let Some(b) = b else { return false };
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Returns true if the file should be ignored by organization logic.
///
/// Skips dotfiles (which are often system metadata) and common partial-
/// download markers (`.crdownload`, `.part`, etc.) that indicate the file
/// is still being written.
pub fn should_skip(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return true;
    };
    if name.starts_with('.') {
        return true;
    }
    const TEMP_SUFFIXES: &[&str] = &[".part", ".crdownload", ".download", ".tmp", ".partial"];
    let lower = name.to_ascii_lowercase();
    TEMP_SUFFIXES.iter().any(|s| lower.ends_with(s))
}

/// Poll `path` every `interval`, up to `checks` times, confirming the size
/// has stopped changing. Returns `Ok(true)` once two consecutive samples
/// match.
///
/// This is the second line of defense against acting on a file mid-write:
/// the watcher's event-debounce handles the common case, and this handles
/// anything that slips through.
pub fn wait_for_stable(
    path: &Path,
    checks: u32,
    interval: Duration,
) -> Result<bool, OrganizerError> {
    let mut last: Option<u64> = None;
    for _ in 0..checks {
        let size = std::fs::metadata(path)
            .map_err(|source| OrganizerError::Read {
                path: path.to_path_buf(),
                source,
            })?
            .len();
        if last == Some(size) {
            return Ok(true);
        }
        last = Some(size);
        std::thread::sleep(interval);
    }
    Ok(false)
}

fn find_duplicate_in_dir(dir: &Path, source: &Path) -> Result<Option<PathBuf>, OrganizerError> {
    if !dir.exists() {
        return Ok(None);
    }
    let source_size = std::fs::metadata(source)
        .map_err(|source_err| OrganizerError::Read {
            path: source.to_path_buf(),
            source: source_err,
        })?
        .len();

    let entries = std::fs::read_dir(dir).map_err(|source_err| OrganizerError::Read {
        path: dir.to_path_buf(),
        source: source_err,
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || path == source {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if meta.len() != source_size {
            continue;
        }
        if duplicates::are_duplicates(source, &path)? {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn resolve_duplicate(
    existing: &Path,
    dest_dir: &Path,
    final_stem: &str,
    ext: Option<&str>,
    config: &Config,
) -> Result<Action, OrganizerError> {
    match config.duplicates.action {
        DuplicateAction::Skip => Ok(Action::SkipDuplicate {
            of: existing.to_path_buf(),
        }),
        DuplicateAction::Move => {
            let dup_dir = dest_dir.join(&config.duplicates.folder_name);
            let destination = mover::unique_destination(&dup_dir, final_stem, ext)?;
            Ok(Action::MoveDuplicate {
                destination,
                of: existing.to_path_buf(),
            })
        }
    }
}

pub fn split_stem_ext(path: &Path) -> (String, Option<String>) {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_string());
    (stem, ext)
}
