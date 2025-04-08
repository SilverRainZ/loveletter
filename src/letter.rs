use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use log::{debug, info};
use serde_derive::{Deserialize, Serialize};
use toml;
use unicode_width::UnicodeWidthStr;
use email_address::EmailAddress;

use crate::cfg::ArchiveCfg;
use crate::mail::ParsedMail;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoveLetter {
    // Meta information.
    from: EmailAddress,
    to: EmailAddress,
    from_meimei_if_true_and_gege_if_false: bool,

    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,

    // Content.
    date: NaiveDate,
    title: Option<String>,
    content: String,
}

impl LoveLetter {
    const DATE_FMT: &str = "%Y-%m-%d";
    const YEAR_FMT: &str = "%Y";

    fn load<P: AsRef<Path>>(p: P) -> Result<LoveLetter> {
        let data = fs::read_to_string(p)?;
        let letter: LoveLetter = toml::from_str(&data)?;
        Ok(letter)
    }

    fn to_rstdoc_heading(&self) -> String {
        // Document title:
        //
        // ```rst
        // =========================
        // 💌 Love Letters from YEAR
        // =========================
        // ```
        let title = format!("💌  Love Letter from {}", self.date.format(Self::YEAR_FMT));
        let delim = "=".repeat(title.width_cjk());
        delim.to_string() + "\n" + &title + "\n" + &delim + "\n\n"
    }

    // convert to reStructuredText.
    fn to_rstdoc_section(&self) -> String {
        let mut buf = String::new();

        // Section title:
        //
        // ```rst
        // DATE: TITLE
        // ===========
        // ```
        let date_str = format!("{}", self.date.format(Self::DATE_FMT));
        let title = date_str.to_owned() + &(match &self.title {
            Some(t) => ": ".to_string() + &t,
            None => "".to_string(),
        });
        buf.push_str(&title);
        buf.push('\n');
        buf.push_str(&"=".repeat(title.width_cjk())); // title delim
        buf.push('\n');

        // Push loveletter directive.
        buf.push_str(&format!("
.. loveletter:: _
   :date: {}
   :author: {}
   :createdat: {}
   :updatedat: {}

   {}
",
           date_str,
           self.from.display_part(),
           &self.created_at.map(|x| x.format(Self::DATE_FMT).to_string()).unwrap_or("".to_string()),
           &self.updated_at.map(|x| x.format(Self::DATE_FMT).to_string()).unwrap_or("".to_string()),
           self.content,
           ));
        buf.push('\n');

        buf
    }
}

pub struct Archive {
    cfg: ArchiveCfg,
    letter_dir: PathBuf,
    rstdoc_dir: PathBuf,
}

impl Archive {
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
        let rstdoc_dir = PathBuf::from(cfg.rstdoc_dir.to_owned());
        create_dir(&rstdoc_dir, cfg.create_dirs)?;

