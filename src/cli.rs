use std::{
    error::Error,
    fmt,
    io::{self, BufRead, Write},
};

use rand::rngs::StdRng;

use crate::{
    money::{Chips, MoneyError},
    round::{
        HandStatus, PlayerAction, Round, RoundError, RoundOutcome, RoundPhase, RoundSettlement,
    },
    rules::{BlackjackPayout, RuleError, Soft17Rule, TableRules},
    session::{Session, SessionError},
    shoe::EntropySource,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputAction {
    Play(PlayerAction),
    Quit,
}

pub fn run<R: BufRead, W: Write, E: Write>(
    input: &mut R,
    output: &mut W,
    error: &mut E,
    rng: StdRng,
    entropy_source: EntropySource,
) -> Result<(), CliError> {
    if entropy_source == EntropySource::System {
        writeln!(
            error,
            "Warning: /dev/random unavailable; using system RNG fallback."
        )?;
    }

    writeln!(output, "Blackjack")?;
    let Some(bankroll) = prompt_value(
        input,
        output,
        "Starting bankroll (whole chips): ",
        parse_bankroll,
    )?
    else {
        return Ok(());
    };
    let Some(deck_count) = prompt_value(input, output, "Deck count [6]: ", parse_deck_count)?
    else {
        return Ok(());
    };
    let Some(soft_17) = prompt_value(
        input,
        output,
        "Dealer soft 17 [stand/hit, default stand]: ",
        parse_soft_17,
    )?
    else {
        return Ok(());
    };
    let Some(payout) = prompt_value(
        input,
        output,
        "Blackjack payout [3:2/6:5, default 3:2]: ",
        parse_payout,
    )?
    else {
        return Ok(());
    };

    let rules = TableRules::new(deck_count, soft_17, payout)?;
    let mut session = Session::new(bankroll, rules, rng);
    run_session(input, output, &mut session)
}

fn run_session<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    session: &mut Session,
) -> Result<(), CliError> {
    loop {
        writeln!(output, "\nBankroll: {}", session.bankroll())?;
        if !session.can_place_minimum_wager() {
            writeln!(
                output,
                "Game over: bankroll cannot cover the {} minimum wager.",
                session.rules().wager_increment()
            )?;
            return Ok(());
        }

        let Some(line) = prompt_line(
            input,
            output,
            &format!(
                "Bet (multiple of {}, or quit): ",
                session.rules().wager_increment()
            ),
        )?
        else {
            return Ok(());
        };
        if line.eq_ignore_ascii_case("quit") {
            writeln!(output, "Goodbye.")?;
            return Ok(());
        }
        let wager = match parse_chips(&line) {
            Ok(wager) => wager,
            Err(message) => {
                writeln!(output, "{message}")?;
                continue;
            }
        };
        match session.start_round(wager) {
            Ok(()) => play_round(input, output, session)?,
            Err(error @ SessionError::Wager(_)) => writeln!(output, "{error}")?,
            Err(error) => return Err(error.into()),
        }
    }
}

