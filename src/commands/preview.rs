//! `filo preview` — dry run. Computes every plan but executes none.

use anyhow::Result;
use walkdir::WalkDir;

use crate::config::Config;
use crate::organizer;

pub fn run(config: &Config) -> Result<()> {
    if config.watch.folders.is_empty() {
        println!("No folders configured. Run `filo init` to add some.");
        return Ok(());
    }

    let mut total = 0usize;
    let mut moves = 0usize;
    let mut dup_skip = 0usize;
    let mut dup_move = 0usize;
    let mut errors = 0usize;

    for folder in &config.watch.folders {
        if !folder.is_dir() {
            println!("(skipping missing folder: {})", folder.display());
            continue;
        }
        println!();
        println!("== {} ==", folder.display());

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
            total += 1;
            match organizer::plan(entry.path(), folder, config) {
                Ok(plan) => match &plan.action {
                    organizer::Action::Move { destination } => {
                        println!(
                            "  move    {}  ->  {}",
                            plan.source.display(),
                            destination.display()
                        );
                        moves += 1;
                    }
                    organizer::Action::SkipDuplicate { of } => {
                        println!(
                            "  skip    {}  (duplicate of {})",
                            plan.source.display(),
                            of.display()
                        );
                        dup_skip += 1;
                    }
                    organizer::Action::MoveDuplicate { destination, of } => {
                        println!(
                            "  dup ->  {}  ->  {}  (duplicate of {})",
                            plan.source.display(),
                            destination.display(),
                            of.display()
                        );
                        dup_move += 1;
                    }
                },
                Err(e) => {
                    println!("  error   {}: {}", entry.path().display(), e);
                    errors += 1;
                }
            }
        }
    }

    println!();
    println!("Preview summary ({} files examined):", total);
    println!("  would move:               {}", moves);
    println!("  would skip (duplicate):   {}", dup_skip);
    println!("  would move to Duplicates: {}", dup_move);
    if errors > 0 {
        println!("  errors:                   {}", errors);
    }
    println!();
    println!("No changes were made. Run `filo scan` to apply.");
    Ok(())
}
