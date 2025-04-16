use std::fs;
use std::path::{Path, PathBuf};
use log::{debug, warn};

use anyhow::{Result, bail};
use xshell::{cmd, Shell};
use email_address::EmailAddress;

pub struct Repo {
    prefix: PathBuf,
    sh: Shell,
}

impl Repo {
    pub fn init<P: AsRef<Path>>(prefix: P) -> Result<Repo> {
        let prefix = prefix.as_ref().to_path_buf();
        let sh = Shell::new()?;
        sh.change_dir(&prefix);
        cmd!(sh, "git init").run()?;
        Ok(Repo { prefix, sh })
    }

    pub fn load<P: AsRef<Path>>(prefix: P) -> Result<Repo> {
        debug!("loading git repository from {}...", prefix.as_ref().display());
        let sh = Shell::new()?;
        sh.change_dir(&prefix);
        debug!("git repository {} loaded", fs::canonicalize(&prefix)?.display());
        Ok(Repo { 
            prefix: prefix.as_ref().to_path_buf(),
            sh,
        })
    }

    pub fn add<P: AsRef<Path>>(&self, spec: P) -> Result<()> {
        let spec = spec.as_ref();
        let spec = match spec.starts_with(&self.prefix) {
            true => spec.strip_prefix(&self.prefix)?.to_path_buf(),
            false => spec.to_path_buf(),
        };

        let spec = spec
            .into_os_string()
            .into_string()
            .unwrap();
        cmd!(self.sh, "git add {spec}").run()?;
        Ok(())
    }

    pub fn commit(&self, msg: &str, author: Option<EmailAddress>) -> Result<()> {
        match author {
            Some(author) => {
                let author = author.to_string();
                cmd!(self.sh, "git commit --message {msg} --author {author}").run()?;
            },
            None => cmd!(self.sh, "git commit --message {msg}").run()?,
        }
        
        Ok(())
    }

    pub fn push(&self, retry: i32) -> Result<()> {
        for i in 0..retry {
            match cmd!(self.sh, "git pull --rebase").run() {
                Ok(_) => break,
                Err(e) => {
                    let msg = "failed to pull from remote";
                    warn!("{}: {} ({}/{})", msg, e, i+1, retry);
                    if i == retry - 1 {
                        bail!(msg);
                    }
                }
            }
        }
        for i in 0..retry {
            match cmd!(self.sh, "git push").run() {
                Ok(_) => break,
                Err(e) => {
                    let msg = "failed to push to remote";
                    warn!("{}: {} ({}/{})", msg, e, i+1, retry);
                    if i == retry - 1 {
                        bail!(msg);
                    }
                }
            }
        }
        Ok(())
    }

    /// Ensure the repository is clean and up-to-date that can be pushed changes.
    pub fn cleanup(&self) -> Result<()> {
        cmd!(self.sh, "git clean -d --force").run()?;
        cmd!(self.sh, "git reset --hard HEAD").run()?;
        Ok(())
    }
}
