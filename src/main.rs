use std::fs;

use imap;
use mailparse;

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

fn parse_mail(content: &str) -> mailparse::ParsedMail<'_> {
    mailparse::parse_mail(content.as_bytes()).unwrap()
}


fn print_mail(mail: &mailparse::ParsedMail) {
    fn recursive_print_mail(mail: &mailparse::ParsedMail, indent: usize) {
        let prefix = "  ".repeat(indent);
        println!("{}{}", prefix, "=".repeat(80-2*indent));
        println!("{}MAIL (NESTED {})", prefix, indent);
        println!("{}HEADER:", prefix);
        for h in mail.headers.iter() {
            println!("{}KEY: {}, VALUE: {}", prefix, h.get_key(), h.get_value());
        }

        println!("{}{}", prefix, "-".repeat(80-2*indent)); // delim
        println!("{}BODY:", prefix);
        for line in  mail.get_body().unwrap().lines() {
            println!("{}{}", prefix, line);
        }

        for subpart in mail.subparts.iter() {
            recursive_print_mail(&subpart, indent+1);
        }
    }
    recursive_print_mail(mail, 0);
}

fn main() {
    println!("Hello, world!");
    let raw = fetch_inbox_top_mock().unwrap().unwrap();
    let mail = parse_mail(&raw);
    print_mail(&mail);
}
