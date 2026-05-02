//! `filo start` — live-watch configured folders.

use anyhow::Result;

use crate::config::Config;
use crate::watcher;

pub fn run(config: &Config, rename_override: bool) -> Result<()> {
    let mut config = config.clone();
    if rename_override {
        config.rename.enabled = true;
    }
    watcher::run(&config)?;
    Ok(())
}
