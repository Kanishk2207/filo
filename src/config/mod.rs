//! Config loading, saving, and the canonical schema.
//!
//! The on-disk format is TOML and the user is expected to edit it by hand.
//! Field ordering in [`Config`] is chosen so the serializer emits scalars
//! before tables — TOML forbids the reverse at the same level.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::ConfigError;

pub mod defaults;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Category name used when no rule matches a file's extension.
    #[serde(default = "default_other_category")]
    pub other_category: String,

    #[serde(default)]
    pub watch: WatchConfig,

    #[serde(default)]
    pub rename: RenameConfig,

    #[serde(default)]
    pub duplicates: DuplicatesConfig,

    /// Ordered keyword-routing rules. Evaluated top-to-bottom.
    ///
    /// If a filename matches any keyword in a rule, that rule's `to`
    /// destination wins before extension-based routing is considered.
    #[serde(
        default,
        alias = "keyword_rule",
        skip_serializing_if = "KeywordRuleConfig::is_empty"
    )]
    pub keyword_rules: KeywordRuleConfig,

    /// Category → list of lowercase extensions (without the leading dot).
    #[serde(default = "defaults::default_rules")]
    pub rules: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfig {
    #[serde(default)]
    pub folders: Vec<PathBuf>,

    #[serde(default)]
    pub auto_start: bool,

    /// Idle period (ms) a file must be quiet for before it is acted on.
    /// Protects against operating on files that are still being written.
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default = "true_")]
    pub lowercase_with_hyphens: bool,

    #[serde(default = "true_")]
    pub strip_redundant_words: bool,

    #[serde(default)]
    pub prepend_date: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicatesConfig {
    #[serde(default)]
    pub action: DuplicateAction,

    #[serde(default = "default_duplicates_folder")]
    pub folder_name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DuplicateAction {
    /// Leave the duplicate in place; just log it.
    #[default]
    Skip,
    /// Move the duplicate into a dedicated subfolder next to the category
    /// destination.
    Move,
}

/// How a rule's `to` value should be read.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DestinationKind {
    /// `to` is a folder name, created inside the watch folder the file came
    /// from: `<watch-root>/<to>/`. This is the historical behavior and stays
    /// the default so existing configs are unaffected.
    #[default]
    Folder,
    /// `to` is a filesystem path, independent of the watch folder. A leading
    /// `~` expands to the home directory; a relative path is anchored to the
    /// watch folder the file came from.
    Path,
}

impl DestinationKind {
    /// Used by serde to keep `to_type = "folder"` out of written configs.
    pub fn is_folder(&self) -> bool {
        matches!(self, DestinationKind::Folder)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct KeywordRule {
    /// Destination folder name, or path when `to_type = "path"`.
    pub to: String,

    /// Whether `to` names a folder inside the watch root or a full path.
    #[serde(
        default,
        alias = "destination_type",
        skip_serializing_if = "DestinationKind::is_folder"
    )]
    pub to_type: DestinationKind,

    #[serde(default)]
    pub keywords: Vec<String>,
}

impl KeywordRule {
    /// Rule whose `to` is a folder name created inside the watch root.
    pub fn folder(to: impl Into<String>, keywords: Vec<String>) -> Self {
        Self {
            to: to.into(),
            to_type: DestinationKind::Folder,
            keywords,
        }
    }

    /// Rule whose `to` is a filesystem path outside (or inside) the watch root.
    pub fn path(to: impl Into<String>, keywords: Vec<String>) -> Self {
        Self {
            to: to.into(),
            to_type: DestinationKind::Path,
            keywords,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct KeywordRuleConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<KeywordRule>,
}

impl KeywordRuleConfig {
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

fn default_other_category() -> String {
    defaults::DEFAULT_OTHER_CATEGORY.to_string()
}

fn default_duplicates_folder() -> String {
    defaults::DEFAULT_DUPLICATES_FOLDER.to_string()
}

fn default_debounce_ms() -> u64 {
    defaults::DEFAULT_DEBOUNCE_MS
}

fn true_() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            other_category: default_other_category(),
            watch: WatchConfig::default(),
            rename: RenameConfig::default(),
            duplicates: DuplicatesConfig::default(),
            keyword_rules: KeywordRuleConfig::default(),
            rules: defaults::default_rules(),
        }
    }
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            folders: Vec::new(),
            auto_start: false,
            debounce_ms: default_debounce_ms(),
        }
    }
}

impl Default for RenameConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            lowercase_with_hyphens: true,
            strip_redundant_words: true,
            prepend_date: false,
        }
    }
}

impl Default for DuplicatesConfig {
    fn default() -> Self {
        Self {
            action: DuplicateAction::Skip,
            folder_name: default_duplicates_folder(),
        }
    }
}

impl Config {
    /// Platform-appropriate config path. Uses `dirs::config_dir()` so it
    /// resolves correctly on Linux/macOS (`~/.config/filo/config.toml`)
    /// and Windows (`%APPDATA%\filo\config.toml`).
    pub fn path() -> Result<PathBuf, ConfigError> {
        let base = dirs::config_dir().ok_or(ConfigError::NoConfigDir)?;
        Ok(base.join("filo").join("config.toml"))
    }

    pub fn exists() -> bool {
        Self::path().map(|p| p.exists()).unwrap_or(false)
    }

    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::path()?;
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Err(ConfigError::NotFound(path.to_path_buf()));
        }
        let raw = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let config: Config = toml::from_str(&raw).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(config)
    }

    pub fn save(&self) -> Result<PathBuf, ConfigError> {
        let path = Self::path()?;
        self.save_to(&path)?;
        Ok(path)
    }

    pub fn save_to(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| ConfigError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let serialized = toml::to_string_pretty(self)?;
        fs::write(path, serialized).map_err(|source| ConfigError::Write {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(())
    }

    /// Add a folder to the watch list. Returns `true` if it was actually
    /// added (i.e. not already present).
    pub fn add_folder(&mut self, path: PathBuf) -> bool {
        if self.watch.folders.iter().any(|p| p == &path) {
            return false;
        }
        self.watch.folders.push(path);
        true
    }

    /// Remove a folder from the watch list. Returns `true` if it was
    /// actually removed (i.e. was present).
    pub fn remove_folder(&mut self, path: &Path) -> bool {
        let before = self.watch.folders.len();
        self.watch.folders.retain(|p| p != path);
        self.watch.folders.len() < before
    }
}
