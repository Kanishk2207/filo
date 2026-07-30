# filo TOML Config Reference

This document is the canonical reference for all supported `config.toml` options.
It is designed to be easy to extend as new options are added.

## Scope and source of truth

- Schema and defaults: `src/config/mod.rs`, `src/config/defaults.rs`
- Runtime behavior: `src/commands/*`, `src/organizer/*`, `src/watcher/mod.rs`

If this file and code ever diverge, code behavior wins.

## Where `config.toml` lives

- Linux: `~/.config/filo/config.toml`
- macOS: `~/Library/Application Support/filo/config.toml`
- Windows: `%APPDATA%\\filo\\config.toml`

`filo init` writes this file for you. You can also edit it manually.

## Full example (default-ish starter)

```toml
other_category = "Other"

[watch]
folders = [
  "/home/you/Downloads",
]
auto_start = false
debounce_ms = 2000

[rename]
enabled = false
lowercase_with_hyphens = true
strip_redundant_words = true
prepend_date = false

[duplicates]
action = "skip"
folder_name = "Duplicates"

# Optional keyword routing in one block (first match wins):
# [keyword_rules]
# rules = [
#   { to = "Amazon", keywords = ["amazon"] },
#   { to = "Documents", keywords = ["invoice", "receipt", "statement"] },
#   { to = "~/personal/kanishk-itr", to_type = "path", keywords = ["itr"] },
# ]

[rules]
Archives  = ["zip", "tar", "gz", "bz2", "xz", "rar", "7z", "tgz"]
Audio     = ["mp3", "flac", "wav", "ogg", "m4a", "aac", "opus"]
Code      = ["rs", "py", "js", "ts", "go", "c", "cpp", "h", "hpp", "java", "rb", "sh", "toml", "json", "yaml", "yml"]
Documents = ["pdf", "doc", "docx", "odt", "rtf", "txt", "md", "epub"]
Images    = ["jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "tiff", "heic", "avif"]
Videos    = ["mp4", "mkv", "mov", "avi", "webm", "flv", "wmv", "m4v"]
```

## Option index

| Path | Type | Default | Used by |
|---|---|---|---|
| `other_category` | `string` | `"Other"` | `scan`, `preview`, `start` |
| `watch.folders` | `array<string(path)>` | `[]` | `scan`, `preview`, `start` |
| `watch.auto_start` | `bool` | `false` | `init` |
| `watch.debounce_ms` | `u64` | `2000` | `start` |
| `rename.enabled` | `bool` | `false` | `scan`, `start` |
| `rename.lowercase_with_hyphens` | `bool` | `true` | `scan`, `start` (when `rename.enabled=true`) |
| `rename.strip_redundant_words` | `bool` | `true` | `scan`, `start` (when `rename.enabled=true`) |
| `rename.prepend_date` | `bool` | `false` | `scan`, `start` (when `rename.enabled=true`) |
| `duplicates.action` | `"skip" \| "move"` | `"skip"` | `scan`, `preview`, `start` |
| `duplicates.folder_name` | `string` | `"Duplicates"` | `scan`, `preview`, `start` (when `duplicates.action="move"`) |
| `keyword_rules.rules` | `array<table>` | `[]` | `scan`, `preview`, `start` |
| `keyword_rules.rules[].to` | `string` | (required) | `scan`, `preview`, `start` |
| `keyword_rules.rules[].to_type` | `"folder" \| "path"` | `"folder"` | `scan`, `preview`, `start` |
| `keyword_rules.rules[].keywords` | `array<string>` | `[]` | `scan`, `preview`, `start` |
| `rules.<Category>` | `array<string(extension)>` | built-in per category | `scan`, `preview`, `start` |

## Detailed reference

### `other_category`

- Type: `string`
- Default: `"Other"`
- Meaning:
  Category name used when no extension rule matches a file.
- Runtime behavior:
  Files route to `<watch-root>/<other_category>/...`.
- Notes:
  Category names are also folder names, so avoid path separators.

### `[watch]`

#### `watch.folders`

- Type: `array<string(path)>`
- Default: `[]`
- Meaning:
  Root folders filo will scan/watch.
- Runtime behavior:
  - `scan` and `preview` process top-level files in each folder.
  - `start` watches each folder non-recursively for new/changed files.
