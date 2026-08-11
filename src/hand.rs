use crate::card::{Card, Rank};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandValue {
    pub total: u16,
    pub is_soft: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Hand {
    cards: Vec<Card>,
}

impl Hand {
    #[must_use]
    pub const fn new() -> Self {
        Self { cards: Vec::new() }
    }

    pub fn from_cards(cards: impl IntoIterator<Item = Card>) -> Self {
        Self {
            cards: cards.into_iter().collect(),
        }
    }

    pub fn push(&mut self, card: Card) {
        self.cards.push(card);
    }

    #[must_use]
    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    #[must_use]
    pub fn value(&self) -> HandValue {
        let mut total = 0_u16;
        let mut has_ace = false;

        for card in &self.cards {
            total = total.saturating_add(u16::from(card.rank().blackjack_value()));
            has_ace |= card.rank() == Rank::Ace;
        }

        let is_soft = has_ace && total <= 11;
        if is_soft {
            total = total.saturating_add(10);
        }

        HandValue { total, is_soft }
    }

    #[must_use]
    pub fn is_bust(&self) -> bool {
        self.value().total > 21
    }

    #[must_use]
    pub fn is_two_card_twenty_one(&self) -> bool {
        self.cards.len() == 2 && self.value().total == 21
    }

    #[must_use]
    pub fn split_value(&self) -> Option<u8> {
        match self.cards.as_slice() {
            [first, second]
                if first.rank().blackjack_value() == second.rank().blackjack_value() =>
            {
                Some(first.rank().blackjack_value())
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Hand, HandValue};
    use crate::card::{Card, Rank, Suit};

    fn card(rank: Rank) -> Card {
        Card::new(rank, Suit::Spades)
    }

    #[test]
    fn empty_hand_has_zero_hard_value() {
        assert_eq!(
            Hand::new().value(),
            HandValue {
                total: 0,
                is_soft: false,
            }
        );
    }

    #[test]
    fn ace_is_promoted_when_it_cannot_bust_the_hand() {
        let hand = Hand::from_cards([card(Rank::Ace), card(Rank::Six)]);

        assert_eq!(
            hand.value(),
            HandValue {
                total: 17,
                is_soft: true,
            }
        );
    }

    #[test]
    fn three_aces_and_an_eight_total_twenty_one_soft() {
        let hand = Hand::from_cards([
            card(Rank::Ace),
            card(Rank::Ace),
            card(Rank::Ace),
            card(Rank::Eight),
        ]);

        assert_eq!(
            hand.value(),
            HandValue {
                total: 21,
                is_soft: true,
            }
        );
    }

    #[test]
    fn ace_is_demoted_when_promotion_would_bust() {
        let hand = Hand::from_cards([card(Rank::Ace), card(Rank::King), card(Rank::Queen)]);

        assert_eq!(
            hand.value(),
            HandValue {
                total: 21,
                is_soft: false,
            }
        );
    }

    #[test]
    fn two_card_twenty_one_is_detected() {
        let natural = Hand::from_cards([card(Rank::Ace), card(Rank::King)]);
        let three_card_twenty_one =
            Hand::from_cards([card(Rank::Seven), card(Rank::Seven), card(Rank::Seven)]);

        assert!(natural.is_two_card_twenty_one());
        assert!(!three_card_twenty_one.is_two_card_twenty_one());
    }

    #[test]
    fn totals_over_twenty_one_are_busts() {
        let mut hand = Hand::from_cards([card(Rank::King), card(Rank::Queen)]);

        assert!(!hand.is_bust());
        hand.push(card(Rank::Two));
        assert!(hand.is_bust());
        assert_eq!(hand.cards().len(), 3);
    }

    #[test]
    fn split_value_compares_blackjack_values_for_two_cards_only() {
        let ten_values = Hand::from_cards([card(Rank::King), card(Rank::Queen)]);
        let different = Hand::from_cards([card(Rank::Eight), card(Rank::Nine)]);
        let three_cards =
            Hand::from_cards([card(Rank::Eight), card(Rank::Eight), card(Rank::Eight)]);

        assert_eq!(ten_values.split_value(), Some(10));
        assert_eq!(different.split_value(), None);
        assert_eq!(three_cards.split_value(), None);
    }
}
