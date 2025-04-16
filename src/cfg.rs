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
    #[serde(default = "yes")]
    pub create_dirs: bool, // whether to create data dirs automaticlly, true by default

    // Git integration.
    #[serde(default = "yes")]
    pub git_no_push: bool, // whether to push changes to remote
    #[serde(default = "no")]
    pub git_pre_cleanup: bool, // clean up repo before any operation
    #[serde(default = "i32_3")]
    pub git_retry: i32,

    // Permssion control.
    pub allowed_from_addrs: EmailAddressList,
    pub allowed_to_addrs: EmailAddressList,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeCfg {
    #[serde(default = "u64_60")]
    pub interval: u64, // interval for checking new mails, in seconds
}

fn yes() -> bool { true }
fn no() -> bool { false }
fn i32_3() -> i32 { 3 }
fn u64_60() -> u64 { 60 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cfg_load() {
        let _ = Cfg::load("./test_data/config.toml").unwrap();
    }
}
