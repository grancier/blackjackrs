use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Chips(u64);

impl Chips {
    pub const ZERO: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    pub fn checked_add(self, other: Self) -> Result<Self, MoneyError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(MoneyError::Overflow)
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, MoneyError> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or(MoneyError::InsufficientFunds {
                available: self,
                required: other,
            })
    }

    pub fn checked_mul(self, multiplier: u64) -> Result<Self, MoneyError> {
        self.0
            .checked_mul(multiplier)
            .map(Self)
            .ok_or(MoneyError::Overflow)
    }

    pub fn checked_mul_ratio(self, numerator: u64, denominator: u64) -> Result<Self, MoneyError> {
        if denominator == 0 {
            return Err(MoneyError::ZeroDenominator);
        }

        let product = self.0.checked_mul(numerator).ok_or(MoneyError::Overflow)?;
        if product % denominator != 0 {
            return Err(MoneyError::InexactRatio {
                amount: self,
                numerator,
                denominator,
            });
        }

        Ok(Self(product / denominator))
    }
}

impl fmt::Display for Chips {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} chips", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoneyError {
    Overflow,
    InsufficientFunds {
        available: Chips,
        required: Chips,
    },
    InexactRatio {
        amount: Chips,
        numerator: u64,
        denominator: u64,
    },
    ZeroDenominator,
}

impl fmt::Display for MoneyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow => formatter.write_str("chip arithmetic overflow"),
            Self::InsufficientFunds {
                available,
                required,
            } => write!(
                formatter,
                "insufficient funds: {available} available, {required} required"
            ),
            Self::InexactRatio {
                amount,
                numerator,
                denominator,
            } => write!(
                formatter,
                "{amount} cannot be multiplied exactly by {numerator}:{denominator}"
            ),
            Self::ZeroDenominator => formatter.write_str("payout denominator cannot be zero"),
        }
    }
}

impl Error for MoneyError {}

#[cfg(test)]
mod tests {
    use super::{Chips, MoneyError};

    #[test]
    fn checked_addition_and_subtraction_preserve_exact_chips() {
        assert_eq!(
            Chips::new(40).checked_add(Chips::new(2)),
            Ok(Chips::new(42))
        );
        assert_eq!(
            Chips::new(40).checked_sub(Chips::new(41)),
            Err(MoneyError::InsufficientFunds {
                available: Chips::new(40),
                required: Chips::new(41),
            })
        );
    }

    #[test]
    fn checked_arithmetic_reports_overflow() {
        assert_eq!(
            Chips::new(u64::MAX).checked_add(Chips::new(1)),
            Err(MoneyError::Overflow)
        );
        assert_eq!(
            Chips::new(u64::MAX).checked_mul(2),
            Err(MoneyError::Overflow)
        );
    }

    #[test]
    fn ratio_arithmetic_rejects_nonintegral_results() {
        assert_eq!(Chips::new(10).checked_mul_ratio(3, 2), Ok(Chips::new(15)));
        assert_eq!(
            Chips::new(5).checked_mul_ratio(3, 2),
            Err(MoneyError::InexactRatio {
                amount: Chips::new(5),
                numerator: 3,
                denominator: 2,
            })
        );
        assert_eq!(
            Chips::new(5).checked_mul_ratio(1, 0),
            Err(MoneyError::ZeroDenominator)
        );
    }

    #[test]
    fn chips_display_as_whole_units() {
        assert_eq!(Chips::new(125).to_string(), "125 chips");
        assert_eq!(Chips::ZERO.value(), 0);
    }
}
