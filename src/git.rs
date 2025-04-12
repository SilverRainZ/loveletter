use std::fs;
use std::path::{Path, PathBuf};
use log::debug;

use anyhow::Result;
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

    pub fn push(&self) -> Result<()> {
        cmd!(self.sh, "git push").run()?;
        Ok(())
    }

    pub fn is_clean(&self) -> Result<bool> {
        let stdout = cmd!(self.sh, "git status --short").read()?;
        Ok(stdout.is_empty())
    }
}
