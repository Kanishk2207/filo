//! End-to-end integration tests driving the library API directly.
//!
//! These exercise the real plan → execute pipeline against the real
//! filesystem in a temp directory, with no CLI or config-file layer.

use std::fs;
use std::path::Path;

use filo::config::{Config, DuplicateAction};
use filo::organizer;

fn tmpdir() -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!("filo-test-{}", nanos()));
    fs::create_dir_all(&base).unwrap();
    base
}

fn nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

fn test_config(watch: &Path) -> Config {
    let mut cfg = Config::default();
    cfg.watch.folders = vec![watch.to_path_buf()];
    cfg
}

fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, bytes).unwrap();
}

#[test]
fn routes_by_extension() {
    let root = tmpdir();
    let src = root.join("hello.pdf");
    write(&src, b"doc");

    let cfg = test_config(&root);
    let plan = organizer::plan(&src, &root, &cfg).unwrap();
    match plan.action {
        organizer::Action::Move { destination } => {
            assert_eq!(destination, root.join("Documents").join("hello.pdf"));
        }
        other => panic!("expected Move, got {:?}", other),
    }
}

#[test]
fn unknown_extension_goes_to_other() {
    let root = tmpdir();
    let src = root.join("weird.xyz");
    write(&src, b"?");
    let cfg = test_config(&root);
    let plan = organizer::plan(&src, &root, &cfg).unwrap();
    match plan.action {
        organizer::Action::Move { destination } => {
            assert_eq!(destination.parent().unwrap().file_name().unwrap(), "Other");
        }
        other => panic!("expected Move, got {:?}", other),
    }
}

#[test]
fn never_overwrites_existing_file() {
    let root = tmpdir();
    let dest_dir = root.join("Documents");
    fs::create_dir_all(&dest_dir).unwrap();
    // Pre-existing non-duplicate file at the target name.
    write(&dest_dir.join("note.pdf"), b"OLD");

    let src = root.join("note.pdf");
    write(&src, b"NEW");

    let cfg = test_config(&root);
    let plan = organizer::plan(&src, &root, &cfg).unwrap();
    organizer::execute(&plan).unwrap();

    // The old file must still be intact and untouched.
    assert_eq!(fs::read(dest_dir.join("note.pdf")).unwrap(), b"OLD");

    // The new file was moved and given a unique name (date-suffixed).
    let moved = match plan.action {
        organizer::Action::Move { destination } => destination,
        other => panic!("expected Move, got {:?}", other),
    };
    assert!(!src.exists(), "source should have been moved");
    assert_eq!(fs::read(&moved).unwrap(), b"NEW");
    assert_ne!(moved, dest_dir.join("note.pdf"));
}

#[test]
fn detects_duplicate_by_hash_and_skips() {
    let root = tmpdir();
    let dest_dir = root.join("Documents");
    fs::create_dir_all(&dest_dir).unwrap();
    // An existing file with the SAME content but a DIFFERENT name — the
    // hash-based detection must still catch it.
    write(&dest_dir.join("already-here.pdf"), b"identical bytes");

    let src = root.join("newcomer.pdf");
    write(&src, b"identical bytes");

    let mut cfg = test_config(&root);
    cfg.duplicates.action = DuplicateAction::Skip;

    let plan = organizer::plan(&src, &root, &cfg).unwrap();
    match &plan.action {
        organizer::Action::SkipDuplicate { of } => {
            assert_eq!(of, &dest_dir.join("already-here.pdf"));
        }
        other => panic!("expected SkipDuplicate, got {:?}", other),
    }
    organizer::execute(&plan).unwrap();
    assert!(src.exists(), "skipped duplicate must remain in place");
}

#[test]
fn moves_duplicate_to_duplicates_folder() {
    let root = tmpdir();
    let dest_dir = root.join("Documents");
    fs::create_dir_all(&dest_dir).unwrap();
    write(&dest_dir.join("original.pdf"), b"same content");

    let src = root.join("dup.pdf");
    write(&src, b"same content");

    let mut cfg = test_config(&root);
    cfg.duplicates.action = DuplicateAction::Move;

    let plan = organizer::plan(&src, &root, &cfg).unwrap();
    match &plan.action {
        organizer::Action::MoveDuplicate { destination, .. } => {
            assert_eq!(destination.parent().unwrap(), dest_dir.join("Duplicates"));
        }
        other => panic!("expected MoveDuplicate, got {:?}", other),
    }
    organizer::execute(&plan).unwrap();
    assert!(!src.exists());
}

#[test]
fn different_sizes_are_not_duplicates() {
    let root = tmpdir();
    let dest_dir = root.join("Documents");
    fs::create_dir_all(&dest_dir).unwrap();
    write(&dest_dir.join("a.pdf"), b"small");

    let src = root.join("b.pdf");
    write(&src, b"this is clearly larger");

    let cfg = test_config(&root);
    let plan = organizer::plan(&src, &root, &cfg).unwrap();
    match plan.action {
        organizer::Action::Move { .. } => {}
        other => panic!("expected Move, got {:?}", other),
    }
}

#[test]
fn should_skip_filters_dotfiles_and_partials() {
    assert!(organizer::should_skip(Path::new(".DS_Store")));
    assert!(organizer::should_skip(Path::new(".gitignore")));
    assert!(organizer::should_skip(Path::new("big.zip.crdownload")));
    assert!(organizer::should_skip(Path::new("incomplete.part")));
    assert!(!organizer::should_skip(Path::new("hello.pdf")));
}

#[test]
fn config_roundtrips_through_toml() {
    let root = tmpdir();
    let path = root.join("config.toml");
    let mut original = Config::default();
    original.watch.folders = vec![root.join("Downloads")];
    original.rename.enabled = true;
    original.duplicates.action = DuplicateAction::Move;

    original.save_to(&path).unwrap();
    let loaded = Config::load_from(&path).unwrap();

    assert_eq!(loaded.watch.folders, original.watch.folders);
    assert!(loaded.rename.enabled);
    assert_eq!(loaded.duplicates.action, DuplicateAction::Move);
    assert!(loaded.rules.contains_key("Documents"));
}