fn play_round<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    session: &mut Session,
) -> Result<(), CliError> {
    loop {
        let round = session.round().ok_or(CliError::MissingRound)?;
        render_round(output, round, session.bankroll())?;
        match round.phase() {
            RoundPhase::InsuranceOffer => {
                let Some(line) = prompt_line(input, output, "Insurance wager (0 to half bet): ")?
                else {
                    return Ok(());
                };
                let amount = match parse_chips(&line) {
                    Ok(amount) => amount,
                    Err(message) => {
                        writeln!(output, "{message}")?;
                        continue;
                    }
                };
                match session.place_insurance(amount) {
                    Ok(()) => {}
                    Err(error) if recoverable_round_input(error) => {
                        writeln!(output, "{error}")?;
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            RoundPhase::PlayerTurns => {
                let actions = round.legal_actions(session.bankroll());
                write!(output, "Actions:")?;
                for action in &actions {
                    write!(output, " {}", action_label(*action))?;
                }
                writeln!(output)?;
                let Some(line) = prompt_line(input, output, "Action: ")? else {
                    return Ok(());
                };
                match parse_action(&line) {
                    Ok(InputAction::Quit) => {
                        writeln!(output, "Quit is only available between rounds.")?;
                    }
                    Ok(InputAction::Play(action)) => match session.act(action) {
                        Ok(()) => {}
                        Err(error) if recoverable_round_input(error) => {
                            writeln!(output, "{error}")?;
                        }
                        Err(error) => return Err(error.into()),
                    },
                    Err(message) => writeln!(output, "{message}")?,
                }
            }
            RoundPhase::Settled => {
                let settlement = session.finish_round()?.ok_or(CliError::MissingSettlement)?;
                render_settlement(output, &settlement, session.bankroll())?;
                return Ok(());
            }
            RoundPhase::DealerTurn => return Err(CliError::UnresolvedDealerTurn),
        }
    }
}

fn render_round<W: Write>(output: &mut W, round: &Round, bankroll: Chips) -> Result<(), io::Error> {
    let reveal_dealer = matches!(round.phase(), RoundPhase::DealerTurn | RoundPhase::Settled);
    if reveal_dealer {
        writeln!(
            output,
            "Dealer: {} (total {})",
            cards_label(round.dealer_hand().cards()),
            round.dealer_hand().value().total
        )?;
    } else {
        writeln!(output, "Dealer: {} [hidden]", round.dealer_upcard())?;
    }
    writeln!(output, "Available bankroll: {bankroll}")?;
    for (index, player_hand) in round.player_hands().iter().enumerate() {
        let active = if round.active_hand_index() == Some(index) {
            " *"
        } else {
            ""
        };
        let value = player_hand.hand().value();
        let softness = if value.is_soft { " soft" } else { "" };
        writeln!(
            output,
            "Hand {}{}: {} ({}{}) - {} - wager {}",
            index.saturating_add(1),
            active,
            cards_label(player_hand.hand().cards()),
            value.total,
            softness,
            hand_status_label(player_hand.status()),
            player_hand.wager()
        )?;
    }
    Ok(())
}

fn render_settlement<W: Write>(
    output: &mut W,
    settlement: &RoundSettlement,
    bankroll: Chips,
) -> Result<(), io::Error> {
    writeln!(output, "Round result:")?;
    for (index, result) in settlement.hand_results().iter().enumerate() {
        writeln!(
            output,
            "  Hand {}: {} (credit {})",
            index.saturating_add(1),
            outcome_label(result.outcome()),
            result.credit()
        )?;
    }
    if settlement.insurance_wager() != Chips::ZERO {
        writeln!(
            output,
            "  Insurance: wager {}, credit {}",
            settlement.insurance_wager(),
            settlement.insurance_credit()
        )?;
    }
    writeln!(output, "  Total credit: {}", settlement.total_credit())?;
    writeln!(output, "Bankroll after round: {bankroll}")?;
    Ok(())
}

fn prompt_value<R, W, T, F>(
    input: &mut R,
    output: &mut W,
    prompt: &str,
    parser: F,
) -> Result<Option<T>, io::Error>
where
    R: BufRead,
    W: Write,
    F: Fn(&str) -> Result<T, &'static str>,
{
    loop {
        let Some(line) = prompt_line(input, output, prompt)? else {
            return Ok(None);
        };
        match parser(&line) {
            Ok(value) => return Ok(Some(value)),
            Err(message) => writeln!(output, "{message}")?,
        }
    }
}

fn prompt_line<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    prompt: &str,
) -> Result<Option<String>, io::Error> {
    write!(output, "{prompt}")?;
    output.flush()?;
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(line.trim().to_owned()))
}

fn parse_bankroll(input: &str) -> Result<Chips, &'static str> {
    let bankroll = parse_chips(input)?;
    if bankroll == Chips::ZERO {
        return Err("Starting bankroll must be greater than zero.");
    }
    Ok(bankroll)
}

