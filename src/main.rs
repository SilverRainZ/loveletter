use std::process::ExitCode;
use std::thread;

use anyhow::Result;
use log::{Level, debug, info, warn, error};
use clap::Parser;
use signal_hook::{consts::SIGINT, iterator::Signals};

use loveletter::utils::{logger, exit};
use loveletter::cfg::Cfg;
use loveletter::mail::Mailbox;
use loveletter::letter::Archive;

/// 🐟 ← 💌 ← 📬 ← 💌 ← 🦢
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)] // Read from `Cargo.toml`
struct Args {
    /// Specify the location of the configuration file
    #[arg(short, long, default_value = "./config.toml")] 
    config: String, 

    #[arg(long)] // TODO: ValueEnum
    log_level: Option<Level>,
}

fn _main() -> Result<()> {
    let args = &Args::parse();
    logger::init(args.log_level)?;
    info!("🐟 ← 💌 ← 📬 ← 💌 ← 🦢");

    let cfg = Cfg::load(&args.config)?;
    let archive = Archive::load(cfg.archive)?;
    let mut mailbox = Mailbox::open(cfg.imap)?;

    loop {
        let raw_mails = mailbox.fetch_unseen()?;
        for raw_mail in raw_mails.iter() {
            match raw_mail.parse() {
                Ok(parsed_mail) => archive.upsert_letter(&parsed_mail)?,
                Err(e) => {
                    error!("failed to parse raw mail: {}", e);
                    continue;
                },
            };
        }
    }

    Ok(())
}

fn main() -> ExitCode {
    exit(_main())
}
