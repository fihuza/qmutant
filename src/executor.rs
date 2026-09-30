use std::fs;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::error::{self, Error};
use crate::mutant::{Mutant, SourceFile, Status, Verdict};
use crate::process::{self, Job, Outcome};
use crate::sandbox::Sandboxes;

pub struct Execution<'a> {
    pub command: &'a str,
    pub files: &'a [SourceFile],
    pub sandboxes: &'a Sandboxes,
    pub cancel: &'a AtomicBool,
}

impl Execution<'_> {
    pub fn dry_run(&self) -> Result<Duration, Error> {
        let log = self.sandboxes.log(0);
        let run = process::run(
            &Job {
                command: self.command,
                directory: self.sandboxes.worker(0),
                env: &[("QMUTANT_WORKER", "0".to_owned())],
                log: &log,
                deadline: None,
            },
            self.cancel,
        )
        .map_err(error::at(self.sandboxes.worker(0)))?;
        match run.outcome {
            Outcome::Passed => Ok(run.elapsed),
            Outcome::Cancelled => Err(Error::Interrupted),
            Outcome::Failed | Outcome::TimedOut => Err(Error::DryRunFailed {
                output: process::output_tail(&log),
            }),
        }
    }

    pub fn mutate(
        &self,
        mutants: &[&Mutant],
        workers: usize,
        deadline: Duration,
        mut on_verdict: impl FnMut(&Verdict),
    ) -> Result<Vec<Verdict>, Error> {
        let cursor = AtomicUsize::new(0);
        let (sender, receiver) = mpsc::channel();
        let mut verdicts = Vec::with_capacity(mutants.len());
        let failure = thread::scope(|scope| {
            for worker in 0..workers {
                let sender = sender.clone();
                let cursor = &cursor;
                scope.spawn(move || {
                    while let Some(mutant) = mutants.get(cursor.fetch_add(1, Ordering::Relaxed)) {
                        if self.cancel.load(Ordering::Relaxed) {
                            break;
                        }
                        sender
                            .send(self.judge(worker, mutant, deadline))
                            .expect("the receiver outlives every worker");
                    }
                });
            }
            drop(sender);
            let mut failure = None;
            for verdict in receiver {
                match verdict {
                    Ok(Some(verdict)) => {
                        tracing::debug!(mutant = verdict.mutant, status = ?verdict.status, "judged");
                        on_verdict(&verdict);
                        verdicts.push(verdict);
                    }
                    Ok(None) => {}
                    Err(error) => {
                        self.cancel.store(true, Ordering::Relaxed);
                        failure.get_or_insert(error);
                    }
                }
            }
            failure
        });
        if let Some(error) = failure {
            return Err(error);
        }
        if self.cancel.load(Ordering::Relaxed) {
            return Err(Error::Interrupted);
        }
        verdicts.sort_by_key(|verdict| verdict.mutant);
        Ok(verdicts)
    }

    fn judge(
        &self,
        worker: usize,
        mutant: &Mutant,
        deadline: Duration,
    ) -> Result<Option<Verdict>, Error> {
        let file = &self.files[mutant.file];
        let target = self.sandboxes.worker(worker).join(&file.path);
        fs::write(&target, mutant.apply(&file.source)).map_err(error::at(&target))?;
        let log = self.sandboxes.log(worker);
        let env = [
            ("QMUTANT_WORKER", worker.to_string()),
            ("QMUTANT_MUTANT", mutant.id.to_string()),
        ];
        let run = process::run(
            &Job {
                command: self.command,
                directory: self.sandboxes.worker(worker),
                env: &env,
                log: &log,
                deadline: Some(deadline),
            },
            self.cancel,
        );
        fs::write(&target, &file.source).map_err(error::at(&target))?;
        let (status, reason) = match run {
            Ok(run) => match run.outcome {
                Outcome::Passed => (Status::Survived, None),
                Outcome::Failed => (
                    Status::Killed,
                    Some(process::output_tail(&log)).filter(|output| !output.is_empty()),
                ),
                Outcome::TimedOut => (Status::Timeout, None),
                Outcome::Cancelled => return Ok(None),
            },
            Err(error) => (Status::Error, Some(error.to_string())),
        };
        Ok(Some(Verdict {
            mutant: mutant.id,
            status,
            reason,
        }))
    }
}
