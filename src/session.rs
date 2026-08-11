use std::{error::Error, fmt};

use rand::rngs::StdRng;

use crate::{
    money::{Chips, MoneyError},
    round::{PlayerAction, Round, RoundError, RoundPhase, RoundSettlement},
    rules::{TableRules, WagerError},
    shoe::{Shoe, ShoeError},
};

#[derive(Debug)]
struct ActiveRound {
    round: Round,
    shoe: Shoe,
}

#[derive(Debug)]
struct CompletedRound {
    round: Round,
    settlement: RoundSettlement,
}

#[derive(Debug)]
pub struct Session {
    bankroll: Chips,
    rules: TableRules,
    rng: StdRng,
    active: Option<ActiveRound>,
    completed: Option<CompletedRound>,
}

impl Session {
    #[must_use]
    pub const fn new(bankroll: Chips, rules: TableRules, rng: StdRng) -> Self {
        Self {
            bankroll,
            rules,
            rng,
            active: None,
            completed: None,
        }
    }

    #[must_use]
    pub const fn bankroll(&self) -> Chips {
        self.bankroll
    }

    #[must_use]
    pub const fn rules(&self) -> TableRules {
        self.rules
    }

    #[must_use]
    pub fn round(&self) -> Option<&Round> {
        self.active
            .as_ref()
            .map(|active| &active.round)
            .or_else(|| self.completed.as_ref().map(|completed| &completed.round))
    }

    #[must_use]
    pub fn can_place_minimum_wager(&self) -> bool {
        self.rules
            .validate_wager(self.rules.wager_increment(), self.bankroll)
            .is_ok()
    }

    pub fn start_round(&mut self, wager: Chips) -> Result<(), SessionError> {
        let remaining = self.validate_round_start(wager)?;
        let shoe = Shoe::shuffled(self.rules.deck_count(), &mut self.rng)?;
        self.start_prevalidated_round(wager, remaining, shoe)
    }

    fn start_prevalidated_round(
        &mut self,
        wager: Chips,
        remaining: Chips,
        mut shoe: Shoe,
    ) -> Result<(), SessionError> {
        let round = Round::deal(wager, self.rules, &mut shoe)?;

        self.bankroll = remaining;
        self.active = Some(ActiveRound { round, shoe });
        self.advance_round()
    }

    pub fn place_insurance(&mut self, amount: Chips) -> Result<(), SessionError> {
        let active = self.active.as_mut().ok_or(SessionError::NoActiveRound)?;
        active.round.place_insurance(amount, &mut self.bankroll)?;
        self.advance_round()
    }

    pub fn act(&mut self, action: PlayerAction) -> Result<(), SessionError> {
        let active = self.active.as_mut().ok_or(SessionError::NoActiveRound)?;
        active
            .round
            .act(action, &mut self.bankroll, &mut active.shoe)?;
        self.advance_round()
    }

    pub fn finish_round(&mut self) -> Result<Option<RoundSettlement>, SessionError> {
        Ok(self.completed.take().map(|completed| completed.settlement))
    }

    fn validate_round_start(&self, wager: Chips) -> Result<Chips, SessionError> {
        if self.active.is_some() {
            return Err(SessionError::RoundInProgress);
        }
        if self.completed.is_some() {
            return Err(SessionError::PendingSettlement);
        }
        self.rules.validate_wager(wager, self.bankroll)?;
        Ok(self.bankroll.checked_sub(wager)?)
    }