- Notes:
  - `filo init` asks for absolute paths, but the parser accepts any valid path string.
  - If empty, `scan`/`preview`/`start` do nothing useful and print guidance.

#### `watch.auto_start`

- Type: `bool`
- Default: `false`
- Meaning:
  After `filo init` writes config, automatically start watcher mode.
- Runtime behavior:
  Used only by `filo init`.

#### `watch.debounce_ms`

- Type: `u64` (milliseconds)
- Default: `2000`
- Meaning:
  Idle window before a watched file is processed.
- Runtime behavior:
  Used by `filo start` watcher debounce loop.
- Notes:
  This does not affect `scan` or `preview`.

### `[rename]`

All fields in this section are ignored unless `rename.enabled = true`.

#### `rename.enabled`

- Type: `bool`
- Default: `false`
- Meaning:
  Turns on smart filename normalization before destination naming.
- Runtime behavior:
  Used by `scan` and `start`. Can be force-enabled at runtime with `--rename`.

#### `rename.lowercase_with_hyphens`

- Type: `bool`
- Default: `true`
- Meaning:
  Lowercase and normalize separators to single hyphens.
- Example:
  `"My  Cool File.pdf"` -> `"my-cool-file.pdf"`

#### `rename.strip_redundant_words`

- Type: `bool`
- Default: `true`
- Meaning:
  Removes adjacent repeated tokens in the stem.
- Example:
  `"FINAL_FINAL_report.pdf"` -> `"final-report.pdf"` (if lowercase normalization is also on)

#### `rename.prepend_date`

- Type: `bool`
- Default: `false`
- Meaning:
  Prepends local-date prefix (`YYYY-MM-DD`) to renamed stems.
- Example:
  `"report.pdf"` -> `"2026-05-02-report.pdf"` (date depends on run day)

### `[duplicates]`

#### `duplicates.action`

- Type: enum string
- Allowed values: `"skip"`, `"move"`
- Default: `"skip"`
- Meaning:
  How to handle byte-identical duplicates detected in destination category.
- Runtime behavior:
  - `"skip"`: source file remains in place.
  - `"move"`: source file moves to duplicate subfolder under category.

#### `duplicates.folder_name`

- Type: `string`
- Default: `"Duplicates"`
- Meaning:
  Name of subfolder used when `duplicates.action = "move"`.
- Runtime behavior:
  Duplicate destination is `<category-dir>/<duplicates.folder_name>/...`.

### `[keyword_rules]`

#### `keyword_rules.rules`

- Type: `array<table>`
- Default: `[]`
- Meaning:
  Ordered keyword routing rules evaluated before extension rules.
- Runtime behavior:
  - Rules are evaluated top-to-bottom, first matching rule wins.
  - Filename matching is case-insensitive substring matching.
  - Match mode is OR within each rule (`any` keyword hit matches).
  - Empty `to` values and empty keywords are ignored.
  - If no keyword rule matches, routing falls through to `[rules]`.
- Rule fields:
  - `to` (`string`): destination folder name, or path when `to_type = "path"`.
  - `to_type` (`"folder" | "path"`): how to read `to`. See below.
  - `keywords` (`array<string>`): match keywords for that rule.
- Example:
  `[keyword_rules]` with
  `rules = [{ to = "Documents", keywords = ["invoice", "receipt"] }]`
  routes `May_RECEIPT.png` to `Documents` with first-match precedence.

#### `keyword_rules.rules[].to_type`

- Type: enum string
- Allowed values: `"folder"`, `"path"`
- Default: `"folder"`
- Also accepted as: `destination_type` (alias)
- Meaning:
  Whether the rule's `to` value is a folder name to create inside the watch
  root, or a filesystem path of its own.
