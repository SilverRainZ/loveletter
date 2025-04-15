use std::process::ExitCode;
use std::sync::{Arc, atomic::AtomicBool, atomic::Ordering};
use std::time::Duration;
use std::thread;

use anyhow::Result;
use log::{Level, info, warn, error};
use clap::Parser;
use signal_hook::{flag, consts::SIGTERM};

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

    /// Specify log level [avail: debug, info, warn, error]
    #[arg(long)] // TODO: ValueEnum
    log_level: Option<Level>,

    /// Re-generate rstdoc and exit
    #[arg(long, action)] // TODO: ValueEnum
    generate_rstdoc: bool,
}

fn _main() -> Result<()> {
    let args = &Args::parse();
    logger::init(args.log_level)?;
    info!("🐟 ← 💌 ← 📬 ← 💌 ← 🦢");

    let cfg = Cfg::load(&args.config)?;

    let archive = Archive::load(cfg.archive)?;
    if args.generate_rstdoc {
        archive.generate_rstdoc()?;
        return Ok(())
    }

    let mut mailbox = Mailbox::open(cfg.imap)?;

    let term = Arc::new(AtomicBool::new(false));
    flag::register(SIGTERM, Arc::clone(&term))?;
    let mut first_fetch = true;
    while !term.load(Ordering::Relaxed) {
        if first_fetch {
            first_fetch = false;
        } else {
            let interval = cfg.runtime.interval.unwrap_or(60);
            info!("sleep for {} seconds...", interval);
            thread::sleep(Duration::from_secs(interval));
        }

        let raw_mails = match mailbox.fetch_unseen() {
            Ok(m) => m,
            Err(e) => {
                warn!("failed to fetch unseen mails: {}", e);
                continue;
            },
        };

        let mut upserted = 0;
        for raw_mail in raw_mails.iter() {
            match raw_mail.parse() {
                Ok(parsed_mail) => match archive.upsert_letter(&parsed_mail) {
                    Ok(_) => upserted += 1,
                    Err(e) => error!("failed to upsert letter: {}", e),
                },
                Err(e) => error!("failed to parse raw mail: {}", e),
            };
        }
        if upserted == 0 {
            info!("no letter upserted, skip rst generation");
            continue;
        }

        match archive.generate_rstdoc() {
            Ok(_) => (),
            Err(e) => error!("failed to generate rstdoc: {}", e),
        }
    }

    // TODO: doesn't work
    info!("closing mailbox...");
    mailbox.close()?;
    info!("closed");

    Ok(())
}

fn main() -> ExitCode {
    exit(_main())
}
