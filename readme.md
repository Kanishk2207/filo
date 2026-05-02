# filo

> Your Downloads folder cleans itself — safely, predictably, and reversibly.

`filo` is a cross-platform command-line tool that organizes files in folders
you care about (Downloads, Desktop, anywhere else) by rules you control.
It can run once, or sit in the background and keep things tidy as new files
land.

It is built on three promises:

- **Safety.** It never overwrites an existing file. Ever. Collisions are
  resolved by picking a new, unique name.
- **Predictability.** The rules live in a plain TOML file you can read and
  edit. The same input always produces the same output.
- **Trust.** Every run can be previewed first. `filo preview` shows exactly
  what would move where, without touching a single byte.

---

## Table of contents

- [Why filo exists](#why-filo-exists)
- [Installation](#installation)
- [First-time setup](#first-time-setup)
- [Commands](#commands)
- [Config file](#config-file)
- [Safety guarantees](#safety-guarantees)
- [Preview mode, in detail](#preview-mode-in-detail)
- [Logging](#logging)
- [Contributing](#contributing)
- [License](#license)

---

## Why filo exists

Download folders rot. They start clean, then fifteen PDFs, eight installer
disk images, a dozen random screenshots, and three copies of `Report_FINAL_FINAL.pdf`
later they're impossible to navigate. Manual cleanup is a chore nobody does.

`filo` is a small, boring, reliable tool that does the chore for you.

It is deliberately not clever. It does not look inside files, call machine
learning models, or invent names. It routes by extension, dedupes by SHA-256,
and moves things into subfolders. That's the whole product — and that's the
point.

---

## Installation

### From source

Requires Rust `1.85` or newer.

```bash
git clone https://github.com/your/filo
cd filo
cargo install --path .
```

The `filo` binary is installed to `~/.cargo/bin/filo` (Unix) or
`%USERPROFILE%\.cargo\bin\filo.exe` (Windows). Make sure that path is on
your `PATH`.

### Verify

```bash
filo --version
filo --help
```

---

## First-time setup

Run:

```bash
filo init
```

`init` walks you through an interactive wizard:

1. **Which folders should filo watch?** You'll see suggestions (Downloads,
   Desktop, Documents) detected from your platform, and can add custom paths.
2. **Enable smart renaming?** Default: off. When enabled, filenames are
   lowercased, hyphenated, and stripped of adjacent repeated words
   (`Report_FINAL_FINAL.pdf` → `report-final.pdf`). You can also prepend a
   `YYYY-MM-DD` date.
3. **Duplicate handling.** `skip` (leave the duplicate where it is, log it)
   or `move` (relocate it to a `Duplicates` subfolder next to the category).
4. **Start watching now?** If yes, `filo` jumps straight into `start` mode
   when setup ends.

The wizard writes `config.toml` to the platform config directory
(see [Config file](#config-file)). You can re-run `filo init` any time to
rebuild the config; existing configs are never silently overwritten.

---

## Commands

| Command           | What it does                                                         |
|-------------------|----------------------------------------------------------------------|
| `filo init`       | Interactive first-time setup. Writes `config.toml`.                  |
| `filo preview`    | Dry run: print every move filo _would_ make. No changes on disk.     |
| `filo scan`       | Organize files already sitting in your configured watch folders.     |
| `filo start`      | Watch configured folders and organize new files as they arrive.      |
| `filo arrange`    | Manual, targeted move: pick files by keyword/extension and move them. |

Most commands accept `--rename` to enable smart renaming for that run only,
independently of the config.

### Examples

Preview what would happen if you cleaned up your Downloads folder right now:

```bash
filo preview
```

Apply the plan for real:

```bash
filo scan
```

Run with smart renaming turned on, ignoring whatever's in the config:

```bash
filo scan --rename
```

Start the live watcher in the background:

```bash
filo start
```

Manually collect every `.pdf` in Downloads and Desktop containing the word
"invoice" into `~/Finance/2025`:

```bash
filo arrange \
  --source ~/Downloads --source ~/Desktop \
  --destination ~/Finance/2025 \
  --keyword invoice \
  --extension pdf
```

Same, but skip the confirmation prompt:

```bash
filo arrange -s ~/Downloads -d ~/Finance/2025 -k invoice -e pdf -y
```

---

## Config file

filo uses platform-appropriate paths (via the [`dirs`](https://crates.io/crates/dirs)
crate). It never hardcodes `/home/...` or `C:\Users\...`.

| OS               | Config                                     | Log                                        |
|------------------|--------------------------------------------|--------------------------------------------|
| Linux            | `~/.config/filo/config.toml`               | `~/.local/share/filo/filo.log`             |
| macOS            | `~/Library/Application Support/filo/config.toml` | `~/Library/Application Support/filo/filo.log` |
| Windows          | `%APPDATA%\filo\config.toml`               | `%APPDATA%\filo\filo.log`                  |

The file is human-readable TOML. Edit it freely — filo will pick up your
changes on the next run.

### Annotated example

```toml
# Category used when no rule matches the file's extension.
other_category = "Other"

[watch]
# Folders filo watches and scans. Must be absolute paths.
folders = [
  "/home/you/Downloads",
  "/home/you/Desktop",
]
# If true, `filo init` launches `filo start` automatically after setup.
auto_start = false
# How long (milliseconds) a file must sit idle before filo acts on it.
# Protects against operating on files that are still downloading.
debounce_ms = 2000

[rename]
# Master switch for smart renaming. When false, filenames are preserved.
enabled = false
# "My  Cool File.pdf" -> "my-cool-file.pdf"
lowercase_with_hyphens = true
# "Report_FINAL_FINAL.pdf" -> "report-final.pdf" (collapses adjacent repeats)
strip_redundant_words = true
# "report.pdf" -> "2026-05-02-report.pdf"
prepend_date = false

[duplicates]
# "skip": leave the duplicate in place and log it.
# "move": relocate it into a `Duplicates` subfolder beside the category.
action = "skip"
folder_name = "Duplicates"

[rules]
# Category -> list of extensions (no leading dot). Case-insensitive.
# Add/remove freely. Unknown extensions fall through to `other_category`.
Archives  = ["zip", "tar", "gz", "bz2", "xz", "rar", "7z", "tgz"]
Audio     = ["mp3", "flac", "wav", "ogg", "m4a", "aac", "opus"]
Code      = ["rs", "py", "js", "ts", "go", "c", "cpp", "h", "java", "rb", "sh"]
Documents = ["pdf", "doc", "docx", "odt", "rtf", "txt", "md", "epub"]
Images    = ["jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "tiff", "heic", "avif"]
Videos    = ["mp4", "mkv", "mov", "avi", "webm", "flv", "wmv", "m4v"]
```

---

## Safety guarantees

These are non-negotiable invariants of the tool:

1. **Never overwrite.** Before any move, the destination is checked. If a
   file with the target name exists, filo picks a new unique name in this
   strict priority order:
   1. Original name.
   2. Append current date: `name-2026-05-02.ext`.
   3. Append numeric suffix: `name-2026-05-02-1.ext`, `-2`, ... `-999`.
   4. Append a short hash of the name plus the current timestamp.
2. **Never delete.** filo moves files. It does not delete them — not even
   duplicates. Duplicates are either left alone or relocated to a dedicated
   folder, per your config.
3. **Never act on in-progress writes.** Files are ignored if they end in
   `.part`, `.crdownload`, `.download`, `.tmp`, or `.partial`. Watch events
   are debounced, and a second-stage size-stability check catches anything
   still in motion.
4. **Never touch hidden files.** Anything starting with `.` (e.g.
   `.DS_Store`, `.gitignore`) is left alone.
5. **Graceful failures.** Permission errors, missing files, and other I/O
   failures are logged and skipped. One bad file never aborts a scan.
6. **Deterministic.** Duplicate detection is SHA-256. Same input bytes →
   same result. No fuzzy matching, no heuristics.

---

## Preview mode, in detail

`filo preview` runs the exact same planning pipeline as `filo scan`, but
stops right before any file is touched. The output looks like:

```
== /Users/you/Downloads ==
  move    /Users/you/Downloads/invoice.pdf  ->  /Users/you/Downloads/Documents/invoice.pdf
  move    /Users/you/Downloads/cat.jpg      ->  /Users/you/Downloads/Images/cat.jpg
  skip    /Users/you/Downloads/cat-copy.jpg (duplicate of /Users/you/Downloads/Images/cat.jpg)
  dup ->  /Users/you/Downloads/old.pdf      ->  /Users/you/Downloads/Documents/Duplicates/old.pdf (duplicate of ...)

Preview summary (4 files examined):
  would move:               2
  would skip (duplicate):   1
  would move to Duplicates: 1

No changes were made. Run `filo scan` to apply.
```

Because the planner is pure, you can:

- Preview → read the plan → edit `config.toml` → preview again, risk-free.
- Diff two configs by diffing their previews.

---

## Logging

filo writes to both stderr and an append-only log file.

- **Level** defaults to `info`. Override with the standard env var:

  ```bash
  RUST_LOG=debug filo scan
  ```

- **Log file location** is listed in the [config file table](#config-file).
  Every line is timestamped with millisecond precision.
- Logging failures never affect command execution — if the log file can't
  be written, filo still does the right thing on disk.

---

## Contributing

filo is structured as a library (`filo` in `src/lib.rs`) plus a thin binary
(`src/main.rs`). The binary only parses arguments and dispatches; all
behavior lives in the library, which makes it straightforward to:

- add a new command (drop a file in `src/commands/`);
- add a new category (edit `src/config/defaults.rs` or just edit your own
  `config.toml`);
- add a new duplicate policy (extend `DuplicateAction` in `src/config/mod.rs`
  and handle it in `src/organizer/mod.rs::resolve_duplicate`).

Layout:

```
src/
  main.rs          # binary entry point — parse CLI, dispatch
  lib.rs           # library root — public API
  cli.rs           # clap definitions (no logic)
  errors.rs        # thiserror types per domain
  logging.rs       # env_logger + file output
  commands/        # one module per `filo <verb>`
    init.rs
    start.rs
    scan.rs
    preview.rs
    arrange.rs
  config/          # on-disk config schema + defaults
    mod.rs
    defaults.rs
  organizer/       # the engine
    mod.rs         # plan() + execute()
    rules.rs       # extension -> category
    mover.rs       # safe_move + unique_destination
    renamer.rs     # smart rename pipeline
    duplicates.rs  # size + SHA-256 duplicate check
  watcher/
    mod.rs         # debounced notify-based watcher
tests/
  integration.rs   # end-to-end pipeline tests
```

Before sending a PR:

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

---

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
