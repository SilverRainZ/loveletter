use std::fs;

use imap;
use mail_parser;
use chrono::{DateTime, Utc};
use serde_derive::{Deserialize, Serialize};


fn fetch_inbox_top() -> imap::error::Result<Option<String>> {

    let client = imap::ClientBuilder::new("imap.yandex.com", 993).connect()?;

    // the client we have here is unauthenticated.
    // to do anything useful with the e-mails, we need to log in
    let mut imap_session = client
        .login("i@example.com", "password")
        .map_err(|e| e.0)?;

    // we want to fetch the first email in the INBOX mailbox
    imap_session.select("INBOX")?;

    // fetch message number 1 in this mailbox, along with its RFC822 field.
    // RFC 822 dictates the format of the body of e-mails
    let messages = imap_session.fetch("2", "RFC822")?;
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
    let contents = fs::read_to_string("./mail.txt")
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
pub struct Config {
    // IMAP login config.
    imap_host: String,
    imap_port: String,
    imap_username: String,
    imap_password: String,
    imap_check_interval: String, // interval for checking new mail

    allowed_sender: Vec<String>,
    data_dir: String,
    rst_dir: String,
}
fn main() {
    let raw = fetch_inbox_top_mock().unwrap().unwrap();
    let mail = parse_mail(&raw);
    print_mail(&mail);
}
