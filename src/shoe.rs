use std::{
    error::Error,
    fmt,
    fs::File,
    io::{self, Read},
};

use rand::{
    SeedableRng,
    rngs::{StdRng, SysRng},
    seq::SliceRandom,
};

use crate::card::{Card, Rank, Suit};

const SUITS: [Suit; 4] = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];
const RANKS: [Rank; 13] = [
    Rank::Ace,
    Rank::Two,
    Rank::Three,
    Rank::Four,
    Rank::Five,
    Rank::Six,
    Rank::Seven,
    Rank::Eight,
    Rank::Nine,
    Rank::Ten,
    Rank::Jack,
    Rank::Queen,
    Rank::King,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntropySource {
    DevRandom,
    System,
}

#[derive(Debug)]
pub enum EntropyError {
    Unavailable {
        primary: io::Error,
        fallback: String,
    },
}

impl fmt::Display for EntropyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { primary, fallback } => write!(
                formatter,
                "no secure entropy source available: /dev/random: {primary}; system RNG: {fallback}"
            ),
        }
    }
}

impl Error for EntropyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unavailable { primary, .. } => Some(primary),
        }
    }
}

pub fn seeded_rng() -> Result<(StdRng, EntropySource), EntropyError> {
    seeded_rng_from(File::open("/dev/random"), || {
        StdRng::try_from_rng(&mut SysRng)
    })
}

