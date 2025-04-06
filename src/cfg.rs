use std::fs;

use anyhow::Result;
use log::{debug, info, warn, error};
use serde_derive::{Deserialize, Serialize};
use toml;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cfg {
    pub imap: ImapCfg,
    pub archive: ArchiveCfg,
}

impl Cfg {
    pub fn load(path: &str) -> Result<Cfg> {
        info!("loading configuration from {}...", path);
        let cfg_data = fs::read_to_string(path)?;
        let cfg: Cfg = toml::from_str(&cfg_data)?;
        info!("loaded");
        Ok(cfg)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapCfg {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveCfg {
    // Data directories.
    pub letter_dir: String, // dir of structured love letters
    pub rstdoc_dir: String, // dir of generated reStructuredText docs
    pub create_dirs: Option<bool>, // whether to create data dirs automaticlly

    // Permssion control.
    pub allowed_from_addrs: Vec<String>,
    pub allowed_to_addrs: Vec<String>,
    // pub allow_edit: bool, // allow sender edits existing love letter
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cfg_load() {
        assert!(Cfg::load("./test_data/config.toml").is_ok());
    }
}

