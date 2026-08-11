use std::{error::Error, fmt};

use crate::{
    card::Card,
    hand::Hand,
    money::{Chips, MoneyError},
    rules::TableRules,
    shoe::{Shoe, ShoeError},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerAction {
    Hit,
    Stand,
    Double,
    Split,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundPhase {
    InsuranceOffer,
    PlayerTurns,
    DealerTurn,
    Settled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandStatus {
    Active,
    Standing,
    Busted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandOrigin {
    Initial,
    Split,
    SplitAces,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerHand {
    hand: Hand,
    wager: Chips,
    status: HandStatus,
    origin: HandOrigin,
}

impl PlayerHand {
    #[must_use]
    pub fn hand(&self) -> &Hand {
        &self.hand
    }

    #[must_use]
    pub const fn wager(&self) -> Chips {
        self.wager
    }

    #[must_use]
    pub const fn status(&self) -> HandStatus {
        self.status
    }

    #[must_use]
    pub fn is_natural(&self) -> bool {
        self.origin == HandOrigin::Initial && self.hand.is_two_card_twenty_one()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Round {
    rules: TableRules,
    player_hands: Vec<PlayerHand>,
    dealer: Hand,
    phase: RoundPhase,
    active_hand: Option<usize>,
}

impl Round {
    pub fn deal(wager: Chips, rules: TableRules, shoe: &mut Shoe) -> Result<Self, RoundError> {
        let cards: [Card; 4] = shoe
            .draw_many(4)?
            .try_into()
            .map_err(|_| RoundError::InvalidDeal)?;
        let [player_first, dealer_upcard, player_second, dealer_hole] = cards;
        let player = Hand::from_cards([player_first, player_second]);
        let dealer = Hand::from_cards([dealer_upcard, dealer_hole]);
        let player_natural = player.is_two_card_twenty_one();
        let dealer_natural = dealer.is_two_card_twenty_one();
        let dealer_value = dealer_upcard.rank().blackjack_value();

        let phase = if dealer_value == 1 {
            RoundPhase::InsuranceOffer
        } else if (dealer_value == 10 && dealer_natural) || player_natural {
            RoundPhase::Settled
        } else {
            RoundPhase::PlayerTurns
        };
        let status = if player_natural || phase == RoundPhase::Settled {
            HandStatus::Standing
        } else {
            HandStatus::Active
        };
        let active_hand = (status == HandStatus::Active).then_some(0);

        Ok(Self {
            rules,
            player_hands: vec![PlayerHand {
                hand: player,
                wager,
                status,
                origin: HandOrigin::Initial,
            }],
            dealer,
            phase,
            active_hand,
        })
    }

    #[must_use]
    pub const fn phase(&self) -> RoundPhase {
        self.phase
    }

    #[must_use]
    pub fn dealer_upcard(&self) -> Card {
        self.dealer.cards()[0]
    }

    #[must_use]
    pub fn dealer_hand(&self) -> &Hand {
        &self.dealer
    }

    #[must_use]
    pub fn player_hands(&self) -> &[PlayerHand] {
        &self.player_hands
    }

    #[must_use]
    pub const fn active_hand_index(&self) -> Option<usize> {
        self.active_hand
    }

    #[must_use]
    pub fn legal_actions(&self, bankroll: Chips) -> Vec<PlayerAction> {
        let Some(index) = self.active_hand else {
            return Vec::new();
        };
        if self.phase != RoundPhase::PlayerTurns {
            return Vec::new();
        }

        let hand = &self.player_hands[index];
        if hand.origin == HandOrigin::SplitAces {
            return self
                .can_split(index, bankroll)
                .then_some(PlayerAction::Split)
                .into_iter()
                .collect();
        }

        let mut actions = vec![PlayerAction::Hit, PlayerAction::Stand];
        if hand.hand.cards().len() == 2 && bankroll >= hand.wager {
            actions.push(PlayerAction::Double);
        }
        if self.can_split(index, bankroll) {
            actions.push(PlayerAction::Split);
        }
        actions
    }

    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "called by session orchestration added in task 7")
    )]
    pub(crate) fn act(
        &mut self,
        action: PlayerAction,
        bankroll: &mut Chips,
        shoe: &mut Shoe,
    ) -> Result<(), RoundError> {
        if !self.legal_actions(*bankroll).contains(&action) {
            return Err(RoundError::IllegalAction(action));
        }
        let index = self
            .active_hand
            .ok_or(RoundError::InvalidPhase(self.phase))?;

        match action {
            PlayerAction::Hit => self.hit(index, shoe)?,
            PlayerAction::Stand => self.player_hands[index].status = HandStatus::Standing,
            PlayerAction::Double => self.double(index, bankroll, shoe)?,
            PlayerAction::Split => {
                self.split(index, bankroll, shoe)?;
                return Ok(());
            }
        }

        if self.player_hands[index].status != HandStatus::Active {
            self.advance_after(index);
        }
        Ok(())
    }

    fn can_split(&self, index: usize, bankroll: Chips) -> bool {
        self.player_hands.len() < self.rules.maximum_hands()
            && bankroll >= self.player_hands[index].wager
            && self.player_hands[index].hand.split_value().is_some()
    }

    fn hit(&mut self, index: usize, shoe: &mut Shoe) -> Result<(), RoundError> {
        let [card]: [Card; 1] = shoe
            .draw_many(1)?
            .try_into()
            .map_err(|_| RoundError::InvalidDeal)?;
        let hand = &mut self.player_hands[index];
        hand.hand.push(card);
        if hand.hand.is_bust() {
            hand.status = HandStatus::Busted;
        } else if hand.hand.value().total == 21 {
            hand.status = HandStatus::Standing;
        }
        Ok(())
    }

    fn double(
        &mut self,
        index: usize,
        bankroll: &mut Chips,
        shoe: &mut Shoe,
    ) -> Result<(), RoundError> {
        let wager = self.player_hands[index].wager;
        let remaining = bankroll.checked_sub(wager)?;
        let doubled = wager.checked_add(wager)?;
        let [card]: [Card; 1] = shoe
            .draw_many(1)?
            .try_into()
            .map_err(|_| RoundError::InvalidDeal)?;

        *bankroll = remaining;
        let hand = &mut self.player_hands[index];
        hand.wager = doubled;
        hand.hand.push(card);
        hand.status = if hand.hand.is_bust() {
            HandStatus::Busted
        } else {
            HandStatus::Standing
        };
        Ok(())
    }

    fn split(
        &mut self,
        index: usize,
        bankroll: &mut Chips,
        shoe: &mut Shoe,
    ) -> Result<(), RoundError> {
        let current = &self.player_hands[index];
        let [first, second] = current.hand.cards() else {
            return Err(RoundError::IllegalAction(PlayerAction::Split));
        };
        let wager = current.wager;
        let remaining = bankroll.checked_sub(wager)?;
        let [first_draw, second_draw]: [Card; 2] = shoe
            .draw_many(2)?
            .try_into()
            .map_err(|_| RoundError::InvalidDeal)?;
        let origin = if current.hand.split_value() == Some(1) {
            HandOrigin::SplitAces
        } else {
            HandOrigin::Split
        };
        let replacements = [
            Self::split_hand(*first, first_draw, wager, origin),
            Self::split_hand(*second, second_draw, wager, origin),
        ];

        *bankroll = remaining;
        self.player_hands.splice(index..=index, replacements);
        self.normalize_split_aces(*bankroll);
        self.active_hand = self
            .player_hands
            .iter()
            .enumerate()
            .skip(index)
            .find_map(|(next, hand)| (hand.status == HandStatus::Active).then_some(next));
        self.phase = if self.active_hand.is_some() {
            RoundPhase::PlayerTurns
        } else {
            RoundPhase::DealerTurn
        };
        Ok(())
    }

    fn split_hand(card: Card, draw: Card, wager: Chips, origin: HandOrigin) -> PlayerHand {
        let hand = Hand::from_cards([card, draw]);
        let status = if origin == HandOrigin::SplitAces || hand.value().total == 21 {
            if origin == HandOrigin::SplitAces && hand.split_value() == Some(1) {
                HandStatus::Active
            } else {
                HandStatus::Standing
            }
        } else {
            HandStatus::Active
        };
        PlayerHand {
            hand,
            wager,
            status,
            origin,
        }
    }

    fn normalize_split_aces(&mut self, bankroll: Chips) {
        let below_hand_limit = self.player_hands.len() < self.rules.maximum_hands();
        for hand in &mut self.player_hands {
            if hand.origin == HandOrigin::SplitAces
                && hand.status == HandStatus::Active
                && (!below_hand_limit
                    || bankroll < hand.wager
                    || hand.hand.split_value() != Some(1))
            {
                hand.status = HandStatus::Standing;
            }
        }
    }

    fn advance_after(&mut self, index: usize) {
        self.active_hand = self
            .player_hands
            .iter()
            .enumerate()
            .skip(index.saturating_add(1))
            .find_map(|(next, hand)| (hand.status == HandStatus::Active).then_some(next));
        if self.active_hand.is_none() {
            self.phase = RoundPhase::DealerTurn;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundError {
    InvalidDeal,
    InvalidPhase(RoundPhase),
    IllegalAction(PlayerAction),
    Shoe(ShoeError),
    Money(MoneyError),
}

impl fmt::Display for RoundError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDeal => formatter.write_str("the initial deal was incomplete"),
            Self::InvalidPhase(phase) => write!(formatter, "operation is invalid during {phase:?}"),
            Self::IllegalAction(action) => write!(formatter, "action {action:?} is not legal"),
            Self::Shoe(error) => write!(formatter, "shoe error: {error}"),
            Self::Money(error) => write!(formatter, "money error: {error}"),
        }
    }
}

impl Error for RoundError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Shoe(error) => Some(error),
            Self::Money(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ShoeError> for RoundError {
    fn from(error: ShoeError) -> Self {
        Self::Shoe(error)
    }
}

impl From<MoneyError> for RoundError {
    fn from(error: MoneyError) -> Self {
        Self::Money(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{HandStatus, PlayerAction, Round, RoundError, RoundPhase};
    use crate::{
        card::{Card, Rank, Suit},
        money::Chips,
        rules::{BlackjackPayout, Soft17Rule, TableRules},
        shoe::{Shoe, ShoeError},
    };

    fn card(rank: Rank) -> Card {
        Card::new(rank, Suit::Spades)
    }

    fn rules() -> TableRules {
        TableRules::new(1, Soft17Rule::Stand, BlackjackPayout::ThreeToTwo)
            .expect("valid test rules")
    }

    fn deal_with_draws(
        player: [Rank; 2],
        dealer: [Rank; 2],
        draws: impl IntoIterator<Item = Rank>,
    ) -> (Round, Shoe) {
        let initial = [player[0], dealer[0], player[1], dealer[1]];
        let mut shoe = Shoe::ordered(initial.into_iter().chain(draws).map(card));
        let round = Round::deal(Chips::new(10), rules(), &mut shoe).expect("deal succeeds");
        (round, shoe)
    }

    #[test]
    fn deal_uses_player_upcard_player_hole_order() {
        let (round, _) = deal_with_draws([Rank::Five, Rank::Six], [Rank::Nine, Rank::Seven], []);

        assert_eq!(
            round.player_hands()[0].hand().cards(),
            &[card(Rank::Five), card(Rank::Six)]
        );
        assert_eq!(round.dealer_upcard(), card(Rank::Nine));
        assert_eq!(
            round.dealer_hand().cards(),
            &[card(Rank::Nine), card(Rank::Seven)]
        );
        assert_eq!(round.phase(), RoundPhase::PlayerTurns);
        assert_eq!(round.active_hand_index(), Some(0));
    }

    #[test]
    fn ace_upcard_offers_insurance_before_peek() {
        let (round, _) = deal_with_draws([Rank::Nine, Rank::Seven], [Rank::Ace, Rank::King], []);

        assert_eq!(round.phase(), RoundPhase::InsuranceOffer);
        assert!(round.legal_actions(Chips::new(100)).is_empty());
    }

    #[test]
    fn ten_value_upcard_peeks_and_ends_on_dealer_natural() {
        let (round, _) = deal_with_draws([Rank::Nine, Rank::Seven], [Rank::Queen, Rank::Ace], []);

        assert_eq!(round.phase(), RoundPhase::Settled);
        assert_eq!(round.active_hand_index(), None);
    }

    #[test]
    fn player_natural_ends_when_dealer_cannot_have_blackjack() {
        let (round, _) = deal_with_draws([Rank::Ace, Rank::King], [Rank::Nine, Rank::Seven], []);

        assert_eq!(round.phase(), RoundPhase::Settled);
        assert_eq!(round.player_hands()[0].status(), HandStatus::Standing);
    }

    #[test]
    fn legal_actions_include_double_only_when_two_cards_and_funded() {
        let (round, _) = deal_with_draws([Rank::Five, Rank::Six], [Rank::Nine, Rank::Seven], []);

        assert_eq!(
            round.legal_actions(Chips::new(10)),
            vec![PlayerAction::Hit, PlayerAction::Stand, PlayerAction::Double]
        );
        assert_eq!(
            round.legal_actions(Chips::new(9)),
            vec![PlayerAction::Hit, PlayerAction::Stand]
        );
    }

    #[test]
    fn hit_draws_one_card_and_bust_completes_player_turns() {
        let (mut round, mut shoe) = deal_with_draws(
            [Rank::King, Rank::Six],
            [Rank::Nine, Rank::Seven],
            [Rank::Queen],
        );
        let mut bankroll = Chips::new(90);

        round
            .act(PlayerAction::Hit, &mut bankroll, &mut shoe)
            .expect("legal hit");

        assert_eq!(round.player_hands()[0].status(), HandStatus::Busted);
        assert_eq!(round.player_hands()[0].hand().cards().len(), 3);
        assert_eq!(round.phase(), RoundPhase::DealerTurn);
        assert_eq!(bankroll, Chips::new(90));
    }

    #[test]
    fn stand_completes_the_active_hand_without_drawing() {
        let (mut round, mut shoe) = deal_with_draws(
            [Rank::Ten, Rank::Seven],
            [Rank::Nine, Rank::Seven],
            [Rank::Ace],
        );
        let mut bankroll = Chips::new(90);

        round
            .act(PlayerAction::Stand, &mut bankroll, &mut shoe)
            .expect("legal stand");

        assert_eq!(round.player_hands()[0].status(), HandStatus::Standing);
        assert_eq!(round.phase(), RoundPhase::DealerTurn);
        assert_eq!(shoe.len(), 1);
    }

    #[test]
    fn double_reserves_one_wager_draws_once_and_stands() {
        let (mut round, mut shoe) = deal_with_draws(
            [Rank::Five, Rank::Six],
            [Rank::Nine, Rank::Seven],
            [Rank::King, Rank::Ace],
        );
        let mut bankroll = Chips::new(90);

        round
            .act(PlayerAction::Double, &mut bankroll, &mut shoe)
            .expect("legal double");

        assert_eq!(bankroll, Chips::new(80));
        assert_eq!(round.player_hands()[0].wager(), Chips::new(20));
        assert_eq!(round.player_hands()[0].hand().value().total, 21);
        assert_eq!(round.player_hands()[0].status(), HandStatus::Standing);
        assert_eq!(shoe.len(), 1);
    }

    #[test]
    fn illegal_action_does_not_mutate_round_bankroll_or_shoe() {
        let (mut round, mut shoe) = deal_with_draws(
            [Rank::Five, Rank::Six],
            [Rank::Nine, Rank::Seven],
            [Rank::King],
        );
        let mut bankroll = Chips::new(90);

        assert_eq!(
            round.act(PlayerAction::Split, &mut bankroll, &mut shoe),
            Err(RoundError::IllegalAction(PlayerAction::Split))
        );
        assert_eq!(bankroll, Chips::new(90));
        assert_eq!(shoe.len(), 1);
        assert_eq!(round.player_hands()[0].hand().cards().len(), 2);
    }

    #[test]
    fn exhausted_shoe_does_not_partially_apply_hit() {
        let (mut round, mut shoe) =
            deal_with_draws([Rank::Five, Rank::Six], [Rank::Nine, Rank::Seven], []);
        let mut bankroll = Chips::new(90);

        assert_eq!(
            round.act(PlayerAction::Hit, &mut bankroll, &mut shoe),
            Err(RoundError::Shoe(ShoeError::InsufficientCards {
                requested: 1,
                remaining: 0,
            }))
        );
        assert_eq!(bankroll, Chips::new(90));
        assert_eq!(round.player_hands()[0].hand().cards().len(), 2);
        assert_eq!(round.phase(), RoundPhase::PlayerTurns);
    }

    #[test]
    fn equal_value_cards_can_split_when_funded() {
        let (round, _) = deal_with_draws([Rank::King, Rank::Queen], [Rank::Nine, Rank::Seven], []);

        assert!(
            round
                .legal_actions(Chips::new(10))
                .contains(&PlayerAction::Split)
        );
        assert!(
            !round
                .legal_actions(Chips::new(9))
                .contains(&PlayerAction::Split)
        );
    }

    #[test]
    fn split_reserves_matching_wager_and_deals_to_both_hands() {
        let (mut round, mut shoe) = deal_with_draws(
            [Rank::Eight, Rank::Eight],
            [Rank::Nine, Rank::Seven],
            [Rank::Three, Rank::King],
        );
        let mut bankroll = Chips::new(90);

        round
            .act(PlayerAction::Split, &mut bankroll, &mut shoe)
            .expect("legal split");

        assert_eq!(bankroll, Chips::new(80));
        assert_eq!(round.player_hands().len(), 2);
        assert_eq!(
            round.player_hands()[0].hand().cards(),
            &[card(Rank::Eight), card(Rank::Three)]
        );
        assert_eq!(
            round.player_hands()[1].hand().cards(),
            &[card(Rank::Eight), card(Rank::King)]
        );
        assert_eq!(round.active_hand_index(), Some(0));
    }

    #[test]
    fn double_remains_legal_after_split() {
        let (mut round, mut shoe) = deal_with_draws(
            [Rank::Eight, Rank::Eight],
            [Rank::Nine, Rank::Seven],
            [Rank::Three, Rank::Two, Rank::King],
        );
        let mut bankroll = Chips::new(90);

        round
            .act(PlayerAction::Split, &mut bankroll, &mut shoe)
            .expect("legal split");
        round
            .act(PlayerAction::Double, &mut bankroll, &mut shoe)
            .expect("double after split");

        assert_eq!(round.player_hands()[0].wager(), Chips::new(20));
        assert_eq!(round.player_hands()[0].hand().value().total, 21);
        assert_eq!(round.active_hand_index(), Some(1));
        assert_eq!(bankroll, Chips::new(70));
    }

    #[test]
    fn resplitting_stops_at_four_total_hands() {
        let (mut round, mut shoe) = deal_with_draws(
            [Rank::Eight, Rank::Eight],
            [Rank::Nine, Rank::Seven],
            [
                Rank::Eight,
                Rank::Eight,
                Rank::Eight,
                Rank::Two,
                Rank::Three,
                Rank::Four,
            ],
        );
        let mut bankroll = Chips::new(90);

        round
            .act(PlayerAction::Split, &mut bankroll, &mut shoe)
            .expect("first split");
        round
            .act(PlayerAction::Split, &mut bankroll, &mut shoe)
            .expect("first resplit");
        round
            .act(PlayerAction::Split, &mut bankroll, &mut shoe)
            .expect("second resplit");
        round
            .act(PlayerAction::Stand, &mut bankroll, &mut shoe)
            .expect("stand first");
        round
            .act(PlayerAction::Stand, &mut bankroll, &mut shoe)
            .expect("stand second");
        round
            .act(PlayerAction::Stand, &mut bankroll, &mut shoe)
            .expect("stand third");

        assert_eq!(round.player_hands().len(), 4);
        assert_eq!(round.active_hand_index(), Some(3));
        assert!(!round.legal_actions(bankroll).contains(&PlayerAction::Split));
        assert_eq!(
            round.act(PlayerAction::Split, &mut bankroll, &mut shoe),
            Err(RoundError::IllegalAction(PlayerAction::Split))
        );
    }

    #[test]
    fn split_aces_take_one_card_each_and_stand() {
        let (mut round, mut shoe) = deal_with_draws(
            [Rank::Ace, Rank::Ace],
            [Rank::Nine, Rank::Seven],
            [Rank::Nine, Rank::King],
        );
        let mut bankroll = Chips::new(90);

        round
            .act(PlayerAction::Split, &mut bankroll, &mut shoe)
            .expect("legal split");

        assert_eq!(round.player_hands().len(), 2);
        assert!(round.player_hands().iter().all(|hand| {
            hand.hand().cards().len() == 2 && hand.status() == HandStatus::Standing
        }));
        assert_eq!(round.phase(), RoundPhase::DealerTurn);
    }

    #[test]
    fn split_aces_can_resplit_when_another_ace_is_dealt() {
        let (mut round, mut shoe) = deal_with_draws(
            [Rank::Ace, Rank::Ace],
            [Rank::Nine, Rank::Seven],
            [Rank::Ace, Rank::Nine, Rank::Five, Rank::Six],
        );
        let mut bankroll = Chips::new(90);

        round
            .act(PlayerAction::Split, &mut bankroll, &mut shoe)
            .expect("split Aces");
        assert_eq!(round.legal_actions(bankroll), vec![PlayerAction::Split]);
        round
            .act(PlayerAction::Split, &mut bankroll, &mut shoe)
            .expect("resplit Aces");

        assert_eq!(round.player_hands().len(), 3);
        assert!(
            round
                .player_hands()
                .iter()
                .all(|hand| hand.status() == HandStatus::Standing)
        );
        assert_eq!(round.phase(), RoundPhase::DealerTurn);
    }

    #[test]
    fn post_split_twenty_one_is_not_a_natural() {
        let (mut round, mut shoe) = deal_with_draws(
            [Rank::King, Rank::Queen],
            [Rank::Nine, Rank::Seven],
            [Rank::Ace, Rank::Ace],
        );
        let mut bankroll = Chips::new(90);

        round
            .act(PlayerAction::Split, &mut bankroll, &mut shoe)
            .expect("legal split");

        assert!(
            round
                .player_hands()
                .iter()
                .all(|hand| { hand.hand().is_two_card_twenty_one() && !hand.is_natural() })
        );
    }
}