    fn advance_round(&mut self) -> Result<(), SessionError> {
        let Some(active) = self.active.as_mut() else {
            return Err(SessionError::NoActiveRound);
        };
        if active.round.phase() == RoundPhase::DealerTurn {
            active.round.play_dealer(&mut active.shoe)?;
        }
        if active.round.phase() != RoundPhase::Settled {
            return Ok(());
        }

        let credit = active
            .round
            .settlement()
            .ok_or(SessionError::MissingSettlement)?
            .total_credit();
        let updated_bankroll = self.bankroll.checked_add(credit)?;
        let mut active = self.active.take().ok_or(SessionError::NoActiveRound)?;
        let settlement = active
            .round
            .take_settlement()
            .ok_or(SessionError::MissingSettlement)?;

        self.bankroll = updated_bankroll;
        self.completed = Some(CompletedRound {
            round: active.round,
            settlement,
        });
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    RoundInProgress,
    PendingSettlement,
    NoActiveRound,
    MissingSettlement,
    Wager(WagerError),
    Money(MoneyError),
    Round(RoundError),
    Shoe(ShoeError),
}

impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RoundInProgress => formatter.write_str("a round is already in progress"),
            Self::PendingSettlement => {
                formatter.write_str("the completed round must be acknowledged first")
            }
            Self::NoActiveRound => formatter.write_str("there is no active round"),
            Self::MissingSettlement => formatter.write_str("settled round has no settlement"),
            Self::Wager(error) => write!(formatter, "invalid wager: {error}"),
            Self::Money(error) => write!(formatter, "money error: {error}"),
            Self::Round(error) => write!(formatter, "round error: {error}"),
            Self::Shoe(error) => write!(formatter, "shoe error: {error}"),
        }
    }
}

impl Error for SessionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Wager(error) => Some(error),
            Self::Money(error) => Some(error),
            Self::Round(error) => Some(error),
            Self::Shoe(error) => Some(error),
            _ => None,
        }
    }
}

impl From<WagerError> for SessionError {
    fn from(error: WagerError) -> Self {
        Self::Wager(error)
    }
}

impl From<MoneyError> for SessionError {
    fn from(error: MoneyError) -> Self {
        Self::Money(error)
    }
}

impl From<RoundError> for SessionError {
    fn from(error: RoundError) -> Self {
        Self::Round(error)
    }
}

impl From<ShoeError> for SessionError {
    fn from(error: ShoeError) -> Self {
        Self::Shoe(error)
    }
}

#[cfg(test)]
mod tests {
    use rand::{SeedableRng, rngs::StdRng};

    use super::{Session, SessionError};
    use crate::{
        card::{Card, Rank, Suit},
        money::Chips,
        round::{PlayerAction, RoundOutcome, RoundPhase},
        rules::{BlackjackPayout, Soft17Rule, TableRules, WagerError},
        shoe::Shoe,
    };

    fn card(rank: Rank) -> Card {
        Card::new(rank, Suit::Spades)
    }

    fn rules(payout: BlackjackPayout) -> TableRules {
        TableRules::new(1, Soft17Rule::Stand, payout).expect("valid test rules")
    }

    fn session(bankroll: u64) -> Session {
        Session::new(
            Chips::new(bankroll),
            rules(BlackjackPayout::ThreeToTwo),
            StdRng::from_seed([11_u8; 32]),
        )
    }

    fn shoe(player: [Rank; 2], dealer: [Rank; 2], draws: impl IntoIterator<Item = Rank>) -> Shoe {
        Shoe::ordered(
            [player[0], dealer[0], player[1], dealer[1]]
                .into_iter()
                .chain(draws)
                .map(card),
        )
    }

    fn start_with_shoe(
        session: &mut Session,
        wager: Chips,
        shoe: Shoe,
    ) -> Result<(), SessionError> {
        let remaining = session.validate_round_start(wager)?;
        session.start_prevalidated_round(wager, remaining, shoe)
    }

    #[test]
    fn starting_round_reserves_valid_wager() {
        let mut session = session(100);

        start_with_shoe(
            &mut session,
            Chips::new(10),
            shoe([Rank::Five, Rank::Six], [Rank::Nine, Rank::Seven], []),
        )
        .expect("round starts");

        assert_eq!(session.bankroll(), Chips::new(90));
        assert_eq!(
            session.round().expect("active round").phase(),
            RoundPhase::PlayerTurns
        );
    }