fn seeded_rng_from<R, F, E>(
    primary: io::Result<R>,
    fallback: F,
) -> Result<(StdRng, EntropySource), EntropyError>
where
    R: Read,
    F: FnOnce() -> Result<StdRng, E>,
    E: fmt::Display,
{
    let primary_error = match primary {
        Ok(mut reader) => {
            let mut seed = [0_u8; 32];
            match reader.read_exact(&mut seed) {
                Ok(()) => return Ok((StdRng::from_seed(seed), EntropySource::DevRandom)),
                Err(error) => error,
            }
        }
        Err(error) => error,
    };

    fallback()
        .map(|rng| (rng, EntropySource::System))
        .map_err(|error| EntropyError::Unavailable {
            primary: primary_error,
            fallback: error.to_string(),
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shoe {
    cards: Vec<Card>,
}

impl Shoe {
    pub fn shuffled(deck_count: u8, rng: &mut StdRng) -> Result<Self, ShoeError> {
        if !(1..=8).contains(&deck_count) {
            return Err(ShoeError::InvalidDeckCount(deck_count));
        }

        let mut cards = Vec::with_capacity(usize::from(deck_count) * 52);
        for _ in 0..deck_count {
            for suit in SUITS {
                for rank in RANKS {
                    cards.push(Card::new(rank, suit));
                }
            }
        }
        cards.shuffle(rng);

        Ok(Self { cards })
    }

    #[must_use]
    pub fn ordered(cards_in_draw_order: impl IntoIterator<Item = Card>) -> Self {
        let mut cards: Vec<_> = cards_in_draw_order.into_iter().collect();
        cards.reverse();
        Self { cards }
    }

    pub fn draw(&mut self) -> Result<Card, ShoeError> {
        self.cards.pop().ok_or(ShoeError::Exhausted)
    }

    pub fn draw_many(&mut self, count: usize) -> Result<Vec<Card>, ShoeError> {
        if count > self.cards.len() {
            return Err(ShoeError::InsufficientCards {
                requested: count,
                remaining: self.cards.len(),
            });
        }

        (0..count).map(|_| self.draw()).collect()
    }

    #[must_use]
    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.cards.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShoeError {
    InvalidDeckCount(u8),
    Exhausted,
    InsufficientCards { requested: usize, remaining: usize },
}

impl fmt::Display for ShoeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDeckCount(count) => {
                write!(formatter, "deck count must be between 1 and 8, got {count}")
            }
            Self::Exhausted => formatter.write_str("the shoe is exhausted"),
            Self::InsufficientCards {
                requested,
                remaining,
            } => write!(
                formatter,
                "cannot draw {requested} cards from a shoe with {remaining} remaining"
            ),
        }
    }
}

impl Error for ShoeError {}

#[cfg(test)]
mod tests {
    use std::io;

    use rand::{Rng, SeedableRng, rngs::StdRng};

    use super::{EntropyError, EntropySource, Shoe, ShoeError, seeded_rng_from};
    use crate::card::{Card, Rank, Suit};

    fn card(rank: Rank) -> Card {
        Card::new(rank, Suit::Spades)
    }

    #[test]
    fn shuffled_shoe_contains_every_card_for_each_deck() {
        let mut rng = StdRng::from_seed([3_u8; 32]);
        let shoe = Shoe::shuffled(2, &mut rng).expect("valid deck count");

        assert_eq!(shoe.len(), 104);
        for rank in [
            Rank::Ace,
            Rank::Two,
            Rank::Three,
            Rank::Four,
            Rank::Five,
            Rank::Six,
            Rank::Seven,
            Rank::Eight,
            Rank::Nine,
            Rank::Ten,
            Rank::Jack,
            Rank::Queen,
            Rank::King,
        ] {
            assert_eq!(
                shoe.cards()
                    .iter()
                    .filter(|card| card.rank() == rank)
                    .count(),
                8
            );
        }
    }

    #[test]
    fn shuffled_shoe_rejects_deck_counts_outside_table_limits() {
        let mut rng = StdRng::from_seed([4_u8; 32]);

        assert_eq!(
            Shoe::shuffled(0, &mut rng),
            Err(ShoeError::InvalidDeckCount(0))
        );
        assert_eq!(
            Shoe::shuffled(9, &mut rng),
            Err(ShoeError::InvalidDeckCount(9))
        );
    }

    #[test]
    fn ordered_shoe_draws_cards_in_provided_order() {
        let mut shoe = Shoe::ordered([card(Rank::Two), card(Rank::King)]);

        assert_eq!(shoe.draw(), Ok(card(Rank::Two)));
        assert_eq!(shoe.draw(), Ok(card(Rank::King)));
        assert!(shoe.is_empty());
        assert_eq!(shoe.draw(), Err(ShoeError::Exhausted));
    }

    #[test]
    fn multi_card_draw_is_atomic_when_shoe_is_too_short() {
        let mut shoe = Shoe::ordered([card(Rank::Ace)]);

        assert_eq!(
            shoe.draw_many(2),
            Err(ShoeError::InsufficientCards {
                requested: 2,
                remaining: 1,
            })
        );
        assert_eq!(shoe.len(), 1);
    }

    #[test]
    fn complete_primary_seed_uses_dev_random_source() {
        let (mut first, source) =
            seeded_rng_from(Ok(&[7_u8; 32][..]), || -> Result<StdRng, &'static str> {
                Err("fallback must not run")
            })
            .expect("primary seed is complete");
        let (mut second, _) =
            seeded_rng_from(Ok(&[7_u8; 32][..]), || -> Result<StdRng, &'static str> {
                Err("fallback must not run")
            })
            .expect("primary seed is complete");

        assert_eq!(source, EntropySource::DevRandom);
        assert_eq!(first.next_u64(), second.next_u64());
    }

    #[test]
    fn short_primary_read_uses_system_fallback() {
        let mut fallback_called = false;
        let (_, source) = seeded_rng_from(Ok(&[7_u8; 8][..]), || {
            fallback_called = true;
            Ok::<StdRng, &'static str>(StdRng::from_seed([9_u8; 32]))
        })
        .expect("fallback succeeds");

        assert!(fallback_called);
        assert_eq!(source, EntropySource::System);
    }

    #[test]
    fn both_entropy_failures_return_context() {
        let result = seeded_rng_from::<io::Empty, _, _>(
            Err(io::Error::new(io::ErrorKind::NotFound, "missing primary")),
            || Err::<StdRng, _>("missing fallback"),
        );

        assert!(matches!(
            result,
            Err(EntropyError::Unavailable {
                primary,
                fallback,
            }) if primary.kind() == io::ErrorKind::NotFound && fallback == "missing fallback"
        ));
    }
}
