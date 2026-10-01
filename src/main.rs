mod app;
mod cli;
mod config;
mod directive;
mod discover;
mod error;
mod executor;
mod instrument;
mod mutant;
mod mutator;
mod parse;
mod process;
#[cfg(test)]
mod properties;
mod report;
mod sandbox;
mod score;

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anstream::{eprintln, print};
use anyhow::Context;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::cli::{Cli, Command};
use crate::config::Config;
use crate::error::Error;

const FAILED: u8 = 1;
const UNUSABLE: u8 = 2;
const INTERRUPTED: u8 = 130;

fn main() -> ExitCode {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter(cli.verbose))
        .init();
    match execute(cli.command) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            let interrupted = matches!(error.downcast_ref(), Some(Error::Interrupted));
            ExitCode::from(if interrupted { INTERRUPTED } else { UNUSABLE })
        }
    }
}

fn filter(verbose: u8) -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(match verbose {
            0 => "warn",
            1 => "qmutant=info",
            _ => "qmutant=debug",
        })
    })
}

fn execute(command: Command) -> anyhow::Result<ExitCode> {
    match command {
        Command::Run(args) => {
            let config = Config::load(&args.selection.config, args.overrides())?;
            let plan = app::plan(&config)?;
            let cancel = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&cancel);
            ctrlc::set_handler(move || flag.store(true, Ordering::Relaxed))
                .context("cannot install the Ctrl-C handler")?;
            let summary = app::run(&config, &plan, args.dry_run, &cancel)?;
            print!("{}", summary.output);
            for directive in &summary.unused {
                eprintln!("error: {directive}");
            }
            Ok(if summary.passed() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(FAILED)
            })
        }
        Command::List(selection) => {
            let config = Config::load(&selection.config, selection.overrides())?;
            print!("{}", app::list(&config)?);
            Ok(ExitCode::SUCCESS)
        }
        Command::Init(args) => {
            let path = app::init(&std::env::current_dir()?, &args.command)?;
            print!("Wrote {}\n", path.display());
            Ok(ExitCode::SUCCESS)
        }
    }
}
