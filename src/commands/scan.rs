//! `filo scan` — one-shot organize of files already present.

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use walkdir::WalkDir;

use crate::config::Config;
use crate::organizer;

pub fn run(config: &Config, rename_override: bool) -> Result<()> {
    let mut config = config.clone();
    if rename_override {
        config.rename.enabled = true;
    }

    if config.watch.folders.is_empty() {
        println!("No folders configured. Run `filo init` to add some.");
        return Ok(());
    }

    let files = collect_files(&config);
    if files.is_empty() {
        println!("No files found in watch folders.");
        return Ok(());
    }

    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>5}/{len:5} {msg}",
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("=>-"),
    );

    let mut moved = 0usize;
    let mut skipped_dup = 0usize;
    let mut errors = 0usize;

    for (root, file) in files {
        pb.set_message(
            file.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string(),
        );

        match process_one(&root, &file, &config) {
            Ok(Outcome::Moved) => moved += 1,
            Ok(Outcome::Skipped) => skipped_dup += 1,
            Err(e) => {
                // Per the safety rules, a single file error must not crash
                // the whole run. Log and continue.
                log::error!("failed on {}: {}", file.display(), e);
                errors += 1;
            }
        }
        pb.inc(1);
    }

    pb.finish_with_message("done");

    println!();
    println!(
        "Scan complete: {} moved, {} skipped (duplicates), {} errors",
        moved, skipped_dup, errors
    );
    Ok(())
}

enum Outcome {
    Moved,
    Skipped,
}

fn process_one(
    root: &std::path::Path,
    file: &std::path::Path,
    config: &Config,
) -> anyhow::Result<Outcome> {
    let plan = organizer::plan(file, root, config)?;
    organizer::execute(&plan)?;
    match &plan.action {
        organizer::Action::Move { destination } => {
            log::info!(
                "scan: {} -> {}",
                plan.source.display(),
                destination.display()
            );
            Ok(Outcome::Moved)
        }
        organizer::Action::MoveDuplicate { destination, of } => {
            log::info!(
                "scan duplicate: {} -> {} (identical to {})",
                plan.source.display(),
                destination.display(),
                of.display()
            );
            Ok(Outcome::Moved)
        }
        organizer::Action::SkipDuplicate { of } => {
            log::info!(
                "scan skipped duplicate: {} (identical to {})",
                plan.source.display(),
                of.display()
            );
            Ok(Outcome::Skipped)
        }
    }
}

fn collect_files(config: &Config) -> Vec<(std::path::PathBuf, std::path::PathBuf)> {
    let mut out = Vec::new();
    for folder in &config.watch.folders {
        if !folder.is_dir() {
            log::warn!(
                "watch folder missing or not a directory: {}",
                folder.display()
            );
            continue;
        }
        // Depth 1 = top level only. Files already organized into
        // category subfolders are intentionally left alone.
        for entry in WalkDir::new(folder)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            if organizer::should_skip(entry.path()) {
                continue;
            }
            out.push((folder.clone(), entry.path().to_path_buf()));
        }
    }
    out
}
