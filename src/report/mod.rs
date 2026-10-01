pub mod document;
pub mod html;
pub mod progress;
pub mod terminal;

use crate::config::Thresholds;
use crate::instrument::Plan;
use crate::mutant::{Status, Verdict};
use crate::score::Counts;

pub struct Report<'a> {
    pub plan: &'a Plan,
    pub statuses: Vec<Status>,
    pub reasons: Vec<Option<String>>,
    pub thresholds: Thresholds,
}

impl<'a> Report<'a> {
    #[must_use]
    pub fn new(plan: &'a Plan, verdicts: Vec<Verdict>, thresholds: Thresholds) -> Self {
        let mut statuses: Vec<Option<Status>> = plan.decided.clone();
        let mut reasons: Vec<Option<String>> = vec![None; plan.mutants.len()];
        for verdict in verdicts {
            statuses[verdict.mutant] = Some(verdict.status);
            reasons[verdict.mutant] = verdict.reason;
        }
        Self {
            plan,
            statuses: statuses
                .into_iter()
                .map(|status| status.expect("every pending mutant receives a verdict"))
                .collect(),
            reasons,
            thresholds,
        }
    }

    #[must_use]
    pub fn counts(&self) -> Counts {
        Counts::tally(self.statuses.iter().copied())
    }

    #[must_use]
    pub fn counts_for(&self, file: usize) -> Counts {
        Counts::tally(
            self.plan
                .mutants
                .iter()
                .filter(|mutant| mutant.file == file)
                .map(|mutant| self.statuses[mutant.id]),
        )
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use std::path::PathBuf;

    use crate::instrument::Plan;
    use crate::mutant::{Lines, Mutant, SourceFile};
    use crate::mutator::{Mutator, mutations};
    use crate::parse;

    pub(crate) fn plan(source: &str, only: Mutator) -> Plan {
        let excluded: Vec<Mutator> = Mutator::ALL.into_iter().filter(|m| *m != only).collect();
        let lines = Lines::new(source);
        let mutants = mutations(&parse::parse(source).unwrap(), source, &excluded)
            .into_iter()
            .enumerate()
            .map(|(id, mutation)| Mutant::new(id, 0, mutation, &lines))
            .collect::<Vec<_>>();
        Plan {
            decided: vec![None; mutants.len()],
            files: vec![SourceFile {
                path: PathBuf::from("A.qml"),
                source: source.to_owned(),
            }],
            mutants,
            unused: Vec::new(),
        }
    }
}