fn parse_chips(input: &str) -> Result<Chips, &'static str> {
    input
        .parse::<u64>()
        .map(Chips::new)
        .map_err(|_| "Enter a whole number of chips.")
}

fn parse_deck_count(input: &str) -> Result<u8, &'static str> {
    if input.is_empty() {
        return Ok(6);
    }
    match input.parse::<u8>() {
        Ok(count @ 1..=8) => Ok(count),
        _ => Err("Deck count must be between 1 and 8."),
    }
}

fn parse_soft_17(input: &str) -> Result<Soft17Rule, &'static str> {
    match input.to_ascii_lowercase().as_str() {
        "" | "s" | "stand" => Ok(Soft17Rule::Stand),
        "h" | "hit" => Ok(Soft17Rule::Hit),
        _ => Err("Enter stand or hit."),
    }
}

fn parse_payout(input: &str) -> Result<BlackjackPayout, &'static str> {
    match input.trim() {
        "" | "3:2" => Ok(BlackjackPayout::ThreeToTwo),
        "6:5" => Ok(BlackjackPayout::SixToFive),
        _ => Err("Enter 3:2 or 6:5."),
    }
}

fn parse_action(input: &str) -> Result<InputAction, &'static str> {
    match input.to_ascii_lowercase().as_str() {
        "h" | "hit" => Ok(InputAction::Play(PlayerAction::Hit)),
        "s" | "stand" => Ok(InputAction::Play(PlayerAction::Stand)),
        "d" | "double" => Ok(InputAction::Play(PlayerAction::Double)),
        "p" | "split" => Ok(InputAction::Play(PlayerAction::Split)),
        "q" | "quit" => Ok(InputAction::Quit),
        _ => Err("Unknown action. Enter one of the listed actions."),
    }
}

fn recoverable_round_input(error: SessionError) -> bool {
    matches!(
        error,
        SessionError::Round(
            RoundError::IllegalAction(_)
                | RoundError::InvalidInsurance { .. }
                | RoundError::Money(MoneyError::InsufficientFunds { .. })
        )
    )
}

const fn action_label(action: PlayerAction) -> &'static str {
    match action {
        PlayerAction::Hit => "hit",
        PlayerAction::Stand => "stand",
        PlayerAction::Double => "double",
        PlayerAction::Split => "split",
    }
}

const fn hand_status_label(status: HandStatus) -> &'static str {
    match status {
        HandStatus::Active => "active",
        HandStatus::Standing => "standing",
        HandStatus::Busted => "busted",
    }
}

const fn outcome_label(outcome: RoundOutcome) -> &'static str {
    match outcome {
        RoundOutcome::Bust => "bust",
        RoundOutcome::Loss => "loss",
        RoundOutcome::Push => "push",
        RoundOutcome::Win => "win",
        RoundOutcome::Blackjack => "blackjack",
    }
}

