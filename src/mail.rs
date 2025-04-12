use std::collections::HashSet;
use std::borrow::Cow;
use std::fmt;
use std::iter::IntoIterator;

use anyhow::{Context, Result};
use log::{debug, info, error};
use chrono::{DateTime, Utc};
use imap;
use mail_parser::{MessageParser, Addr, Message, PartType};
use email_address::EmailAddress;

use crate::cfg::ImapCfg;

pub struct Mailbox {
    session: imap::Session<Box<dyn imap::ImapConnection>>,
}

impl Mailbox {
    const INBOX: &str = "INBOX";

    pub fn open(cfg: ImapCfg) -> Result<Mailbox> {
        info!("connecting to {}:{}...", &cfg.host, cfg.port);
        let client = imap::ClientBuilder::new(&cfg.host, cfg.port).connect()?;
        info!("connected");

        // The client we have here is unauthenticated.
        // To do anything useful with the e-mails, we need to log in
        info!("login with username {}, password: {})...", cfg.username, "*".repeat(cfg.password.len()));
        let session = client
            .login(&cfg.username, &cfg.password)
            .map_err(|e| e.0)?;
        info!("logined");

        Ok(Mailbox{session})
    }

    // fn fetch_unseen() -> Result<Recipient> {
    //     let mailbox = session.status(Self::INBOX, "(MESSAGES UNSEEN)")?;
    //     info!("found {} mails ({} unread) in mailbox {}", mailbox.exists, mailbox.unseen.unwrap(), Self::INBOX);

    //     for 
    // }

    fn search(&mut self, query: &str) -> Result<HashSet<u32>> {
        info!("selecting mailbox {}...", Self::INBOX);
        let mailbox = self.session.select(Self::INBOX)?;
        info!("selected, found {} mails ({} recent, {} unread) in mailbox {} (readonly: {})",
        mailbox.exists, mailbox.recent, mailbox.unseen.unwrap_or(0), Self::INBOX, mailbox.is_read_only);

        debug!("searching mails that match searching criteria {}", query);
        let seqs = self.session.search(query)?;
        debug!("found {} mails that match searching criteria: {:?}", seqs.len(), seqs);
        Ok(seqs)
    }

    // TODO: fetch size
    pub fn fetch(&mut self, query: &str) -> Result<Vec<RawMail>> {
        let seqs = self.search(query)?.
            into_iter().
            map(|i| i.to_string()).
            collect::<Vec<_>>().
            join(",");

        // Fetch message numbers in this mailbox, along with its RFC822 field.
        // RFC 822 dictates the format of the body of e-mails.
        debug!("fetching sequence_set {}...", seqs);
        let msgs = self.session.fetch(seqs, "RFC822")?;
        debug!("fetched {} mails", msgs.len());

        let mut mails: Vec<RawMail> = Vec::new();
        // Extract the message's body.
        for msg in msgs.iter() {
            match msg.body() {
                None => {
                    error!("failed to extract mail body from message: {:?}, skipped", msg);
                    continue;
                },
                Some(body) => match std::str::from_utf8(body) {
                    Err(e) => {
                        error!("mail body was not valid utf-8: {}, skipped", e);
                        continue;
                    },
                    Ok(body) => mails.push(RawMail{data: body.to_owned()}),
                },
            }
        }

        Ok(mails)
    }

    pub fn fetch_seen(&mut self) -> Result<Vec<RawMail>> {
        self.fetch("SEEN")
    }

    pub fn fetch_unseen(&mut self) -> Result<Vec<RawMail>> {
        self.fetch("UNSEEN")
    }

    pub fn close(mut self) -> Result<()> {
        self.session.logout()?;
        Ok(())
    }
}

pub struct RawMail {
    data: String,
}

impl RawMail {
    pub fn new(data: &str) -> RawMail {
        RawMail { data: data.to_owned() }
    }

    pub fn parse(&self) -> Result<ParsedMail<'_>> {
        info!("parsing raw mail...");
        let msg = MessageParser::default().
            parse(self.data.as_bytes()).
            context("parse failed")?;
        info!("parsed mail: {}", msg.subject().unwrap_or("untitled"));
        Ok(ParsedMail{ msg })
    }

}
pub struct ParsedMail<'a> {
    msg: Message<'a>,
}

