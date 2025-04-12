use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::str::pattern::Pattern;
use std::fmt;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use email_address::EmailAddress;
use log::{debug, info, warn, error};
use serde_derive::{Deserialize, Serialize};
use serde::ser;
use serde::de;
use toml;
use unicode_width::UnicodeWidthStr;
use base64::{engine::general_purpose::URL_SAFE, Engine as _};

use crate::cfg::ArchiveCfg;
use crate::mail::ParsedMail;
use crate::git::Repo;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoveLetter {
    // Meta information.
    from: EmailAddress,
    to: EmailAddress,
    from_meimei_if_true_and_gege_if_false: bool,

    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,

    // Content.
    date: Date,
    title: Option<String>,
    content: String,
}

impl LoveLetter {
    fn load<P: AsRef<Path>>(p: P) -> Result<LoveLetter> {
        let data = fs::read_to_string(p)?;
        let letter: LoveLetter = toml::from_str(&data)?;
        Ok(letter)
    }

    fn rstdoc_heading(&self) -> String {
        // Document title:
        //
        // ```rst
        // =========================
        // 💌 Love Letters from YEAR
        // =========================
        // ```
        let title = format!("💌  Love Letters from {}", self.date.year);
        let delim = "=".repeat(title.width_cjk());
        delim.to_string() + "\n" + &title + "\n" + &delim + "\n\n"
    }

    // convert to reStructuredText.
    fn rstdoc_section(&self) -> String {
        let mut buf = String::new();

        // Section title:
        //
        // ```rst
        // DATE: TITLE
        // ===========
        // ```
        let title = self.date.to_string()
            + &(match &self.title {
                Some(t) => ": ".to_string() + &t,
                None => "".to_string(),
            });
        buf.push_str(&title);
        buf.push('\n');
        buf.push_str(&"=".repeat(title.width_cjk())); // title delim
        buf.push('\n');

        // Push loveletter directive.
        buf.push_str(&format!(
            "
.. loveletter:: _
   :date: {}
   :nick: {}
   :author: {}
   :createdat: {}
   :updatedat: {}

   .. raw:: html

      {}
",
            self.date.to_string(),
            self.from.display_part(),
            self.author(),
            &self
                .created_at
                .map(|x| x.format(Date::FMT).to_string())
                .unwrap_or("".to_string()),
            &self
                .updated_at
                .map(|x| x.format(Date::FMT).to_string())
                .unwrap_or("".to_string()),
            self.content,
        ));
        buf.push('\n');

        buf
    }

    fn author(&self) -> &str {
        if self.from_meimei_if_true_and_gege_if_false {
            "妹妹"
        } else {
            "哥哥"
        }
    }

    fn letter_filename(&self) -> String {
        match &self.title {
            Some(title) => format!("{}_{}.toml", self.date, URL_SAFE.encode(&title)),
            None => self.date.to_string() + ".toml",
        }
    }

    fn rstdoc_filename(&self) -> String {
        return self.date.year.to_string() + ".toml"
    }
}

impl fmt::Display for LoveLetter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.title {
            Some(title) => write!(f, "<{}: {}>", self.date, title),
            None => write!(f, "<{}>", self.date),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: Option<u32>,
}

impl Date {
    const FMT: &str = "%Y-%m-%d";

    fn parse<P: Pattern>(s: &str, delim: P) -> Result<Date> {
        // Extract year/month/day from "YYYY/MM/[DD]".
        let mut splits = s.splitn(3, delim);
        let year: i32 = splits.next().context("expect date *YYYY*/MM/DD")?.parse()?;
        let month = splits.next().context("expect date YYYY/*MM*/DD")?.parse()?;
        let day = splits.next().map(|x| x.parse::<u32>()).transpose()?;
        Ok(Date{ year, month, day })
    }

    fn from_subject(s: &str) -> Result<Date> {
        Self::parse(s, "/")
    }

    fn from_filename(s: &str) -> Result<Date> {
        Self::parse(s, "-")
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (year, month, day) = (self.year, self.month, self.day.unwrap_or(1));
        let fmt = match self.day {
            Some(_) => Self::FMT,
            None => "%Y-%m",
        };
        let s = match NaiveDate::from_ymd_opt(year, month, day) {
            Some(d) => d.format(fmt).to_string(),
            None => {
                error!("failed to create native date from {:?}", self);
                "".to_string()
            }
        };
        write!(f, "{}", s)
    }
}

