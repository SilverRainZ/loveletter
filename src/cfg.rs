// TODO: use a cfg 3rd party crate
use std::fs;

use anyhow::Result;
use email_address::EmailAddress;
use log::info;
use serde_derive::{Deserialize, Serialize};
use toml;

use crate::utils::EmailAddressList;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cfg {
    pub imap: ImapCfg,
    pub archive: ArchiveCfg,
    pub runtime: RuntimeCfg,
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
    pub username: EmailAddress,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveCfg {
    // Data directories.
    pub letter_dir: String,        // dir of structured love letters
    pub rstdoc_dir: String,        // dir of generated reStructuredText docs
    pub create_dirs: Option<bool>, // whether to create data dirs automaticlly, true by default

    // Git integration.
    pub git_no_push: Option<bool>, // do not push changes to remote, true by default

    // Permssion control.
    pub allowed_from_addrs: EmailAddressList,
    pub allowed_to_addrs: EmailAddressList,
    pub overwrite: Option<bool>, // allow overwrite letters without "[edit]" action, false by default
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeCfg {
    pub interval: Option<u64>, // interval for checking new mails, in seconds, 60 by default
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cfg_load() {
        let _ = Cfg::load("./test_data/config.toml").unwrap();
    }
}
