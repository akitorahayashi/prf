use crate::footprint::{Error, Estimate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionEstimate {
    Known(Estimate),
    Unestimated,
}

impl ActionEstimate {
    pub const fn known(self) -> Option<Estimate> {
        match self {
            Self::Known(estimate) => Some(estimate),
            Self::Unestimated => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EstimateSummary {
    known: Estimate,
    unestimated_actions: usize,
}

impl EstimateSummary {
    pub const ZERO: Self = Self { known: Estimate::ZERO, unestimated_actions: 0 };

    pub const fn new(known: Estimate, unestimated_actions: usize) -> Self {
        Self { known, unestimated_actions }
    }

    pub const fn known(self) -> Estimate {
        self.known
    }

    pub const fn unestimated_actions(self) -> usize {
        self.unestimated_actions
    }

    pub fn checked_add(self, other: Self) -> Result<Self, Error> {
        Ok(Self {
            known: self.known.checked_add(other.known)?,
            unestimated_actions: self
                .unestimated_actions
                .checked_add(other.unestimated_actions)
                .ok_or(Error::Overflow)?,
        })
    }
}

impl From<ActionEstimate> for EstimateSummary {
    fn from(estimate: ActionEstimate) -> Self {
        match estimate {
            ActionEstimate::Known(estimate) => Self::new(estimate, 0),
            ActionEstimate::Unestimated => Self::new(Estimate::ZERO, 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_keeps_known_bytes_and_unestimated_actions_distinct() {
        let known = EstimateSummary::from(ActionEstimate::Known(Estimate::from_bytes(42)));
        let unestimated = EstimateSummary::from(ActionEstimate::Unestimated);

        assert_eq!(
            known.checked_add(unestimated).expect("summary adds"),
            EstimateSummary::new(Estimate::from_bytes(42), 1)
        );
    }
}