impl ser::Serialize for Date {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

struct DateVisitor;

impl<'de> de::Visitor<'de> for DateVisitor {
    type Value = Date;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Date::from_filename(value).map_err(|e| E::custom(format!("{}", e)))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
        Date::from_filename(&value).map_err(|e| E::custom(format!("{}", e)))
    }
}

impl<'de> de::Deserialize<'de> for Date {
    fn deserialize<D>(deserializer: D) -> Result<Date, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_str(DateVisitor)
    }
}

pub struct Archive {
    cfg: ArchiveCfg,
    letter_dir: PathBuf,
    rstdoc_dir: PathBuf,
    letter_git_repo: Repo,
    rstdoc_git_repo: Repo,
}

impl Archive {
    pub fn load(cfg: ArchiveCfg) -> Result<Archive> {
        fn create_dir(p: &Path, create_dirs: Option<bool>) -> Result<()> {
            if !p.exists() && create_dirs.unwrap_or(true) {
                info!("creating dir {}", p.display());
                fs::create_dir_all(p)?;
                info!("created");
            }
            Ok(())
        }

        fn load_repo(p: &Path, create_dirs: Option<bool>) -> Result<Repo> {
            Repo::load(p).or_else(|e| if create_dirs.unwrap_or(true) {
                    Repo::init(p)
                }  else {
                    Err(e)
                })
        }

        let letter_dir = PathBuf::from(cfg.letter_dir.to_owned());
        create_dir(&letter_dir, cfg.create_dirs)?;
        let letter_git_repo = load_repo(&letter_dir, cfg.create_dirs)?;
        let rstdoc_dir = PathBuf::from(cfg.rstdoc_dir.to_owned());
        create_dir(&rstdoc_dir, cfg.create_dirs)?;
        let rstdoc_git_repo = load_repo(&rstdoc_dir, cfg.create_dirs)?;

        Ok(Archive {
            cfg,
            letter_dir,
            rstdoc_dir,
            letter_git_repo,
            rstdoc_git_repo,
        })
    }