    #[test]
    fn invalid_wager_does_not_change_session() {
        let mut session = session(100);

        assert_eq!(
            start_with_shoe(
                &mut session,
                Chips::new(11),
                shoe([Rank::Five, Rank::Six], [Rank::Nine, Rank::Seven], []),
            ),
            Err(SessionError::Wager(WagerError::InvalidIncrement {
                required: Chips::new(2),
            }))
        );
        assert_eq!(session.bankroll(), Chips::new(100));
        assert!(session.round().is_none());
    }

    #[test]
    fn session_rejects_overlapping_rounds() {
        let mut session = session(100);
        start_with_shoe(
            &mut session,
            Chips::new(10),
            shoe([Rank::Five, Rank::Six], [Rank::Nine, Rank::Seven], []),
        )
        .expect("round starts");

        assert_eq!(
            session.start_round(Chips::new(10)),
            Err(SessionError::RoundInProgress)
        );
    }

    #[test]
    fn completed_round_is_credited_exactly_once() {
        let mut session = session(100);
        start_with_shoe(
            &mut session,
            Chips::new(10),
            shoe([Rank::Ace, Rank::King], [Rank::Nine, Rank::Seven], []),
        )
        .expect("natural settles");

        assert_eq!(session.bankroll(), Chips::new(115));
        let settlement = session
            .finish_round()
            .expect("finish succeeds")
            .expect("settled");
        assert_eq!(
            settlement.hand_results()[0].outcome(),
            RoundOutcome::Blackjack
        );
        assert!(session.finish_round().expect("idempotent finish").is_none());
        assert_eq!(session.bankroll(), Chips::new(115));
    }

    #[test]
    fn action_completion_plays_dealer_and_credits_bankroll() {
        let mut session = session(100);
        start_with_shoe(
            &mut session,
            Chips::new(10),
            shoe([Rank::Ten, Rank::Nine], [Rank::Ten, Rank::Eight], []),
        )
        .expect("round starts");

        session.act(PlayerAction::Stand).expect("stand settles");

        assert_eq!(session.bankroll(), Chips::new(110));
        assert_eq!(
            session
                .finish_round()
                .expect("finish succeeds")
                .expect("settled")
                .hand_results()[0]
                .outcome(),
            RoundOutcome::Win
        );
    }

    #[test]
    fn insurance_completion_is_delegated_and_credited() {
        let mut session = session(100);
        start_with_shoe(
            &mut session,
            Chips::new(10),
            shoe([Rank::Nine, Rank::Seven], [Rank::Ace, Rank::King], []),
        )
        .expect("insurance offer starts");

        session
            .place_insurance(Chips::new(5))
            .expect("insurance settles");

        assert_eq!(session.bankroll(), Chips::new(100));
        let settlement = session
            .finish_round()
            .expect("finish succeeds")
            .expect("settled");
        assert_eq!(settlement.insurance_credit(), Chips::new(15));
    }

    #[test]
    fn pending_settlement_must_be_consumed_before_next_round() {
        let mut session = session(100);
        start_with_shoe(
            &mut session,
            Chips::new(10),
            shoe([Rank::Ace, Rank::King], [Rank::Nine, Rank::Seven], []),
        )
        .expect("natural settles");

        assert_eq!(
            session.start_round(Chips::new(10)),
            Err(SessionError::PendingSettlement)
        );
        assert!(session.round().is_some());
    }

    #[test]
    fn minimum_wager_reflects_table_payout_increment() {
        let three_to_two = session(2);
        let six_to_five = Session::new(
            Chips::new(9),
            rules(BlackjackPayout::SixToFive),
            StdRng::from_seed([12_u8; 32]),
        );

        assert!(three_to_two.can_place_minimum_wager());
        assert!(!six_to_five.can_place_minimum_wager());
    }
}
