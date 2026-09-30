use indicatif::{ProgressBar, ProgressStyle};

use crate::mutant::{Status, Verdict};
use crate::score::Counts;

pub struct Progress {
    bar: ProgressBar,
    counts: Counts,
}

impl Progress {
    #[must_use]
    pub fn new(total: usize, visible: bool) -> Self {
        let bar = if visible {
            ProgressBar::new(total as u64)
        } else {
            ProgressBar::hidden()
        };
        bar.set_style(
            ProgressStyle::with_template("{bar:30} {pos}/{len} {msg} [{elapsed_precise}, ~{eta}]")
                .expect("the template is a constant"),
        );
        Self {
            bar,
            counts: Counts::default(),
        }
    }

    pub fn record(&mut self, verdict: &Verdict) {
        self.counts = Counts {
            killed: self.counts.killed + u32::from(verdict.status == Status::Killed),
            survived: self.counts.survived + u32::from(verdict.status == Status::Survived),
            timeout: self.counts.timeout + u32::from(verdict.status == Status::Timeout),
            ..self.counts
        };
        self.bar.set_message(format!(
            "killed {} · survived {} · timeout {}",
            self.counts.killed, self.counts.survived, self.counts.timeout
        ));
        self.bar.inc(1);
    }

    pub fn finish(&self) {
        self.bar.finish_and_clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(status: Status) -> Verdict {
        Verdict {
            mutant: 0,
            status,
            reason: None,
        }
    }

    #[test]
    fn each_verdict_advances_the_bar_and_its_tally() {
        let mut progress = Progress::new(5, false);
        for status in [
            Status::Killed,
            Status::Killed,
            Status::Survived,
            Status::Timeout,
            Status::Error,
        ] {
            progress.record(&verdict(status));
        }
        assert_eq!(progress.bar.position(), 5);
        assert_eq!(progress.bar.message(), "killed 2 · survived 1 · timeout 1");
        progress.finish();
        assert!(progress.bar.is_finished());
    }
}