    /// Parse subject like "[ACTION] YYYY/MM/DD: TITLE", returns (date, title, action).
    fn parse_subject(subject: &str) -> Result<(Date, Option<String>, Option<String>)> {
        let ptr: &str = subject.trim();

        // Extract title from "...: TITLE".
        debug!("extracting title from {:?}...", ptr);
        let (ptr, title) = match ptr.split_once(':') {
            // TODO: support '：'
            Some((ptr, title)) => (ptr, Some(title)),
            None => match ptr.split_once('：') { // CJK chars compat
                Some((ptr, title)) => (ptr, Some(title)),
                None => (ptr, None),
            },
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
        let date = Date::from_subject(ptr)?;
        debug!("date: {}", date);

        Ok((date, title, action))
    }

    fn is_from_meimei_or_gege(&self, addr: &EmailAddress) -> Result<bool> {
        let matched = self.cfg.allowed_from_addrs.find(addr).context("mail is not allowed: {}")?;
        match matched.display_part() {
            "妹妹" => Ok(true),
            "哥哥" => Ok(false),
            _ => bail!("name in address {} is unknown, only {} or {} is allowed",
                addr.display_part(), "哥哥", "妹妹"),
        }
    }

    // TODO: dedup by Message-ID? need index.
    pub fn upsert_letter(&self, mail: &ParsedMail) -> Result<LoveLetter> {
        let from = mail
            .from()
            .context("failed to extract mail sender's address")?;
        let from = match self.cfg.allowed_from_addrs.find(&from) {
            Some(a) => if from.display_part().is_empty() {
                a.to_owned()
            } else {
                from
            },
            None => bail!(
                "sender {} not in allowed list {:?}",
                from,
                self.cfg.allowed_from_addrs
            ),
        };
        let to = mail
            .to()
            .context("failed to extract mail recipient's address")?;
        let to = match self.cfg.allowed_to_addrs.find(&to) {
            Some(a) => if to.display_part().is_empty() {
                a.to_owned()
            } else {
                to
            },
            None => bail!(
                "recipient {} not in allowed list {:?}",
                to,
                self.cfg.allowed_to_addrs
            ),
        };
        let subject = mail.subject().context("failed to extract mail subject")?;
        let (date, title, action) =
            Self::parse_subject(subject).context("failed to parse mail subject:")?;
        let content = mail.html_body().context("failed to extract mail body")?;

        // Combine the aboved fields together.
        let mut letter = LoveLetter {
            from: from.clone(),
            to,
            from_meimei_if_true_and_gege_if_false: self.is_from_meimei_or_gege(&from)?,
            created_at: mail.date(), // TODO: update for edit
            updated_at: mail.date(),

            date,
            title,
            content,
        };

        let letter_path = self.letter_path(&letter);
        let letter_exists = letter_path.exists();
        info!(
            "writing letter {} (action: {:?}) to {} (exist: {})...",
            letter,
            action,
            letter_path.display(),
            letter_exists
        );

        // Premission checks.
        match action.as_deref() {
            None => if letter_exists && !self.cfg.overwrite.unwrap_or(false) {
                bail!("letter {} already exists: {} ", &letter, letter_path.display());
            }
            Some("edit") => (), // pass
            Some(x) => bail!("unknown action: {}", x),
        }

        if letter_exists {
            warn!("editing existing letter {}: {},", letter, letter_path.display());
            letter.created_at = LoveLetter::load(&letter_path)?.created_at;
        }
        let letter_data = toml::to_string(&letter)?;
        fs::write(&letter_path, letter_data)
            .with_context(|| format!("{}", letter_path.display()))?;
        info!("wrote");

        self.letter_git_repo.add(&letter_path)?;
        self.letter_git_repo.commit(&("[loveletter] ".to_owned() + subject), Some(from.clone()))?;
        if !self.cfg.git_no_push.unwrap_or(true) {
            self.letter_git_repo.push()?;
        }

        Ok(letter)
    }

    pub fn letter_path(&self, letter: &LoveLetter) -> PathBuf {
        let mut p = self.letter_dir.clone();
        p.push(letter.letter_filename());
        p
    }

    pub fn generate_rstdoc(&self) -> Result<()> {
        // Generate index.rst
        let index_path = self.rstdoc_index_path();
        info!("generating love letter index {}...", index_path.display());
        fs::write(
            &index_path,
            "\
===============
💌 Love Letters
===============

.. hint::
   Generated from :ghrepo:`SilverRainZ/loveletter`.

.. toctree::
   :glob:

   *
",
        )?;
        self.rstdoc_git_repo.add(&index_path)?;
        info!("generated");

        info!("listing letter dir {}...", self.letter_dir.display());
        let mut entries: Vec<_> = fs::read_dir(&self.letter_dir)?
            .map(|e| e.map(|e| e.path()))
            .collect::<Result<Vec<_>, io::Error>>()?
            .into_iter()
            .filter(|e| e.is_file())
            .filter(|e| e.extension() == Some(OsStr::new("toml")))
            .collect();
        info!(
            "found {} letters: letter dir {:?}...",
            entries.len(),
            entries
        );

        // Letter's filename are prefixed with YYYY-MM-DD (see LoveLetter::letter_filename)
        // sort by newest to oldest.
        entries.sort();
        entries.reverse();

        let mut files: HashMap<PathBuf, String> = HashMap::new();
        for entry in entries {
            let letter = LoveLetter::load(entry)?;
            let file = self.rstdoc_path(&letter);
            if let Some(content) = files.get_mut(&file) {
                (*content).push_str(&letter.rstdoc_section());
            } else {
                files.insert(file, letter.rstdoc_heading() + &letter.rstdoc_section());
            }
        }

        for (file, content) in files.iter() {
            debug!("writing letters to {}...", file.display());
            fs::write(file, content)?;
            debug!("wrote");
            self.rstdoc_git_repo.add(file)?;
        }

        self.rstdoc_git_repo.commit("[loveletter] generate rstdoc", None)?;
        if !self.cfg.git_no_push.unwrap_or(true) {
            self.rstdoc_git_repo.push()?;
        }

        Ok(())
    }

    pub fn rstdoc_path(&self, letter: &LoveLetter) -> PathBuf {
        let mut p = self.rstdoc_dir.clone();
        p.push(letter.rstdoc_filename());
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
    use crate::cfg::Cfg;
    use crate::mail::RawMail;
    use tempfile::{tempdir, TempDir};

    #[test]
    fn test_archive_parse_subject() {
        assert_eq!(
            Archive::parse_subject("[edit] 1998/01/28: 妹妹生日快乐").unwrap(),
            (
                Date{ year: 1998, month: 1, day: Some(28) },
                Some("妹妹生日快乐".to_string()),
                Some("edit".to_string())
            )
        );
        assert_eq!(
            Archive::parse_subject("[edit] 1998/01/28:妹妹生日快乐").unwrap(),
            (
                Date{ year: 1998, month: 1, day: Some(28) },
                Some("妹妹生日快乐".to_string()),
                Some("edit".to_string())
            )
        );
        assert_eq!(
            Archive::parse_subject("[edit]1998/01/28:妹妹生日快乐").unwrap(),
            (
                Date{ year: 1998, month: 1, day: Some(28) },
                Some("妹妹生日快乐".to_string()),
                Some("edit".to_string())
            )
        );
        assert_eq!(
            Archive::parse_subject("[edit] 1998/01/28").unwrap(),
            (
                Date{ year: 1998, month: 1, day: Some(28) },
                None,
                Some("edit".to_string())
            )
        );
        assert_eq!(
            Archive::parse_subject("[edit]1998/01/28").unwrap(),
            (
                Date{ year: 1998, month: 1, day: Some(28) },
                None,
                Some("edit".to_string())
            )
        );
        assert_eq!(
            Archive::parse_subject("[edit] 1998/01/28:").unwrap(),
            (
                Date{ year: 1998, month: 1, day: Some(28) },
                None,
                Some("edit".to_string())
            )
        );
        assert_eq!(
            Archive::parse_subject("1998/01/28: 妹妹生日快乐").unwrap(),
            (
                Date{ year: 1998, month: 1, day: Some(28) },
                Some("妹妹生日快乐".to_string()),
                None
            )
        );
        assert_eq!(
            Archive::parse_subject("1998/01/28:妹妹生日快乐").unwrap(),
            (
                Date{ year: 1998, month: 1, day: Some(28) },
                Some("妹妹生日快乐".to_string()),
                None
            )
        );
        assert_eq!(
            Archive::parse_subject("1998/01/28:").unwrap(),
            (Date{ year: 1998, month: 1, day: Some(28) }, None, None)
        );
        assert_eq!(
            Archive::parse_subject("1998/01/28").unwrap(),
            (Date{ year: 1998, month: 1, day: Some(28) }, None, None)
        );
    }

    #[test]
    fn test_archive_upsert_letter() {
        use xshell::{cmd, Shell};

        fn tmpdir_path(d: &TempDir) -> String {
            let dir = d.path();
            let sh = Shell::new().unwrap();
            sh.change_dir(&dir);
            cmd!(sh, "git init").run().unwrap();
            dir.to_str().unwrap().to_owned()
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

        let letter = archive.upsert_letter(&parsed_mail).unwrap();
        assert!(archive.upsert_letter(&parsed_mail).is_err()); // test duplicate writing

        // Test TOML.
        assert_eq!(
            fs::read_to_string(archive.letter_path(&letter)).unwrap(),
            fs::read_to_string("./test_data/2025-04-03.toml").unwrap()
        );

        // Test read and write consistency.
        let letter2 = LoveLetter::load(archive.letter_path(&letter)).unwrap();
        assert_eq!(letter, letter2);

        archive.generate_rstdoc().unwrap();
        assert_eq!(
            fs::read_to_string(archive.rstdoc_index_path()).unwrap(),
            fs::read_to_string("./test_data/index.rst").unwrap()
        );
        assert_eq!(
            fs::read_to_string(archive.rstdoc_path(&letter)).unwrap(),
            fs::read_to_string("./test_data/2025.rst").unwrap()
        );
    }
}
