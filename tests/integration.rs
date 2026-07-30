//! End-to-end integration tests driving the library API directly.
//!
//! These exercise the real plan → execute pipeline against the real
//! filesystem in a temp directory, with no CLI or config-file layer.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use filo::config::{Config, DestinationKind, DuplicateAction, KeywordRule, KeywordRuleConfig};
use filo::organizer;

static TMPDIR_COUNTER: AtomicU64 = AtomicU64::new(0);

fn tmpdir() -> std::path::PathBuf {
    let seq = TMPDIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!(
        "filo-test-{}-{}-{}",
        std::process::id(),
        nanos(),
        seq
    ));
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
fn keyword_rule_overrides_extension_rule() {
    let root = tmpdir();
    let src = root.join("invoice.png");
    write(&src, b"img");

    let mut cfg = test_config(&root);
    cfg.keyword_rules.rules.push(KeywordRule::folder(
        "Documents",
        vec!["invoice".to_string()],
    ));

    let plan = organizer::plan(&src, &root, &cfg).unwrap();
    match plan.action {
        organizer::Action::Move { destination } => {
            assert_eq!(destination, root.join("Documents").join("invoice.png"));
        }
        other => panic!("expected Move, got {:?}", other),
    }
}

#[test]
fn keyword_rules_are_case_insensitive() {
    let root = tmpdir();
    let src = root.join("Monthly-RECEIPT.jpg");
    write(&src, b"img");

    let mut cfg = test_config(&root);
    cfg.keyword_rules.rules.push(KeywordRule::folder(
        "Documents",
        vec!["receipt".to_string()],
    ));

    let plan = organizer::plan(&src, &root, &cfg).unwrap();
    match plan.action {
        organizer::Action::Move { destination } => {
            assert_eq!(
                destination.parent().unwrap().file_name().unwrap(),
                "Documents"
            );
        }
        other => panic!("expected Move, got {:?}", other),
    }
}

#[test]
fn keyword_rules_trim_surrounding_spaces() {
    let root = tmpdir();
    let src = root.join("invoice-final.png");
    write(&src, b"img");

    let mut cfg = test_config(&root);
    cfg.keyword_rules.rules.push(KeywordRule::folder(
        "Documents",
        vec!["  invoice  ".to_string()],
    ));

    let plan = organizer::plan(&src, &root, &cfg).unwrap();
    match plan.action {
        organizer::Action::Move { destination } => {
            assert_eq!(
                destination,
                root.join("Documents").join("invoice-final.png")
            );
        }
        other => panic!("expected Move, got {:?}", other),
    }
}

#[test]
fn keyword_rule_order_decides_conflicts() {
    let root = tmpdir();
    let src = root.join("amazon_invoice.pdf");
    write(&src, b"doc");

    let mut cfg = test_config(&root);
    cfg.keyword_rules
        .rules
        .push(KeywordRule::folder("Amazon", vec!["amazon".to_string()]));
    cfg.keyword_rules.rules.push(KeywordRule::folder(
        "Documents",
        vec!["invoice".to_string()],
    ));

    let plan = organizer::plan(&src, &root, &cfg).unwrap();
    match plan.action {
        organizer::Action::Move { destination } => {
            assert_eq!(destination, root.join("Amazon").join("amazon_invoice.pdf"));
        }
        other => panic!("expected Move, got {:?}", other),
    }
}

// ── Path destinations (`to_type = "path"`) ─────────────────────────────

#[test]
fn path_rule_moves_outside_the_watch_root() {
    let base = tmpdir();
    let root = base.join("Downloads");
    let target = base.join("personal").join("kanishk-itr");
    let src = root.join("ITR-kanishk-2026.pdf");
    write(&src, b"tax");

    let mut cfg = test_config(&root);
    cfg.keyword_rules.rules.push(KeywordRule::path(
        target.to_str().unwrap(),
        vec!["itr".to_string()],
    ));

    let plan = organizer::plan(&src, &root, &cfg).unwrap();
    match &plan.action {
        organizer::Action::Move { destination } => {
            assert_eq!(destination, &target.join("ITR-kanishk-2026.pdf"));
        }
        other => panic!("expected Move, got {:?}", other),
    }

    // The destination path did not exist beforehand; executing creates it.
    organizer::execute(&plan).unwrap();
    assert!(!src.exists(), "source should have been moved");
    assert_eq!(
        fs::read(target.join("ITR-kanishk-2026.pdf")).unwrap(),
        b"tax"
    );
}