impl ParsedMail<'_> {
    /// NOTE: Only support single address for now.
    fn addr_to_addr(addr: Option<&Addr>) -> Option<EmailAddress> {
        addr.and_then(|x| {
                match (x.name(), x.address()) {
                    (Some(n), Some(a)) => format!("{} <{}>", n, a).parse::<EmailAddress>().ok(),
                    (_, Some(a)) => a.parse::<EmailAddress>().ok(),
                    (_, _) => None,
                }
            })
    }
    pub fn from(&self) -> Option<EmailAddress> {
        Self::addr_to_addr(self.msg.from().and_then(|x| x.first()))
    }

    /// NOTE: Only support single address for now.
    pub fn to(&self) -> Option<EmailAddress> {
        Self::addr_to_addr(self.msg.to().and_then(|x| x.first()))
    }

    pub fn subject(&self) -> Option<&str> {
        self.msg.subject()
    }

    pub fn date(&self) -> Option<DateTime<Utc>> {
        self.msg.date().
            and_then(|x| DateTime::from_timestamp(x.to_timestamp(), 0))
    }

    pub fn text_body(&self) -> Option<String> {
        let mut body:Vec<Cow<'_, str>> = Vec::new();
        for i in self.msg.text_body.iter() {
            if let Some(x) = self.msg.body_text(*i) {
                body.push(x)
            };
        }
        match body.is_empty() {
            true => None,
            false => Some(body.join("\n")),
        }
    }

    pub fn html_body(&self) -> Option<String> {
        let mut body:Vec<Cow<'_, str>> = Vec::new();
        for i in self.msg.html_body.iter() {
            if let Some(x) = self.msg.body_html(*i) {
                body.push(x)
            };
        }
        match body.is_empty() {
            true => None,
            false => Some(body.join("\n")),
        }
    }
}

impl fmt::Display for ParsedMail<'_> {
   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { 
        fn recursive_fmt(f: &mut fmt::Formatter<'_>, mail: &Message, indent: usize) -> fmt::Result {
            macro_rules! fmt_indent {
                ($($arg:tt)*) => {
                    write!(f, "{}{}\n", "  ".repeat(indent), format!($($arg)*))?
                };
            }

            fmt_indent!("{}", "=".repeat(80-2*indent));
            fmt_indent!("MAIL (NESTED {})", indent);
            for (i, p) in mail.parts.iter().enumerate() {
                fmt_indent!("{}", "-".repeat(80-2*indent));
                fmt_indent!("PART {}:", i);
                fmt_indent!("HEADER:");
                for h in p.headers.iter() {
                    fmt_indent!("KEY: {}, VALUE: {:?}", h.name.as_str(), h.value);
                }
                fmt_indent!("{}", "~".repeat(80-2*indent));
                fmt_indent!("BODY:");
                match &p.body {
                    PartType::Text(x) | PartType::Html(x) => {
                        for line in  x.lines() {
                            fmt_indent!("{}", line);
                        }
                    },
                    PartType::Message(m) => recursive_fmt(f, &m, indent+1)?,
                    _ => fmt_indent!("{:?}", p.body),
                }
            }
            Ok(())
        }
        recursive_fmt(f, &self.msg, 0)
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use crate::cfg::Cfg;

    #[test]
    fn test_raw_mail_parse() {
        let data = fs::read_to_string("./test_data/mail.txt").unwrap();
        let raw_mail = RawMail{data};
        let parsed_mail = raw_mail.parse().unwrap();
        assert_eq!(parsed_mail.from(), Some(EmailAddress::new_unchecked("Shengyu Zhang <gege@example.com>")));
        assert_eq!(parsed_mail.to(), Some(EmailAddress::new_unchecked("Love Letter <loveletter@example.com>")));
        assert_eq!(parsed_mail.subject(), Some("2025/04/03: 测试数据"));
        assert_eq!(parsed_mail.text_body(), Some("张同学 我们这个 I 人交朋友的项目还有效咩\u{a0}--\u{a0}Best regards,Shengyu Zhang\u{a0}https://example.com\u{a0}".to_string()));
    }

    #[ignore]
    #[test]
    fn test_mailbox() {
        let cfg = Cfg::load("./test_data/config.toml").unwrap().imap;
        let mut mailbox = Mailbox::open(cfg).unwrap();
        let mails = mailbox.fetch_seen().unwrap();
        assert!(!mails.is_empty());
        mailbox.close().unwrap();
    }
}
