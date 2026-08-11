use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl fmt::Display for Suit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let symbol = match self {
            Self::Clubs => "♣",
            Self::Diamonds => "♦",
            Self::Hearts => "♥",
            Self::Spades => "♠",
        };
        formatter.write_str(symbol)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rank {
    Ace,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
}

impl Rank {
    #[must_use]
    pub const fn blackjack_value(self) -> u8 {
        match self {
            Self::Ace => 1,
            Self::Two => 2,
            Self::Three => 3,
            Self::Four => 4,
            Self::Five => 5,
            Self::Six => 6,
            Self::Seven => 7,
            Self::Eight => 8,
            Self::Nine => 9,
            Self::Ten | Self::Jack | Self::Queen | Self::King => 10,
        }
    }
}

impl fmt::Display for Rank {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Ace => "A",
            Self::Two => "2",
            Self::Three => "3",
            Self::Four => "4",
            Self::Five => "5",
            Self::Six => "6",
            Self::Seven => "7",
            Self::Eight => "8",
            Self::Nine => "9",
            Self::Ten => "10",
            Self::Jack => "J",
            Self::Queen => "Q",
            Self::King => "K",
        };
        formatter.write_str(label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Card {
    rank: Rank,
    suit: Suit,
}

impl Card {
    #[must_use]
    pub const fn new(rank: Rank, suit: Suit) -> Self {
        Self { rank, suit }
    }

    #[must_use]
    pub const fn rank(self) -> Rank {
        self.rank
    }

    #[must_use]
    pub const fn suit(self) -> Suit {
        self.suit
    }
}

impl fmt::Display for Card {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}{}", self.rank, self.suit)
    }
}

#[cfg(test)]
mod tests {
    use super::{Card, Rank, Suit};

    #[test]
    fn ranks_have_blackjack_values() {
        assert_eq!(Rank::Ace.blackjack_value(), 1);
        assert_eq!(Rank::Two.blackjack_value(), 2);
        assert_eq!(Rank::Nine.blackjack_value(), 9);
        assert_eq!(Rank::Ten.blackjack_value(), 10);
        assert_eq!(Rank::Jack.blackjack_value(), 10);
        assert_eq!(Rank::Queen.blackjack_value(), 10);
        assert_eq!(Rank::King.blackjack_value(), 10);
    }

    #[test]
    fn card_display_combines_rank_and_suit() {
        assert_eq!(Card::new(Rank::Ace, Suit::Spades).to_string(), "A♠");
        assert_eq!(Card::new(Rank::Ten, Suit::Hearts).to_string(), "10♥");
        assert_eq!(Card::new(Rank::Queen, Suit::Diamonds).to_string(), "Q♦");
    }

    #[test]
    fn card_exposes_immutable_rank_and_suit() {
        let card = Card::new(Rank::Seven, Suit::Clubs);

        assert_eq!(card.rank(), Rank::Seven);
        assert_eq!(card.suit(), Suit::Clubs);
    }
}
