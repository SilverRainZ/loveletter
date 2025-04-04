use std::fs;
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use log::{Level, info, debug};
use chrono::{DateTime, Utc};
use serde_derive::{Deserialize, Serialize};
use imap;
use mail_parser;
use toml;

use loveletter::utils::{logger, exit};

pub struct Recipient {
    cfg: Config,
    session: imap::Session<Box<dyn imap::ImapConnection>>,
}

impl Recipient {
    const INBOX: &str = "INBOX";

    fn login(cfg: Config) -> Result<Recipient> {
        let icfg = &cfg.imap;

        info!("connecting to {}:{}...", &icfg.host, icfg.port);
        let client = imap::ClientBuilder::new(&icfg.host, icfg.port).connect()?;
        info!("connected");

        // The client we have here is unauthenticated.
        // To do anything useful with the e-mails, we need to log in
        info!("login with username {}, password: {})...", icfg.username, "*".repeat(icfg.password.len()));
        let mut session = client
            .login(&icfg.username, &icfg.password)
            .map_err(|e| e.0)?;
        info!("logined");
        Ok(Recipient{
            cfg, session,
        })
    }

    fn fetch_unseen() -> Result<Recipient> {
        let mailbox = session.status(Self::INBOX, "(MESSAGES UNSEEN)")?;
        info!("found {} mails ({} unread) in mailbox {}", mailbox.exists, mailbox.unseen.unwrap(), Self::INBOX);

        for 
    }
}
    fn fetch_recent() -> Result<Recipient> {
        let mailbox = session.status(Self::INBOX, "(MESSAGES UNSEEN)")?;
        info!("found {} mails ({} unread) in mailbox {}", mailbox.exists, mailbox.unseen.unwrap(), Self::INBOX);
        for 
    }

fn fetch_inbox_top() -> imap::error::Result<Option<String>> {
    let inbox = "INBOX";

    // list message count.
    let mailbox = imap_session.status(inbox, "(MESSAGES UNSEEN)")?;
    info!("imap: found {} mails ({} unread) in {}", mailbox.exists, mailbox.unseen.unwrap(), inbox);

   // we want to fetch the first email in the INBOX mailbox
   imap_session.select(inbox)?;

   // fetch message number 1 in this mailbox, along with its RFC822 field.
   // RFC 822 dictates the format of the body of e-mails
   let messages = imap_session.fetch("1", "RFC822")?;
   let message = if let Some(m) = messages.iter().next() {
       m
   } else {
       return Ok(None);
   };

   // extract the message's body
   let body = message.body().expect("message did not have a body!");
   let body = std::str::from_utf8(body)
       .expect("message was not valid utf-8")
       .to_string();

    // be nice to the server and log out
    imap_session.logout()?;

    Ok(Some(body))
}

fn fetch_inbox_top_mock() -> imap::error::Result<Option<String>> {
    let contents = fs::read_to_string("./mail2.txt")
        .expect("Should have been able to read the file");
    Ok(Some(contents))
}

fn parse_mail(content: &str) -> mail_parser::Message<'_> {
    mail_parser::MessageParser::default().parse(content.as_bytes()).unwrap()
}


fn print_mail(mail: &mail_parser::Message) {
    fn recursive_print_mail(mail: &mail_parser::Message, indent: usize) {
        let prefix = "  ".repeat(indent);
        println!("{}{}", prefix, "=".repeat(80-2*indent));
        println!("{}MAIL (NESTED {})", prefix, indent);
        for (i, p) in mail.parts.iter().enumerate() {
            println!("{}{}", prefix, "-".repeat(80-2*indent));
            println!("{}PART {}:", prefix, i);
            println!("{}HEADER:", prefix);
            for h in p.headers.iter() {
                println!("{}KEY: {}, VALUE: {:?}", prefix, h.name.as_str(), h.value);
            }
            println!("{}{}", prefix, "~".repeat(80-2*indent));
            println!("{}BODY:", prefix);
            match &p.body {
                mail_parser::PartType::Text(x) | mail_parser::PartType::Html(x) => {
                    for line in  x.lines() {
                        println!("{}{}", prefix, line);
                    }
                },
                mail_parser::PartType::Message(m) => recursive_print_mail(&m, indent+1),
                _ => println!("{}{:?}", prefix, p.body),
            }
        }
    }

    recursive_print_mail(mail, 0);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoveLetter {
    // Sender information.
    sender_address: String,
    sender_is_meimei_if_true_and_gege_if_false: bool,

    // Content.
    date: DateTime<Utc>,
    title: String,
    content: String,

    // Meta data.
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermConfig {
    allowed_sender_addresses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    letter_dir: String,
    rst_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub imap: ImapConfig,
    pub perm: PermConfig,
    pub storage: StorageConfig,
}

fn load_config() -> Result<Config> {
    let cfg_data = fs::read_to_string("./config.toml")?;
    let cfg: Config = toml::from_str(&cfg_data)?;
    Ok(cfg)
}

fn main() -> ExitCode {
    logger::init(Some(Level::Debug)).unwrap();

    let raw = fetch_inbox_top().unwrap().unwrap();
    println!("{}", raw);
    // let raw = fetch_inbox_top_mock().unwrap().unwrap();
    // klet mail = parse_mail(&raw);
    // kprint_mail(&mail);
    //
    //

    ExitCode::SUCCESS
}