#[test]
fn path_rule_accepts_a_relative_path_anchored_to_the_watch_root() {
    let root = tmpdir();
    let src = root.join("itr-2026.pdf");
    write(&src, b"tax");

    let mut cfg = test_config(&root);
    cfg.keyword_rules
        .rules
        .push(KeywordRule::path("tax/2026", vec!["itr".to_string()]));

    let plan = organizer::plan(&src, &root, &cfg).unwrap();
    match plan.action {
        organizer::Action::Move { destination } => {
            assert_eq!(
                destination,
                root.join("tax").join("2026").join("itr-2026.pdf")
            );
        }
        other => panic!("expected Move, got {:?}", other),
    }
}

#[test]
fn path_rule_expands_a_leading_tilde() {
    let root = tmpdir();
    let src = root.join("itr-2026.pdf");
    write(&src, b"tax");

    let mut cfg = test_config(&root);
    cfg.keyword_rules.rules.push(KeywordRule::path(
        "~/personal/kanishk-itr",
        vec!["itr".to_string()],
    ));

    // Plan only — never execute a home-directory move from a test.
    let plan = organizer::plan(&src, &root, &cfg).unwrap();
    let destination = match plan.action {
        organizer::Action::Move { destination } => destination,
        other => panic!("expected Move, got {:?}", other),
    };

    let home = dirs::home_dir().expect("test host has a home directory");
    assert_eq!(
        destination,
        home.join("personal")
            .join("kanishk-itr")
            .join("itr-2026.pdf")
    );
    assert!(src.exists(), "planning must not touch the source");
}

#[test]
fn path_rule_beats_extension_routing_and_folder_rules_still_work() {
    let base = tmpdir();
    let root = base.join("Downloads");
    let target = base.join("archive");
    let itr = root.join("kanishk-itr.pdf");
    let other = root.join("holiday.pdf");
    write(&itr, b"tax");
    write(&other, b"pics");

    let mut cfg = test_config(&root);
    cfg.keyword_rules.rules.push(KeywordRule::path(
        target.to_str().unwrap(),
        vec!["itr".to_string()],
    ));

    match organizer::plan(&itr, &root, &cfg).unwrap().action {
        organizer::Action::Move { destination } => {
            assert_eq!(destination, target.join("kanishk-itr.pdf"));
        }
        other => panic!("expected Move, got {:?}", other),
    }

    // A file that matches no keyword rule still routes by extension into the
    // watch root, untouched by the new option.
    match organizer::plan(&other, &root, &cfg).unwrap().action {
        organizer::Action::Move { destination } => {
            assert_eq!(destination, root.join("Documents").join("holiday.pdf"));
        }
        other => panic!("expected Move, got {:?}", other),
    }
}

#[test]
fn path_rule_pointing_at_the_source_folder_is_a_no_op() {
    let root = tmpdir();
    let src = root.join("itr-2026.pdf");
    write(&src, b"tax");

    let mut cfg = test_config(&root);
    // Worst case: the rule resolves to the watch root itself, and duplicates
    // are configured to move. The file must not be shunted into Duplicates.
    cfg.duplicates.action = DuplicateAction::Move;
    cfg.keyword_rules.rules.push(KeywordRule::path(
        root.to_str().unwrap(),
        vec!["itr".to_string()],
    ));

    let plan = organizer::plan(&src, &root, &cfg).unwrap();
    match plan.action {
        organizer::Action::AlreadyInPlace => {}
        other => panic!("expected AlreadyInPlace, got {:?}", other),
    }
    organizer::execute(&plan).unwrap();
    assert!(src.exists(), "file must stay exactly where it is");
    assert!(!root.join("Duplicates").exists());
}

