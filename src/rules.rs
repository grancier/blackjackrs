use std::{error::Error, fmt};

use crate::money::Chips;

const MIN_DECKS: u8 = 1;
const MAX_DECKS: u8 = 8;
const MAXIMUM_HANDS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Soft17Rule {
    Stand,
    Hit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlackjackPayout {
    ThreeToTwo,
    SixToFive,
}

impl BlackjackPayout {
    #[must_use]
    pub const fn profit_ratio(self) -> (u64, u64) {
        match self {
            Self::ThreeToTwo => (3, 2),
            Self::SixToFive => (6, 5),
        }
    }

    #[must_use]
    pub const fn wager_increment(self) -> Chips {
        match self {
            Self::ThreeToTwo => Chips::new(2),
            Self::SixToFive => Chips::new(10),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableRules {
    deck_count: u8,
    soft_17: Soft17Rule,
    blackjack_payout: BlackjackPayout,
}

impl TableRules {
    pub fn new(
        deck_count: u8,
        soft_17: Soft17Rule,
        blackjack_payout: BlackjackPayout,
    ) -> Result<Self, RuleError> {
        if !(MIN_DECKS..=MAX_DECKS).contains(&deck_count) {
            return Err(RuleError::InvalidDeckCount(deck_count));
        }

        Ok(Self {
            deck_count,
            soft_17,
            blackjack_payout,
        })
    }

    #[must_use]
    pub const fn deck_count(self) -> u8 {
        self.deck_count
    }

    #[must_use]
    pub const fn soft_17(self) -> Soft17Rule {
        self.soft_17
    }

    #[must_use]
    pub const fn blackjack_payout(self) -> BlackjackPayout {
        self.blackjack_payout
    }

    #[must_use]
    pub const fn blackjack_profit_ratio(self) -> (u64, u64) {
        self.blackjack_payout.profit_ratio()
    }

    #[must_use]
    pub const fn wager_increment(self) -> Chips {
        self.blackjack_payout.wager_increment()
    }

    #[must_use]
    pub const fn maximum_hands(self) -> usize {
        MAXIMUM_HANDS
    }

    pub fn validate_wager(self, wager: Chips, bankroll: Chips) -> Result<(), WagerError> {
        if wager == Chips::ZERO {
            return Err(WagerError::ZeroWager);
        }
        if wager > bankroll {
            return Err(WagerError::InsufficientFunds {
                available: bankroll,
                required: wager,
            });
        }

        let increment = self.wager_increment();
        if wager.value() % increment.value() != 0 {
            return Err(WagerError::InvalidIncrement {
                required: increment,
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleError {
    InvalidDeckCount(u8),
}

impl fmt::Display for RuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDeckCount(count) => {
                write!(formatter, "deck count must be between 1 and 8, got {count}")
            }
        }
    }
}

impl Error for RuleError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WagerError {
    ZeroWager,
    InsufficientFunds { available: Chips, required: Chips },
    InvalidIncrement { required: Chips },
}

impl fmt::Display for WagerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroWager => formatter.write_str("wager must be greater than zero"),
            Self::InsufficientFunds {
                available,
                required,
            } => write!(
                formatter,
                "insufficient funds: {available} available, {required} required"
            ),
            Self::InvalidIncrement { required } => {
                write!(formatter, "wager must be a multiple of {required}")
            }
        }
    }
}

impl Error for WagerError {}

#[cfg(test)]
mod tests {
    use super::{BlackjackPayout, RuleError, Soft17Rule, TableRules, WagerError};
    use crate::money::Chips;

    #[test]
    fn deck_count_must_be_between_one_and_eight() {
        assert_eq!(
            TableRules::new(0, Soft17Rule::Stand, BlackjackPayout::ThreeToTwo),
            Err(RuleError::InvalidDeckCount(0))
        );
        assert_eq!(
            TableRules::new(9, Soft17Rule::Stand, BlackjackPayout::ThreeToTwo),
            Err(RuleError::InvalidDeckCount(9))
        );
        assert!(TableRules::new(1, Soft17Rule::Stand, BlackjackPayout::ThreeToTwo).is_ok());
        assert!(TableRules::new(8, Soft17Rule::Hit, BlackjackPayout::SixToFive).is_ok());
    }

    #[test]
    fn three_to_two_requires_even_wagers() {
        let rules = TableRules::new(6, Soft17Rule::Stand, BlackjackPayout::ThreeToTwo)
            .expect("valid test rules");

        assert_eq!(
            rules.validate_wager(Chips::new(20), Chips::new(100)),
            Ok(())
        );
        assert_eq!(
            rules.validate_wager(Chips::new(11), Chips::new(100)),
            Err(WagerError::InvalidIncrement {
                required: Chips::new(2),
            })
        );
    }

    #[test]
    fn six_to_five_requires_ten_chip_wager_increments() {
        let rules = TableRules::new(6, Soft17Rule::Stand, BlackjackPayout::SixToFive)
            .expect("valid test rules");

        assert_eq!(
            rules.validate_wager(Chips::new(20), Chips::new(100)),
            Ok(())
        );
        assert_eq!(
            rules.validate_wager(Chips::new(12), Chips::new(100)),
            Err(WagerError::InvalidIncrement {
                required: Chips::new(10),
            })
        );
    }

    #[test]
    fn wager_must_be_positive_and_affordable() {
        let rules = TableRules::new(6, Soft17Rule::Stand, BlackjackPayout::ThreeToTwo)
            .expect("valid test rules");

        assert_eq!(
            rules.validate_wager(Chips::ZERO, Chips::new(100)),
            Err(WagerError::ZeroWager)
        );
        assert_eq!(
            rules.validate_wager(Chips::new(102), Chips::new(100)),
            Err(WagerError::InsufficientFunds {
                available: Chips::new(100),
                required: Chips::new(102),
            })
        );
    }

    #[test]
    fn validated_rules_expose_table_configuration() {
        let rules = TableRules::new(4, Soft17Rule::Hit, BlackjackPayout::SixToFive)
            .expect("valid test rules");

        assert_eq!(rules.deck_count(), 4);
        assert_eq!(rules.soft_17(), Soft17Rule::Hit);
        assert_eq!(rules.blackjack_payout(), BlackjackPayout::SixToFive);
        assert_eq!(rules.blackjack_profit_ratio(), (6, 5));
        assert_eq!(rules.wager_increment(), Chips::new(10));
        assert_eq!(rules.maximum_hands(), 4);
    }
}