        Ok(Archive {
            cfg,
            letter_dir,
            rstdoc_dir,
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

    pub fn upsert_letter(&self, mail: &ParsedMail) -> Result<LoveLetter> {
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

        let from_meimei_if_true_and_gege_if_false = from.display_part().contains("妹妹");
        let letter = LoveLetter {
            from, to, from_meimei_if_true_and_gege_if_false,
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

    pub fn get_letter(&self, date: NaiveDate) -> Result<LoveLetter> {
        LoveLetter::load(self.letter_path(&date))
    }

    pub fn letter_path(&self, date: &NaiveDate) -> PathBuf {
        let mut p = self.letter_dir.clone();
        p.push(format!("{}.toml", date.format(LoveLetter::DATE_FMT)));
        p
    }

    pub fn generate_rstdoc(&self) -> Result<()> {
        // Clear rstdoc_dir.
        info!("clearing rstdoc dir {}...", self.rstdoc_dir.display());
        fs::remove_dir_all(&self.rstdoc_dir).unwrap_or(());
        fs::create_dir(&self.rstdoc_dir)?;
        info!("cleared");

        // Generate index.rst
        let index_path = self.rstdoc_index_path();
        info!("generating love letter index {}...", index_path.display());
        fs::write(index_path, "\
===============
💌 Love Letters
===============

.. hint::
   Generated from :ghrepo:`SilverRainZ/loveletter`.

.. toctree::
   :glob:

   *
"
        )?;
        info!("generated");
        
        info!("listing letter dir {}...", self.letter_dir.display());
        let mut entries: Vec<_> = fs::read_dir(&self.letter_dir)?
            .map(|e| e.map(|e| e.path()))
            .collect::<Result<Vec<_>, io::Error>>()?.
            into_iter().
            filter(|e| *e != self.rstdoc_index_path()).
            collect();
        info!("found {} letters: letter dir {:?}...", entries.len(), entries);

        // The order in which `read_dir` returns entries is not guaranteed. If reproducible
        // ordering is required the entries should be explicitly sorted.
        entries.sort();

        for entry in entries {
            let letter = LoveLetter::load(entry)?;
            let mut file = self.rstdoc_dir.to_owned();
            file.push(self.rstdoc_path(&letter.date));

            info!("writing letter {} to rstdoc {}..." , letter.date, file.display());
            if !file.exists() {
                // Create new file if non-exist.
                info!("created new rstdoc");
                fs::write(&file, letter.to_rstdoc_heading())?;
            }
            // Append to existing file.
            OpenOptions::new().append(true).open(file)?.write(letter.to_rstdoc_section().as_bytes())?;
            info!("wrote");
        }

        Ok(())
    }

    pub fn rstdoc_path(&self, date: &NaiveDate) -> PathBuf {
        let mut p = self.rstdoc_dir.clone();
        p.push(format!("{}.rst", date.format(LoveLetter::YEAR_FMT)));
        p
    }

    pub fn rstdoc_index_path(&self) -> PathBuf {
        let mut p = self.rstdoc_dir.clone();
        // https://www.sphinx-doc.org/en/master/usage/configuration.html#confval-master_doc
        p.push("index.rst");
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
    fn test_archive_upsert_and_get_letter() {
        fn tmpdir_path(d: &TempDir) -> String {
                d.path()
                .to_str()
                .unwrap()
                .to_owned()
        }
        let mut cfg = Cfg::load("./test_data/config.toml").unwrap().archive;
        let tmp_letter_dir = tempdir().unwrap();
        cfg.letter_dir = tmpdir_path(&tmp_letter_dir);
        let tmp_rstdoc_dir = tempdir().unwrap();
        cfg.rstdoc_dir = tmpdir_path(&tmp_rstdoc_dir);
        let archive = Archive::load(cfg).unwrap();

        let data = fs::read_to_string("./test_data/mail.txt").unwrap();
        let raw_mail = RawMail::new(&data);
        let parsed_mail = raw_mail.parse().unwrap();
        let (date, _, _) = Archive::parse_subject(parsed_mail.subject().unwrap()).unwrap();

        assert!(archive.get_letter(date).is_err()); // test read non-exist
        let letter = archive.upsert_letter(&parsed_mail).unwrap();
        assert!(archive.upsert_letter(&parsed_mail).is_err()); // test duplicate writing

        // Test TOML.
        assert_eq!(fs::read_to_string(archive.letter_path(&date)).unwrap(),
        fs::read_to_string("./test_data/2025-04-03.toml").unwrap());

        // Test read and write consistency.
        let letter2 = archive.get_letter(date).unwrap();
        assert_eq!(letter, letter2);

        archive.generate_rstdoc().unwrap();
        assert_eq!(fs::read_to_string(archive.rstdoc_index_path()).unwrap(),
        fs::read_to_string("./test_data/index.rst").unwrap());
        assert_eq!(fs::read_to_string(archive.rstdoc_path(&date)).unwrap(),
        fs::read_to_string("./test_data/2025.rst").unwrap());
    }
}
