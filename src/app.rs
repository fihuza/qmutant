use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use serde::Serialize;

use crate::config::{self, Config, Reporter};
use crate::error::{self, Error};
use crate::{discover, report};

use crate::executor::Execution;
use crate::instrument::{Plan, instrument};
use crate::mutant::Mutant;
use crate::report::Report;
use crate::report::progress::Progress;
use crate::sandbox::Sandboxes;

const REPORTS: &str = "reports";

#[derive(Debug)]
pub struct Summary {
    pub output: String,
    pub problems: Vec<String>,
    pub meets_break: bool,
}

impl Summary {
    #[must_use]
    pub fn passed(&self) -> bool {
        self.meets_break && self.problems.is_empty()
    }
}

pub fn plan(config: &Config) -> Result<Plan, Error> {
    instrument(config)
}

pub fn run(
    config: &Config,
    plan: &Plan,
    dry_run: bool,
    cancel: &AtomicBool,
) -> Result<Summary, Error> {
    let pending: Vec<&Mutant> = plan.pending().collect();
    let cpus = std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    let workers = if dry_run {
        1
    } else {
        config.jobs.resolve(cpus).get().min(pending.len()).max(1)
    };
    let sandboxes = Sandboxes::create(
        &config.root,
        &config.sandbox_dir,
        &config.ignore,
        workers,
        cancel,
    )?;
    let execution = Execution {
        command: &config.command,
        files: &plan.files,
        sandboxes: &sandboxes,
        cancel,
    };
    let baseline = execution.dry_run()?;
    let deadline = config.timeout.for_baseline(baseline);
    tracing::info!(?baseline, ?deadline, workers, "the unmutated tests pass");
    let unused = unused(plan);
    if dry_run {
        return Ok(Summary {
            output: format!(
                "The tests pass unmutated in {baseline:.2?}. {} mutants would run, each stopped after {deadline:.2?}.\n",
                pending.len()
            ),
            problems: unused,
            meets_break: true,
        });
    }
    let mut progress = Progress::new(
        pending.len(),
        config.reporters.contains(&Reporter::Progress),
    );
    let verdicts = execution.mutate(&pending, workers, deadline, |verdict| {
        progress.record(verdict);
    });
    progress.finish();
    let report = Report::new(plan, verdicts?, config.thresholds);
    Ok(summarize(write_reports(config, &report)?, unused, &report))
}

fn summarize(output: String, mut problems: Vec<String>, report: &Report) -> Summary {
    let counts = report.counts();
    if counts.error > 0 {
        problems.push(format!(
            "{} mutants could not be run; the report gives the reason for each",
            counts.error
        ));
    }
    Summary {
        output,
        problems,
        meets_break: counts.meets(report.thresholds),
    }
}

pub fn list(config: &Config) -> Result<String, Error> {
    Ok(report::terminal::listing(&plan(config)?))
}

pub fn init(root: &Path, command: &str) -> Result<PathBuf, Error> {
    #[derive(Serialize)]
    struct Initial<'a> {
        mutate: Vec<String>,
        command: &'a str,
    }
    let path = root.join(config::DEFAULT_FILE);
    if path.exists() {
        return Err(Error::Config {
            path,
            message: "already exists; edit it instead".to_owned(),
        });
    }
    let found = discover::discover(root, &["**/*.qml".to_owned()], Path::new(".qmutant"), &[])?;
    let mutate: Vec<String> = found
        .iter()
        .filter(|file| !holds_tests(file))
        .map(|file| globset::escape(&file.to_string_lossy()))
        .collect();
    if mutate.is_empty() {
        return Err(Error::Config {
            path,
            message: "found only test files; name the QML to mutate yourself".to_owned(),
        });
    }
    let text = toml::to_string(&Initial { mutate, command })
        .expect("a list of paths and a command always serialise");
    fs::write(&path, text).map_err(error::at(&path))?;
    Ok(path)
}

fn holds_tests(file: &Path) -> bool {
    let named_as_a_test = file
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("tst_"));
    let kept_with_the_tests = file
        .components()
        .any(|part| part.as_os_str() == "tests" || part.as_os_str() == "test");
    named_as_a_test || kept_with_the_tests
}

fn unused(plan: &Plan) -> Vec<String> {
    plan.unused
        .iter()
        .map(|(path, directive)| {
            format!(
                "{}:{}: `{}` disables nothing; remove it",
                path.display(),
                directive.line,
                directive.text
            )
        })
        .collect()
}

fn write_reports(config: &Config, report: &Report) -> Result<String, Error> {
    let mut output = String::new();
    if config.reporters.contains(&Reporter::Terminal) {
        output += &report::terminal::render(report);
    }
    let wants = |reporter| config.reporters.contains(&reporter);
    if wants(Reporter::Json) || wants(Reporter::Toml) || wants(Reporter::Html) {
        let directory = config.root.join(REPORTS);
        fs::create_dir_all(&directory).map_err(error::at(&directory))?;
        let json = report::document::json(report);
        if wants(Reporter::Json) {
            output += &save(&directory.join("mutation.json"), &json)?;
        }
        if wants(Reporter::Toml) {
            output += &save(
                &directory.join("mutation.toml"),
                &report::document::toml(report),
            )?;
        }
        if wants(Reporter::Html) {
            output += &save(
                &directory.join("mutation.html"),
                &report::html::render(&json),
            )?;
        }
    }
    Ok(output)
}

fn save(path: &Path, content: &str) -> Result<String, Error> {
    fs::write(path, content).map_err(error::at(path))?;
    Ok(format!("Report written to {}\n", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Thresholds;
    use crate::mutant::{Status, Verdict};
    use crate::mutator::Mutator;
    use crate::report::fixtures::plan;

    fn summary_of(statuses: &[Status]) -> Summary {
        let plan = plan(
            "Item { x: a && b; y: c && d; z: e && f }",
            Mutator::LogicalOperator,
        );
        let verdicts = statuses
            .iter()
            .enumerate()
            .map(|(mutant, &status)| Verdict {
                mutant,
                status,
                reason: None,
            })
            .collect();
        let report = Report::new(&plan, verdicts, Thresholds::default());
        summarize(String::new(), Vec::new(), &report)
    }

    #[test]
    fn a_complete_run_passes() {
        let summary = summary_of(&[Status::Killed, Status::Killed, Status::Survived]);
        assert!(summary.passed(), "{:?}", summary.problems);
    }

    #[test]
    fn a_run_where_a_mutant_could_not_be_run_fails() {
        let summary = summary_of(&[Status::Killed, Status::Error, Status::Error]);
        assert!(!summary.passed());
        assert_eq!(
            summary.problems,
            ["2 mutants could not be run; the report gives the reason for each"]
        );
    }
}