#[test]
fn path_rules_roundtrip_through_toml() {
    let root = tmpdir();
    let path = root.join("config.toml");
    let original = Config {
        keyword_rules: KeywordRuleConfig {
            rules: vec![
                KeywordRule::path("~/personal/kanishk-itr", vec!["itr".to_string()]),
                KeywordRule::folder("Amazon", vec!["amazon".to_string()]),
            ],
        },
        ..Config::default()
    };

    original.save_to(&path).unwrap();
    let raw = fs::read_to_string(&path).unwrap();
    assert!(
        raw.contains(r#"to_type = "path""#),
        "path rules must be written out: {}",
        raw
    );
    assert!(
        !raw.contains(r#"to_type = "folder""#),
        "folder is the default and should stay implicit: {}",
        raw
    );

    let loaded = Config::load_from(&path).unwrap();
    assert_eq!(loaded.keyword_rules, original.keyword_rules);
}

#[test]
fn rules_without_to_type_default_to_folder() {
    let root = tmpdir();
    let path = root.join("config.toml");
    fs::write(
        &path,
        r#"
other_category = "Other"

[keyword_rules]
rules = [
  { to = "Amazon", keywords = ["amazon"] },
]

[rules]
Documents = ["pdf"]
"#,
    )
    .unwrap();

    let loaded = Config::load_from(&path).unwrap();
    assert_eq!(loaded.keyword_rules.rules.len(), 1);
    assert_eq!(
        loaded.keyword_rules.rules[0].to_type,
        DestinationKind::Folder
    );
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
    original.keyword_rules = KeywordRuleConfig {
        rules: vec![
            KeywordRule::folder("Amazon", vec!["amazon".to_string()]),
            KeywordRule::folder(
                "Documents",
                vec!["invoice".to_string(), "receipt".to_string()],
            ),
        ],
    };

    original.save_to(&path).unwrap();
    let loaded = Config::load_from(&path).unwrap();

    assert_eq!(loaded.watch.folders, original.watch.folders);
    assert!(loaded.rename.enabled);
    assert_eq!(loaded.duplicates.action, DuplicateAction::Move);
    assert_eq!(loaded.keyword_rules, original.keyword_rules);
    assert!(loaded.rules.contains_key("Documents"));
}

// ── Config folder management ───────────────────────────────────────────

#[test]
fn add_folder_deduplicates() {
    let mut cfg = Config::default();
    let p = PathBuf::from("/tmp/filo-test-add-folder");
    assert!(cfg.add_folder(p.clone()));
    assert!(!cfg.add_folder(p.clone()), "second add should be no-op");
    assert_eq!(cfg.watch.folders.len(), 1);
}

#[test]
fn remove_folder_returns_false_when_absent() {
    let mut cfg = Config::default();
    assert!(!cfg.remove_folder(Path::new("/nonexistent")));
}

#[test]
fn add_then_remove_folder() {
    let mut cfg = Config::default();
    let p = PathBuf::from("/tmp/filo-test-add-remove");
    cfg.add_folder(p.clone());
    assert!(cfg.remove_folder(&p));
    assert!(cfg.watch.folders.is_empty());
}

#[test]
fn config_with_folders_roundtrips() {
    let root = tmpdir();
    let path = root.join("config.toml");
    let mut cfg = Config::default();
    cfg.add_folder(root.join("A"));
    cfg.add_folder(root.join("B"));
    cfg.save_to(&path).unwrap();

    let loaded = Config::load_from(&path).unwrap();
    assert_eq!(loaded.watch.folders.len(), 2);
    assert_eq!(loaded.watch.folders[0], root.join("A"));
    assert_eq!(loaded.watch.folders[1], root.join("B"));
}

#[test]
fn rename_disabled_avoids_no_unique_name_for_dotted_real_world_patterns() {
    let root = tmpdir();
    let mut cfg = test_config(&root);
    cfg.rename.enabled = false;

    let fixtures: [(&str, &[u8]); 7] = [
        ("Screenshot 2026-02-17 at 12.18.55 PM.jpg", b"a"),
        ("Screenshot 2026-02-17 at 12.18.49 PM.jpg", b"b"),
        ("WhatsApp Image 2026-05-02 at 21.35.38.jpeg", b"c"),
        ("WhatsApp Image 2026-05-02 at 21.35.38 (1).jpeg", b"d"),
        ("WhatsApp Image 2026-05-02 at 21.35.37.jpeg", b"e"),
        ("kanishk.shrivastava_credentials.csv", b"f"),
        ("kanishk.shrivastava_accessKeys_dummy_acc.csv", b"g"),
    ];

    for (name, bytes) in fixtures {
        write(&root.join(name), bytes);
    }

    for (name, _) in fixtures {
        let src = root.join(name);
        let plan = organizer::plan(&src, &root, &cfg)
            .unwrap_or_else(|e| panic!("plan failed for {}: {}", src.display(), e));

        let destination = match &plan.action {
            organizer::Action::Move { destination } => destination.clone(),
            other => panic!("expected Move for {}, got {:?}", src.display(), other),
        };

        let dest_name = destination
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        if name.contains("12.18.49") {
            assert!(
                dest_name.contains("12.18.49"),
                "expected destination to preserve timestamp segment for {}, got {}",
                name,
                dest_name
            );
        }
        if name.contains("21.35.38 (1)") {
            assert!(
                dest_name.contains("21.35.38 (1)"),
                "expected destination to preserve whatsapp stem for {}, got {}",
                name,
                dest_name
            );
        }
        if name.contains("21.35.37") {
            assert!(
                dest_name.contains("21.35.37"),
                "expected destination to preserve whatsapp stem for {}, got {}",
                name,
                dest_name
            );
        }
        if name.contains("kanishk.shrivastava_accessKeys_dummy_acc") {
            assert!(
                dest_name.contains("kanishk.shrivastava_accessKeys_dummy_acc"),
                "expected destination to preserve dotted csv stem for {}, got {}",
                name,
                dest_name
            );
        }

        organizer::execute(&plan).unwrap();
    }
}
