use std::fs;
use std::path::{Path, PathBuf};
use log::debug;

use anyhow::{bail, Result};
use xshell::{cmd, Shell};
use email_address::EmailAddress;

pub struct Repo {
    prefix: PathBuf,
    sh: Shell,
}

impl Repo {
    pub fn load<P: AsRef<Path>>(prefix: P) -> Result<Repo> {
        debug!("loading git repository from {}...", prefix.as_ref().display());
        let prefix = fs::canonicalize(&prefix)?;
        let sh = Shell::new()?;
        sh.change_dir(&prefix);
        let prefix = PathBuf::from(cmd!(sh, "git rev-parse --show-toplevel").read()?);
        debug!("git repository {} loaded", prefix.display());
        Ok(Repo { prefix, sh })
    }

    pub fn add<P: AsRef<Path>>(&self, spec: P) -> Result<()> {
        let spec = match spec.as_ref().is_absolute() {
            true => {
                let p = fs::canonicalize(&spec)?;
                if !p.starts_with(&self.prefix) {
                    bail!("spec {} not in git repository {}", p.display(), self.prefix.display());
                }
                p.strip_prefix(&self.prefix)?.to_path_buf()
            },
            false => spec.as_ref().to_path_buf(),
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
