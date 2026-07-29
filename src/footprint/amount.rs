use super::Error;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Estimate(u64);

impl Estimate {
    pub const ZERO: Self = Self(0);

    pub const fn from_bytes(bytes: u64) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> u64 {
        self.0
    }

    pub fn checked_add(self, other: Self) -> Result<Self, Error> {
        self.0.checked_add(other.0).map(Self).ok_or(Error::Overflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_rejects_overflow() {
        assert!(matches!(
            Estimate::from_bytes(u64::MAX).checked_add(Estimate::from_bytes(1)),
            Err(Error::Overflow)
        ));
    }
}
