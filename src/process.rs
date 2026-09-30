use std::fs::File;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use process_wrap::std::{ChildWrapper, CommandWrap, ProcessGroup};

const POLL: Duration = Duration::from_millis(10);
const OUTPUT_TAIL: usize = 4000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Passed,
    Failed,
    TimedOut,
    Cancelled,
}

#[derive(Debug)]
pub struct Run {
    pub outcome: Outcome,
    pub elapsed: Duration,
}

pub struct Job<'a> {
    pub command: &'a str,
    pub directory: &'a Path,
    pub env: &'a [(&'a str, String)],
    pub log: &'a Path,
    pub deadline: Option<Duration>,
}

pub fn run(job: &Job, cancel: &AtomicBool) -> io::Result<Run> {
    let log = File::create(job.log)?;
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(job.command)
        .current_dir(job.directory)
        .envs(job.env.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    let mut child = CommandWrap::from(command)
        .wrap(ProcessGroup::leader())
        .spawn()?;
    let started = Instant::now();
    let outcome = loop {
        if let Some(status) = child.try_wait()? {
            break if status.success() {
                Outcome::Passed
            } else {
                Outcome::Failed
            };
        }
        if cancel.load(Ordering::Relaxed) {
            break Outcome::Cancelled;
        }
        if job
            .deadline
            .is_some_and(|deadline| started.elapsed() >= deadline)
        {
            break Outcome::TimedOut;
        }
        thread::sleep(POLL);
    };
    let elapsed = started.elapsed();
    if matches!(outcome, Outcome::TimedOut | Outcome::Cancelled) {
        child.kill()?;
    } else {
        sweep_stragglers(child.as_mut());
    }
    Ok(Run { outcome, elapsed })
}

fn sweep_stragglers(child: &mut dyn ChildWrapper) {
    if child.start_kill().is_err() {
        tracing::debug!("the process group had already emptied");
    }
}

pub fn output_tail(log: &Path) -> String {
    let output = std::fs::read(log).unwrap_or_default();
    let output = String::from_utf8_lossy(&output);
    let output = output.trim_end();
    let start = output
        .char_indices()
        .rev()
        .nth(OUTPUT_TAIL - 1)
        .map_or(0, |(index, _)| index);
    output[start..].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_in(directory: &Path, command: &str, deadline: Option<Duration>) -> (Run, String) {
        let log = directory.join("log");
        let env = [("QMUTANT_MUTANT", "7".to_owned())];
        let run = run(
            &Job {
                command,
                directory,
                env: &env,
                log: &log,
                deadline,
            },
            &AtomicBool::new(false),
        )
        .unwrap();
        (run, output_tail(&log))
    }

    #[test]
    fn exit_status_decides_the_outcome() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(
            run_in(directory.path(), "true", None).0.outcome,
            Outcome::Passed
        );
        assert_eq!(
            run_in(directory.path(), "exit 3", None).0.outcome,
            Outcome::Failed
        );
    }

    #[test]
    fn output_environment_and_directory_reach_the_log() {
        let directory = tempfile::tempdir().unwrap();
        let (_, output) = run_in(
            directory.path(),
            "echo out; echo err >&2; echo $QMUTANT_MUTANT; basename \"$PWD\"",
            None,
        );
        let name = directory.path().file_name().unwrap().to_string_lossy();
        assert_eq!(output, format!("out\nerr\n7\n{name}"));
    }

    #[test]
    fn a_command_past_its_deadline_is_killed_with_its_whole_group() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("survivor");
        let command = format!("(sleep 1; touch {}) & sleep 30", marker.display());
        let (run, _) = run_in(directory.path(), &command, Some(Duration::from_millis(200)));
        assert_eq!(run.outcome, Outcome::TimedOut);
        assert!(run.elapsed < Duration::from_secs(5));
        thread::sleep(Duration::from_millis(1500));
        assert!(!marker.exists(), "a grandchild outlived the timeout");
    }

    #[test]
    fn background_processes_left_by_a_finished_command_are_killed() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("straggler");
        let command = format!("(sleep 1; touch {}) & exit 0", marker.display());
        assert_eq!(
            run_in(directory.path(), &command, None).0.outcome,
            Outcome::Passed
        );
        thread::sleep(Duration::from_millis(1500));
        assert!(
            !marker.exists(),
            "a background process outlived its command"
        );
    }

    #[test]
    fn cancelling_stops_a_running_command() {
        let directory = tempfile::tempdir().unwrap();
        let log = directory.path().join("log");
        let run = run(
            &Job {
                command: "sleep 30",
                directory: directory.path(),
                env: &[],
                log: &log,
                deadline: None,
            },
            &AtomicBool::new(true),
        )
        .unwrap();
        assert_eq!(run.outcome, Outcome::Cancelled);
    }

    #[test]
    fn only_the_tail_of_long_output_is_kept() {
        let directory = tempfile::tempdir().unwrap();
        let log = directory.path().join("log");
        std::fs::write(&log, format!("{}é\n", "x".repeat(10_000))).unwrap();
        let tail = output_tail(&log);
        assert_eq!(tail.chars().count(), OUTPUT_TAIL);
        assert!(tail.ends_with("xé"));
        assert_eq!(output_tail(&directory.path().join("missing")), "");
    }
}
