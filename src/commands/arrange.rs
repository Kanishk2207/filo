//! `filo arrange` — manual, filtered bulk move.
//!
//! Distinct from `scan` in that the user specifies exactly what to match
//! and where to send it. No categorization rules, no duplicate detection —
//! just collision-safe moves into a single destination.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use dialoguer::{theme::ColorfulTheme, Confirm};
use walkdir::WalkDir;

use crate::organizer::{self, mover};

pub struct ArrangeArgs {
    pub sources: Vec<PathBuf>,
    pub destination: PathBuf,
    pub keywords: Vec<String>,
    pub extensions: Vec<String>,
    pub yes: bool,
}

pub fn run(args: ArrangeArgs) -> Result<()> {
    if args.keywords.is_empty() && args.extensions.is_empty() {
        anyhow::bail!("provide at least one --keyword or --extension");
    }

    let keywords: Vec<String> = args.keywords.iter().map(|k| k.to_lowercase()).collect();
    let extensions: Vec<String> = args
        .extensions
        .iter()
        .map(|e| e.trim_start_matches('.').to_lowercase())
        .collect();

    let matches = collect_matches(&args.sources, &keywords, &extensions);

    if matches.is_empty() {
        println!("No files matched.");
        return Ok(());
    }

    println!(
        "Matched {} file(s). Destination: {}",
        matches.len(),
        args.destination.display()
    );
    for m in &matches {
        println!("  {}", m.display());
    }

    if !args.yes {
        let proceed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Proceed with move?")
            .default(false)
            .interact()?;
        if !proceed {
            println!("Aborted.");
            return Ok(());
        }
    }

    mover::ensure_dir(&args.destination).context("preparing destination")?;

    let mut moved = 0usize;
    let mut errors = 0usize;

    for src in matches {
        match move_one(&src, &args.destination) {
            Ok(dest) => {
                log::info!("arrange: {} -> {}", src.display(), dest.display());
                println!("  moved  {}  ->  {}", src.display(), dest.display());
                moved += 1;
            }
            Err(e) => {
                log::error!("arrange failed on {}: {}", src.display(), e);
                println!("  error  {}: {}", src.display(), e);
                errors += 1;
            }
        }
    }

    println!();
    println!("Arrange complete: {} moved, {} errors", moved, errors);
    Ok(())
}

fn move_one(src: &Path, destination_dir: &Path) -> Result<PathBuf> {
    let (stem, ext) = organizer::split_stem_ext(src);
    let dest = mover::unique_destination(destination_dir, &stem, ext.as_deref())?;
    mover::safe_move(src, &dest)?;
    Ok(dest)
}

fn collect_matches(sources: &[PathBuf], keywords: &[String], exts: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for src in sources {
        if !src.is_dir() {
            log::warn!(
                "source does not exist or is not a directory: {}",
                src.display()
            );
            continue;
        }
        for entry in WalkDir::new(src).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if organizer::should_skip(path) {
                continue;
            }
            if file_matches(path, keywords, exts) {
                out.push(path.to_path_buf());
            }
        }
    }
    out
}

fn file_matches(path: &Path, keywords: &[String], exts: &[String]) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());

    let kw_ok = keywords.is_empty() || keywords.iter().any(|k| name.contains(k));
    let ext_ok = exts.is_empty()
        || ext
            .as_ref()
            .map(|e| exts.iter().any(|x| x == e))
            .unwrap_or(false);
    kw_ok && ext_ok
}
