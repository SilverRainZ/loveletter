use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use log::{debug, info};
use serde_derive::{Deserialize, Serialize};
use toml;

use crate::cfg::ArchiveCfg;
use crate::mail::ParsedMail;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoveLetter {
    // Meta information.
    from: String,
    to: String,
    from_meimei_if_true_and_gege_if_false: bool,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,

    // Content.
    date: NaiveDate,
    title: Option<String>,
    content: String,
}

pub struct Archive {
    cfg: ArchiveCfg,
    pub letter_dir: PathBuf,
    pub rst_dir: PathBuf,
}

impl Archive {
    const DATE_FMT: &str = "%Y-%m-%d";

    pub fn load(cfg: ArchiveCfg) -> Result<Archive> {
        fn create_dir(p: &Path, create_dirs: Option<bool>) -> Result<()> {
            if !p.exists() && create_dirs.unwrap_or(false) {
                info!("creating dir {}", p.display());
                fs::create_dir_all(p)?;
                info!("created");
            }
            Ok(())
        }

        let letter_dir = PathBuf::from(cfg.letter_dir.to_owned());
        create_dir(&letter_dir, cfg.create_dirs)?;
        let rst_dir = PathBuf::from(cfg.rst_dir.to_owned());
        create_dir(&rst_dir, cfg.create_dirs)?;

        Ok(Archive {
            cfg,
            letter_dir,
            rst_dir,
        })
    }

    /// Parse subject like "[ACTION] YYYY/MM/DD: TITLE", returns (date, title, action).
    fn parse_subject(subject: &str) -> Result<(NaiveDate, Option<String>, Option<String>)> {
        let ptr: &str = subject.trim();

        // Extract title from "...: TITLE".
        debug!("extracting title from {:?}...", ptr);
        let (ptr, title) = match ptr.split_once(':') {
            // TODO: support '：'
            Some((ptr, title)) => (ptr, Some(title)),
            None => (ptr, None),
        };
        let ptr = ptr.trim();
        let title = title
            .map(str::trim)
            .filter(|&x| !x.is_empty())
            .map(str::to_owned);
        debug!("title: {:?}", title);

        // Extract action from "[ACTION] YYYY/MM/DD...".
        debug!("extracting action from {:?}...", ptr);
        let (action, ptr) = match ptr.split_once(']') {
            // TODO: support '：'
            Some((action, ptr)) => {
                let action = match action.split_once('[') {
                    Some((_, action)) => action,
                    None => bail!("unmatched square brackets"),
                };
                (Some(action), ptr)
            }
            None => (None, ptr),
        };
        let ptr = ptr.trim();
        let action = action
            .map(str::trim)
            .filter(|&x| !x.is_empty())
            .map(str::to_owned);
        debug!("action: {:?}", action);

        // Extract year/month/day from "YYYY/MM/DD".
        debug!("extracting date from {:?}...", ptr);
        let mut splits = ptr.splitn(3, '/');
        let year: i32 = splits.next().context("expect date *YYYY*/MM/DD")?.parse()?;
        let month = splits.next().context("expect date YYYY/*MM*/DD")?.parse()?;
        let day: u32 = splits.next().context("expect date YYYY/MM/*DD*")?.parse()?;
        let date =
            NaiveDate::from_ymd_opt(year, month, day).context("failed to creart native date")?;
        debug!("date: {}", date);

        Ok((date, title, action))
    }

    pub fn write(&self, mail: &ParsedMail) -> Result<LoveLetter> {
        let from = mail
            .from()
            .context("failed to extract mail sender's address")?;
        if !self.cfg.allowed_from_addrs.contains(&from.to_owned()) {
            // FIXME: why?
            bail!(
                "sender {} not in allowed list {:?}",
                from,
                self.cfg.allowed_from_addrs
            )
        }
        let to = mail
            .to()
            .context("failed to extract mail recipient's address")?;
        if !self.cfg.allowed_to_addrs.contains(&to.to_owned()) {
            bail!(
                "recipient {} not in allowed list {:?}",
                to,
                self.cfg.allowed_to_addrs
            )
        }

        let subject = mail.subject().context("failed to extract mail subject")?;
        let (date, title, action) =
            Self::parse_subject(subject).context("failed to parse mail subject:")?;
        let body = mail.body().context("failed to extract mail body")?;
        let letter_path = self.letter_path(&date);
        let letter_exists = letter_path.exists();
        info!("writing letter (date: {}, title: {:?}, action: {:?}) to {} (exist: {})...", 
            date, title, action, letter_path.display(), letter_exists);

        let letter = LoveLetter {
            from: from.to_owned(),
            to: to.to_owned(),
            from_meimei_if_true_and_gege_if_false: true, // TODO: distinguish gege and meimei
            created_at: mail.date(),                     // TODO: update for edit
            updated_at: mail.date(),

            date,
            title,
            content: body.join("\n"),
        };
        match action.as_deref() {
            None | Some("") | Some("new") => {
                if letter_exists {
                    bail!(
                        "letter of {} already exists: {}",
                        date,
                        letter_path.display()
                    );
                }
                let letter_data = toml::to_string(&letter)?;
                fs::write(&letter_path, letter_data).
                    with_context(|| format!("{}", letter_path.display()))?;
                // TOOD: git commit
            }
            // Some("edit") => {
            //     // TODO:
            // },
            Some(x) => bail!("unknown action: {}", x),
        }
        info!("wrote");

        Ok(letter)
    }

