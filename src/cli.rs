use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use qmutant::config::{self, Jobs, Overrides, Reporter};

#[derive(Debug, Parser)]
#[command(version, about = "Mutation testing for QML")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
    #[arg(short, long, global = true, action = clap::ArgAction::Count, help = "Log more; repeat for debug output")]
    pub verbose: u8,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Run the tests against every mutant and report the score")]
    Run(RunArgs),
    #[command(about = "Print every mutant without running anything")]
    List(Selection),
    #[command(about = "Write a qmutant.toml listing this project's QML files")]
    Init(InitArgs),
}

#[derive(Debug, Args)]
pub struct Selection {
    #[arg(long, default_value = config::DEFAULT_FILE, help = "Configuration file")]
    pub config: PathBuf,
    #[arg(
        long = "mutate",
        value_name = "GLOB",
        help = "Files to mutate, replacing `mutate`"
    )]
    pub mutate: Vec<String>,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    #[command(flatten)]
    pub selection: Selection,
    #[arg(
        short,
        long,
        value_name = "N|N%",
        help = "Worker count, or a share of the CPUs"
    )]
    pub jobs: Option<Jobs>,
    #[arg(
        long,
        value_name = "MS",
        help = "Milliseconds added to every mutant's deadline"
    )]
    pub timeout: Option<u64>,
    #[arg(
        long,
        value_name = "FACTOR",
        help = "Multiple of the unmutated run's time allowed per mutant"
    )]
    pub timeout_factor: Option<f64>,
    #[arg(
        long = "reporter",
        value_name = "NAME",
        help = "terminal, progress, json or html; replaces `reporters`"
    )]
    pub reporters: Vec<Reporter>,
    #[arg(long, help = "Only check that the tests pass unmutated")]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct InitArgs {
    #[arg(long, help = "The command that runs the project's tests")]
    pub command: String,
}

impl Selection {
    pub fn overrides(&self) -> Overrides {
        Overrides {
            mutate: (!self.mutate.is_empty()).then(|| self.mutate.clone()),
            ..Overrides::default()
        }
    }
}

impl RunArgs {
    pub fn overrides(&self) -> Overrides {
        Overrides {
            jobs: self.jobs,
            timeout_ms: self.timeout,
            timeout_factor: self.timeout_factor,
            reporters: (!self.reporters.is_empty()).then(|| self.reporters.clone()),
            ..self.selection.overrides()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_overrides(args: &[&str]) -> Overrides {
        let cli = Cli::try_parse_from(["qmutant", "run"].iter().chain(args)).unwrap();
        match cli.command {
            Command::Run(run) => run.overrides(),
            command => panic!("parsed {command:?}"),
        }
    }

    #[test]
    fn every_run_flag_becomes_an_override() {
        let overrides = run_overrides(&[
            "--mutate",
            "A.qml",
            "-j",
            "25%",
            "--timeout",
            "300",
            "--timeout-factor",
            "2.5",
            "--reporter",
            "json",
        ]);
        assert_eq!(overrides.mutate, Some(vec!["A.qml".to_owned()]));
        assert_eq!(overrides.jobs, Some(Jobs::Percent(25)));
        assert_eq!(overrides.timeout_ms, Some(300));
        assert_eq!(overrides.timeout_factor, Some(2.5));
        assert_eq!(overrides.reporters, Some(vec![Reporter::Json]));
    }

    #[test]
    fn absent_flags_leave_the_file_alone() {
        let overrides = run_overrides(&[]);
        assert_eq!(overrides.mutate, None);
        assert_eq!(overrides.jobs, None);
        assert_eq!(overrides.timeout_ms, None);
        assert_eq!(overrides.timeout_factor, None);
        assert_eq!(overrides.reporters, None);
    }

    #[test]
    fn a_bad_flag_value_is_refused_by_the_parser() {
        let error = Cli::try_parse_from(["qmutant", "run", "--reporter", "xml"]).unwrap_err();
        assert!(error.to_string().contains("unknown reporter `xml`"));
    }
}
