use crate::config::Thresholds;
use crate::mutant::Status;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub killed: u32,
    pub survived: u32,
    pub timeout: u32,
    pub invalid: u32,
    pub error: u32,
    pub ignored: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    High,
    Low,
    Failing,
}

impl Counts {
    pub fn tally(statuses: impl IntoIterator<Item = Status>) -> Self {
        let mut counts = Self::default();
        for status in statuses {
            *match status {
                Status::Killed => &mut counts.killed,
                Status::Survived => &mut counts.survived,
                Status::Timeout => &mut counts.timeout,
                Status::Invalid => &mut counts.invalid,
                Status::Error => &mut counts.error,
                Status::Ignored => &mut counts.ignored,
            } += 1;
        }
        counts
    }

    #[must_use]
    pub fn detected(self) -> u32 {
        self.killed + self.timeout
    }

    #[must_use]
    pub fn score(self) -> Option<f64> {
        let valid = self.detected() + self.survived;
        (valid > 0).then(|| f64::from(self.detected()) * 100.0 / f64::from(valid))
    }

    #[must_use]
    pub fn band(self, thresholds: Thresholds) -> Band {
        match self.score() {
            None => Band::High,
            Some(score) if score >= f64::from(thresholds.high) => Band::High,
            Some(score) if score >= f64::from(thresholds.low) => Band::Low,
            Some(_) => Band::Failing,
        }
    }

    #[must_use]
    pub fn meets(self, thresholds: Thresholds) -> bool {
        match (self.score(), thresholds.minimum) {
            (Some(score), Some(minimum)) => score >= f64::from(minimum),
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(killed: u32, timeout: u32, survived: u32) -> Counts {
        Counts {
            killed,
            survived,
            timeout,
            ..Counts::default()
        }
    }

    fn thresholds(minimum: Option<u8>) -> Thresholds {
        Thresholds {
            high: 80,
            low: 60,
            minimum,
        }
    }

    #[test]
    fn every_status_is_counted_in_its_own_bucket() {
        let counts = Counts::tally([
            Status::Killed,
            Status::Killed,
            Status::Survived,
            Status::Timeout,
            Status::Invalid,
            Status::Error,
            Status::Ignored,
        ]);
        assert_eq!(
            counts,
            Counts {
                killed: 2,
                survived: 1,
                timeout: 1,
                invalid: 1,
                error: 1,
                ignored: 1
            }
        );
    }

    #[test]
    fn timeouts_count_as_detected_and_the_rest_do_not_count() {
        let counts = Counts {
            invalid: 5,
            error: 5,
            ignored: 5,
            ..counts(2, 1, 1)
        };
        assert_eq!(counts.score(), Some(75.0));
    }

    #[test]
    fn nothing_valid_has_no_score_and_passes() {
        let counts = Counts {
            ignored: 3,
            ..Counts::default()
        };
        assert_eq!(counts.score(), None);
        assert!(counts.meets(thresholds(Some(100))));
        assert_eq!(counts.band(thresholds(None)), Band::High);
    }

    #[test]
    fn bands_include_their_lower_bound() {
        assert_eq!(counts(80, 0, 20).band(thresholds(None)), Band::High);
        assert_eq!(counts(79, 0, 21).band(thresholds(None)), Band::Low);
        assert_eq!(counts(60, 0, 40).band(thresholds(None)), Band::Low);
        assert_eq!(counts(59, 0, 41).band(thresholds(None)), Band::Failing);
    }

    #[test]
    fn break_is_met_at_exactly_the_minimum() {
        assert!(counts(3, 0, 1).meets(thresholds(Some(75))));
        assert!(!counts(3, 0, 1).meets(thresholds(Some(76))));
        assert!(counts(0, 0, 1).meets(thresholds(None)));
    }
}
