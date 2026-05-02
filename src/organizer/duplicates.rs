//! Size-first, SHA-256 confirmation duplicate detection.
//!
//! Hashing is the expensive step, so we short-circuit on `metadata().len()`.
//! The hash itself reads the file in fixed-size chunks to avoid loading the
//! whole file into memory — important for videos and archives.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::errors::OrganizerError;

const CHUNK_SIZE: usize = 64 * 1024;

pub type Sha256Digest = [u8; 32];

pub fn are_duplicates(a: &Path, b: &Path) -> Result<bool, OrganizerError> {
    let size_a = file_size(a)?;
    let size_b = file_size(b)?;
    if size_a != size_b {
        return Ok(false);
    }
    Ok(sha256_file(a)? == sha256_file(b)?)
}

pub fn sha256_file(path: &Path) -> Result<Sha256Digest, OrganizerError> {
    let file = File::open(path).map_err(|source| OrganizerError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK_SIZE];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|source| OrganizerError::Read {
                path: path.to_path_buf(),
                source,
            })?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().into())
}

fn file_size(path: &Path) -> Result<u64, OrganizerError> {
    let meta = std::fs::metadata(path).map_err(|source| OrganizerError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(meta.len())
}
