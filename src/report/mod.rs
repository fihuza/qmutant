pub mod html;
pub mod json;
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
