//! Safe move + collision-free destination naming.
//!
//! The "never overwrite" invariant is enforced here. Every caller that
//! intends to create a new file at a destination must route through
//! [`unique_destination`] to pick a non-colliding path, then through
//! [`safe_move`] which re-checks existence immediately before the rename.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::errors::OrganizerError;

/// Build `dir/stem.ext`, leaving the extension off if `ext` is `None`.
pub fn build_dest(dir: &Path, stem: &str, ext: Option<&str>) -> PathBuf {
    let mut p = dir.join(stem);
    if let Some(e) = ext {
        p.set_extension(e);
    }
    p
}

/// Find the first non-existent path in the strict priority order:
///
/// 1. `stem.ext`
/// 2. `stem-YYYY-MM-DD.ext`
/// 3. `stem-YYYY-MM-DD-N.ext` for N in 1..=999
/// 4. `stem-<8-char-hash>.ext` (salted with system time → unique per call)
///
/// Only if every strategy fails does this return [`OrganizerError::NoUniqueName`].
pub fn unique_destination(
    dir: &Path,
    stem: &str,
    ext: Option<&str>,
) -> Result<PathBuf, OrganizerError> {
    let base = build_dest(dir, stem, ext);
    if !base.exists() {
        return Ok(base);
    }

    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let dated = build_dest(dir, &format!("{}-{}", stem, date), ext);
    if !dated.exists() {
        return Ok(dated);
    }

    for n in 1..=999u32 {
        let p = build_dest(dir, &format!("{}-{}-{}", stem, date, n), ext);
        if !p.exists() {
            return Ok(p);
        }
    }

    let suffix = short_hash(stem);
    let hashed = build_dest(dir, &format!("{}-{}", stem, suffix), ext);
    if !hashed.exists() {
        return Ok(hashed);
    }

    Err(OrganizerError::NoUniqueName(base))
}

pub fn ensure_dir(dir: &Path) -> Result<(), OrganizerError> {
    fs::create_dir_all(dir).map_err(|source| OrganizerError::CreateDir {
        path: dir.to_path_buf(),
        source,
    })
}

/// Move `from` → `to`, guaranteeing no existing file is overwritten.
///
/// Tries `rename` first (atomic on the same filesystem), falling back to
/// copy-then-remove for cross-filesystem moves. The pre-move existence
/// check is a safety net — callers should already have resolved a unique
/// destination via [`unique_destination`].
pub fn safe_move(from: &Path, to: &Path) -> Result<(), OrganizerError> {
    if !from.exists() {
        return Err(OrganizerError::SourceMissing(from.to_path_buf()));
    }
    if to.exists() {
        return Err(OrganizerError::Move {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "destination already exists",
            ),
        });
    }
    if let Some(parent) = to.parent() {
        ensure_dir(parent)?;
    }

    // Fast path: atomic same-filesystem rename.
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }

    // Slow path: cross-filesystem copy-then-remove. If the copy succeeds
    // but the source delete fails we leave both intact rather than risk
    // corrupting state — the user can manually resolve.
    fs::copy(from, to).map_err(|source| OrganizerError::Move {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        source,
    })?;
    fs::remove_file(from).map_err(|source| OrganizerError::Move {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn short_hash(stem: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(stem.as_bytes());
    hasher.update(b"|");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    hasher.update(nanos.to_le_bytes());
    let digest = hasher.finalize();
    let mut s = String::with_capacity(8);
    for b in &digest[..4] {
        s.push_str(&format!("{:02x}", b));
    }
    s
}