fn cards_label(cards: &[crate::card::Card]) -> String {
    cards
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug)]
pub enum CliError {
    Io(io::Error),
    Rule(RuleError),
    Session(SessionError),
    MissingRound,
    MissingSettlement,
    UnresolvedDealerTurn,
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "console I/O error: {error}"),
            Self::Rule(error) => write!(formatter, "invalid table rules: {error}"),
            Self::Session(error) => write!(formatter, "session error: {error}"),
            Self::MissingRound => formatter.write_str("session has no round to display"),
            Self::MissingSettlement => formatter.write_str("round completed without settlement"),
            Self::UnresolvedDealerTurn => {
                formatter.write_str("session returned before completing the dealer turn")
            }
        }
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Rule(error) => Some(error),
            Self::Session(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for CliError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<RuleError> for CliError {
    fn from(error: RuleError) -> Self {
        Self::Rule(error)
    }
}

impl From<SessionError> for CliError {
    fn from(error: SessionError) -> Self {
        Self::Session(error)
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use rand::{SeedableRng, rngs::StdRng};

    use super::{
        CliError, InputAction, parse_action, parse_deck_count, parse_payout, parse_soft_17, run,
    };
    use crate::{
        round::PlayerAction,
        rules::{BlackjackPayout, Soft17Rule},
        shoe::EntropySource,
    };

    #[test]
    fn action_parser_accepts_short_and_full_commands() {
        assert_eq!(parse_action("h"), Ok(InputAction::Play(PlayerAction::Hit)));
        assert_eq!(
            parse_action("hit"),
            Ok(InputAction::Play(PlayerAction::Hit))
        );
        assert_eq!(
            parse_action("s"),
            Ok(InputAction::Play(PlayerAction::Stand))
        );
        assert_eq!(
            parse_action("double"),
            Ok(InputAction::Play(PlayerAction::Double))
        );
        assert_eq!(
            parse_action("p"),
            Ok(InputAction::Play(PlayerAction::Split))
        );
        assert_eq!(parse_action("quit"), Ok(InputAction::Quit));
        assert!(parse_action("dance").is_err());
    }

    #[test]
    fn configuration_parsers_apply_documented_defaults() {
        assert_eq!(parse_deck_count(""), Ok(6));
        assert_eq!(parse_deck_count("8"), Ok(8));
        assert!(parse_deck_count("9").is_err());
        assert_eq!(parse_soft_17(""), Ok(Soft17Rule::Stand));
        assert_eq!(parse_soft_17("hit"), Ok(Soft17Rule::Hit));
        assert_eq!(parse_payout(""), Ok(BlackjackPayout::ThreeToTwo));
        assert_eq!(parse_payout("6:5"), Ok(BlackjackPayout::SixToFive));
    }

    #[test]
    fn setup_can_quit_before_starting_a_round() {
        let mut input = &b"100\n\n\n\nquit\n"[..];
        let mut output = Vec::new();
        let mut error = Vec::new();

        run(
            &mut input,
            &mut output,
            &mut error,
            StdRng::from_seed([21_u8; 32]),
            EntropySource::DevRandom,
        )
        .expect("clean quit");

        let output = String::from_utf8(output).expect("UTF-8 output");
        assert!(output.contains("Blackjack"));
        assert!(output.contains("Bankroll: 100 chips"));
        assert!(output.contains("Goodbye."));
        assert!(error.is_empty());
    }

    #[test]
    fn system_fallback_is_reported_on_standard_error() {
        let mut input = &b"100\n\n\n\nquit\n"[..];
        let mut output = Vec::new();
        let mut error = Vec::new();

        run(
            &mut input,
            &mut output,
            &mut error,
            StdRng::from_seed([22_u8; 32]),
            EntropySource::System,
        )
        .expect("clean quit");

        assert!(
            String::from_utf8(error)
                .expect("UTF-8 error")
                .contains("system RNG fallback")
        );
    }

    #[test]
    fn end_of_file_terminates_cleanly() {
        let mut input = io::empty();
        let mut output = Vec::new();
        let mut error = Vec::new();

        assert!(
            run(
                &mut input,
                &mut output,
                &mut error,
                StdRng::from_seed([23_u8; 32]),
                EntropySource::DevRandom,
            )
            .is_ok()
        );
    }

    #[test]
    fn output_failure_is_fatal() {
        struct FailingWriter;

        impl Write for FailingWriter {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
            }

            fn flush(&mut self) -> io::Result<()> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
            }
        }

        let mut input = io::empty();
        let mut output = FailingWriter;
        let mut error = Vec::new();
        let result = run(
            &mut input,
            &mut output,
            &mut error,
            StdRng::from_seed([24_u8; 32]),
            EntropySource::DevRandom,
        );

        assert!(matches!(result, Err(CliError::Io(_))));
    }
}