    pub fn read(&self, date: NaiveDate) -> Result<LoveLetter> {
        let letter_path = self.letter_path(&date);
        let letter_data = fs::read_to_string(letter_path)?;
        let letter: LoveLetter = toml::from_str(&letter_data)?;
        Ok(letter)
    }

    pub fn letter_path(&self, date: &NaiveDate) -> PathBuf {
        let mut p = self.letter_dir.clone();
        p.push(date.format(Self::DATE_FMT).to_string() + ".toml");
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{tempdir, TempDir};
    use crate::mail::RawMail;
    use crate::cfg::Cfg;

    #[test]
    fn test_archive_parse_subject() {
        assert_eq!(
            Archive::parse_subject("[edit] 1998/01/28: 妹妹生日快乐").unwrap(),
            (
                NaiveDate::from_ymd_opt(1998, 1, 28).unwrap(),
                Some("妹妹生日快乐".to_string()),
                Some("edit".to_string())
            )
        );
        assert_eq!(
            Archive::parse_subject("[edit] 1998/01/28:妹妹生日快乐").unwrap(),
            (
                NaiveDate::from_ymd_opt(1998, 1, 28).unwrap(),
                Some("妹妹生日快乐".to_string()),
                Some("edit".to_string())
            )
        );
        assert_eq!(
            Archive::parse_subject("[edit]1998/01/28:妹妹生日快乐").unwrap(),
            (
                NaiveDate::from_ymd_opt(1998, 1, 28).unwrap(),
                Some("妹妹生日快乐".to_string()),
                Some("edit".to_string())
            )
        );
        assert_eq!(
            Archive::parse_subject("[edit] 1998/01/28").unwrap(),
            (
                NaiveDate::from_ymd_opt(1998, 1, 28).unwrap(),
                None,
                Some("edit".to_string())
            )
        );
        assert_eq!(
            Archive::parse_subject("[edit]1998/01/28").unwrap(),
            (
                NaiveDate::from_ymd_opt(1998, 1, 28).unwrap(),
                None,
                Some("edit".to_string())
            )
        );
        assert_eq!(
            Archive::parse_subject("[edit] 1998/01/28:").unwrap(),
            (
                NaiveDate::from_ymd_opt(1998, 1, 28).unwrap(),
                None,
                Some("edit".to_string())
            )
        );
        assert_eq!(
            Archive::parse_subject("1998/01/28: 妹妹生日快乐").unwrap(),
            (
                NaiveDate::from_ymd_opt(1998, 1, 28).unwrap(),
                Some("妹妹生日快乐".to_string()),
                None
            )
        );
        assert_eq!(
            Archive::parse_subject("1998/01/28:妹妹生日快乐").unwrap(),
            (
                NaiveDate::from_ymd_opt(1998, 1, 28).unwrap(),
                Some("妹妹生日快乐".to_string()),
                None
            )
        );
        assert_eq!(
            Archive::parse_subject("1998/01/28:").unwrap(),
            (NaiveDate::from_ymd_opt(1998, 1, 28).unwrap(), None, None)
        );
        assert_eq!(
            Archive::parse_subject("1998/01/28").unwrap(),
            (NaiveDate::from_ymd_opt(1998, 1, 28).unwrap(), None, None)
        );
    }

    #[test]
    fn test_archive_write() {
        fn tmpdir_path(d: &TempDir) -> String {
                d.path()
                .to_str()
                .unwrap()
                .to_owned()
        }
        let mut cfg = Cfg::load("./test_data/config.toml").unwrap().archive;
        let tmp_letter_dir = tempdir().unwrap();
        cfg.letter_dir = tmpdir_path(&tmp_letter_dir);
        let tmp_rst_dir = tempdir().unwrap();
        cfg.rst_dir = tmpdir_path(&tmp_rst_dir);
        let archive = Archive::load(cfg).unwrap();

        let data = fs::read_to_string("./test_data/mail1.txt").unwrap();
        let raw_mail = RawMail::new(&data);
        let parsed_mail = raw_mail.parse().unwrap();
        let (date, _, _) = Archive::parse_subject(parsed_mail.subject().unwrap()).unwrap();

        assert!(archive.read(date).is_err()); // test read non-exist
        let letter = archive.write(&parsed_mail).unwrap();
        assert!(archive.write(&parsed_mail).is_err()); // test duplicate writing

        let letter2 = archive.read(date).unwrap();
        assert_eq!(letter, letter2);
    }
}