- Runtime behavior:
  - `"folder"`: destination is `<watch-root>/<to>/`. Unchanged historical
    behavior, applied whenever `to_type` is absent.
  - `"path"`: destination is `to` itself, resolved as follows:
    1. A leading `~` (or `~\` on Windows) expands to the home directory. A
       `~` anywhere else in the string is left literal. Environment variables
       are never expanded.
    2. If the result is absolute, it is used as-is.
    3. If the result is still relative, it is joined onto the watch root —
       so `to = "tax/2026"` from `~/Downloads` means `~/Downloads/tax/2026/`.
  - Missing destination directories (including parents) are created on the
    first move, not at config load.
  - Duplicate detection, collision-safe naming, and
    `duplicates.action = "move"` all operate on the resolved destination, so
    duplicates land in `<resolved-path>/<duplicates.folder_name>/`.
  - If a rule resolves to the directory the file already sits in, the file is
    left untouched and reported as `already in place` (`keep` in `preview`).
    This is checked against resolved paths, so symlinked and `.`-laden
    spellings of the same directory are recognized.
  - Only keyword rules support `to_type`. Extension rules in `[rules]` and the
    `other_category` fallback always route to a folder under the watch root.
- Notes:
  - `to_type = "folder"` is never written back to disk by `filo init`, since
    it is the default. Existing configs keep working with no edits.
  - `~` expansion needs a discoverable home directory. If there is none, the
    `~` stays literal and a warning is logged, which surfaces as a folder
    actually named `~`.
- Example:
  ```toml
  [keyword_rules]
  rules = [
    { to = "~/personal/kanishk-itr", to_type = "path", keywords = ["itr"] },
    { to = "/mnt/archive/scans", to_type = "path", keywords = ["scan"] },
    { to = "tax/2026", to_type = "path", keywords = ["form16"] },
    { to = "Amazon", keywords = ["amazon"] },
  ]
  ```
  Watching `~/Downloads`, `ITR-kanishk-2026.pdf` moves to
  `~/personal/kanishk-itr/`, `form16.pdf` to `~/Downloads/tax/2026/`, and
  `amazon-order.pdf` to `~/Downloads/Amazon/`.

### `[rules]`

- Type: map/table of category name -> extension list
- Default: built-in categories from `src/config/defaults.rs`
- Meaning:
  Routes files by extension to category folder when no keyword rule matched.
- Runtime behavior:
  - Extension matching is case-insensitive.
  - Files without extensions fall back to `other_category`.
  - Unknown extensions fall back to `other_category`.
- Format rules:
  - Use extensions without leading dot (`"pdf"`, not `".pdf"`).
  - Category key becomes folder name under each watch root.
- Conflict note:
  If an extension appears in multiple categories, the first category in sorted key order wins (because config uses a `BTreeMap`).

## Built-in default `rules` values

These are the built-in defaults used when no custom rules are set:

- `Documents`: `pdf`, `doc`, `docx`, `odt`, `rtf`, `txt`, `md`, `epub`
- `Images`: `jpg`, `jpeg`, `png`, `gif`, `webp`, `svg`, `bmp`, `tiff`, `heic`, `avif`
- `Videos`: `mp4`, `mkv`, `mov`, `avi`, `webm`, `flv`, `wmv`, `m4v`
- `Archives`: `zip`, `tar`, `gz`, `bz2`, `xz`, `rar`, `7z`, `tgz`
- `Audio`: `mp3`, `flac`, `wav`, `ogg`, `m4a`, `aac`, `opus`
- `Code`: `rs`, `py`, `js`, `ts`, `go`, `c`, `cpp`, `h`, `hpp`, `java`, `rb`, `sh`, `toml`, `json`, `yaml`, `yml`

## What config does not control (today)

- `arrange` command filters (`--keyword`, `--extension`) are CLI options, not TOML (separate from `[keyword_rules].rules`, which affects `scan`/`preview`/`start`).
- `scan`/`preview` depth is currently top-level only.
- `start` watcher is currently non-recursive.
- Keyword rules match on filename keywords only. Matching a keyword *and* an extension in one rule is not expressible in TOML today; use `filo arrange -k <keyword> -e <ext> -d <path>` for that.
- Path destinations are not watched implicitly. If you want files that land in a `to_type = "path"` destination to be organized further, add that path to `watch.folders` yourself.

## Extending this document for future TOML options

When adding any new config key, update this file in three places:

1. Add a row to the Option index table.
2. Add/update a detailed subsection under the correct table.
3. If defaults change, update the Full example and default lists.

Use this template for new options:

```md
#### `<table>.<key>`

- Type:
- Default:
- Allowed values:
- Meaning:
- Runtime behavior:
- Notes:
- Example:
```

Also update:

- `src/config/mod.rs` (schema + serde defaults)
- `src/config/defaults.rs` (constants/default maps as needed)
- tests for parse/default/runtime behavior
