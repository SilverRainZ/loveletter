const LOVE_LETTER_LOG_LEVEL: &str = "LOVE_LETTER_LOG_LEVEL";

/// Provides common logic for cang's various command line components.
pub mod logger {
    use anyhow::Result;
    use log::Level;
    use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode};
    use std::env;

    static mut LEVEL: Level = Level::Info;

    // Priv: args > env.
    pub fn init(level: Option<Level>) -> Result<()> {
        let level = match level {
            Some(lv) => lv,
            None => match env::var(&super::LOVE_LETTER_LOG_LEVEL) {
                // TODO: const?
                Ok(lv) => lv.parse()?,
                Err(_) => Level::Info,
            },
        };
        CombinedLogger::init(
            vec![TermLogger::new(
                level.to_level_filter(),
                Config::default(),
                TerminalMode::Mixed,
                ColorChoice::Auto,
            )],
            // TODO: more logger here.
        )?;

        unsafe {
            LEVEL = level;
        }

        Ok(())
    }

    pub fn level() -> Level {
        unsafe { LEVEL }
    }
}
use core::fmt;
use log::error;
use std::process::ExitCode;
use std::result::Result;

pub fn exit<T, E: fmt::Display+fmt::Debug>(r: Result<T, E>) -> ExitCode {
    use log::Level;

    match logger::level() > Level::Debug {
        true => match r {
            Ok(_) => ExitCode::SUCCESS,
            Err(e) => {
                error!("{:#}", e);
                ExitCode::FAILURE
            }
        },
        false => {
            r.unwrap();
            ExitCode::SUCCESS
        },
    }
}
