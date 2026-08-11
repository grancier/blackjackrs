# Console Blackjack Design

Status: Implemented

Date: 2026-08-11

Branch: `codex/console-blackjack`

## Goal

Replace the Hello World binary with a single-player, console-based blackjack
session. The player maintains a bankroll across rounds and plays against a
dealer under configurable table rules.

The implementation must keep blackjack rules independent from console I/O,
use exact chip arithmetic, and use a cryptographically secure RNG.

## Scope

### Included

- One human player against the dealer.
- Repeated rounds with a persistent in-memory bankroll.
- One through eight decks, configured at startup.
- Configurable stand-on-soft-17 or hit-on-soft-17 dealer behavior.
- Configurable 3:2 or 6:5 natural-blackjack payout.
- Hit, stand, double, split, resplit, and insurance.
- A maximum of four player hands after splitting.
- US hole-card and dealer-peek rules.
- A fresh shuffled shoe for each round.
- Exact whole-chip betting and payout arithmetic.

### Excluded

- Local or network multiplayer.
- Persistent bankrolls or saved sessions.
- Surrender.
- Side bets other than insurance.
- Card counting, cut-card penetration, and mid-shoe reshuffling.
- Alternate user interfaces or a command/event extension framework.

These exclusions are rejected as outside the requested branch scope, not
deferred backlog items.

## Table configuration

Startup collects and validates the following values:

| Setting | Allowed values | Default |
| --- | --- | --- |
| Starting bankroll | Positive whole chips | No implicit default |
| Deck count | 1 through 8 | 6 |
| Dealer soft 17 | Stand or hit | Stand |
| Natural payout | 3:2 or 6:5 | 3:2 |

The table supports at most four player hands after splitting.

## Betting and payouts

`Chips` is a private-field `u64` newtype. All additions and
multiplications use checked arithmetic.

A wager must be positive, no greater than the available bankroll, and
compatible with exact insurance and blackjack payouts:

- A 3:2 table accepts wagers divisible by 2 chips.
- A 6:5 table accepts wagers divisible by 10 chips.

The wager is reserved when a round begins. A split or double reserves an
additional wager before the action changes game state.

Settlement credits are:

| Outcome | Credit after stake was reserved |
| --- | --- |
| Loss or bust | 0 |
| Push | Original wager |
| Regular win | Original wager plus 1:1 profit |
| Natural at 3:2 | Original wager plus 3:2 profit |
| Natural at 6:5 | Original wager plus 6:5 profit |
| Winning insurance | Insurance stake plus 2:1 profit |

Each split hand settles independently. A post-split 21 is a regular win, not
a natural blackjack.

## Deal and player actions

The dealer uses the US hole-card model. The player and dealer receive two
cards, with one dealer card hidden.

Insurance is offered only when the dealer upcard is an Ace. The player may
wager from zero through half the original wager. The dealer then peeks for a
natural. With a ten-value upcard, the dealer peeks before player actions
without offering insurance.

The legal actions are:

- `Hit`: draw one card unless the hand is complete.
- `Stand`: complete the active hand.
- `Double`: available on any two-card hand when the bankroll covers the
  additional wager. Draw exactly one card, then complete the hand.
- `Split`: available when two cards have equal point values, the bankroll
  covers the matching wager, and fewer than four hands exist.

Ten, Jack, Queen, and King share a point value and may split with one another.
Resplitting is allowed up to four total hands. Doubling after a split is
allowed.

Split Aces receive one card per hand and then stand. If the new card is
another Ace, the hand may resplit while the four-hand limit permits it.

## Dealer behavior

After all non-busted player hands complete, the dealer reveals the hole card.
The dealer:

- Hits below 17.
- Stands above 17.
- At soft 17, follows the configured table rule.
- Stands on hard 17.

The dealer does not draw when every player hand has already busted.

## Architecture

The package becomes a library and a thin executable:

```text
src/
├── lib.rs
├── card.rs
├── hand.rs
├── money.rs
├── rules.rs
├── shoe.rs
├── round.rs
├── session.rs
├── cli.rs
└── main.rs
```

Module responsibilities are:

| Module | Responsibility |
| --- | --- |
| `card` | `Suit`, `Rank`, `Card`, and display values |
| `hand` | Hand storage, totals, softness, naturals, busts, and split eligibility |
| `money` | `Chips`, wager validation, and exact settlement arithmetic |
| `rules` | Validated table configuration and rule enums |
| `shoe` | Deck construction, entropy acquisition, and shuffling |
| `round` | Round phases, legal actions, dealer behavior, and outcomes |
| `session` | Bankroll ownership and repeated-round orchestration |
| `cli` | Input parsing, prompts, rendering, and recoverable user errors |
| `main` | Dependency construction and fatal process error reporting |

Dependencies remain one-way:

```text
card -> hand
card -> shoe
hand + money + rules + shoe -> round
round + money + rules + shoe -> session
session -> cli
cli -> main
```

`Round` is the authority for legal actions and phase transitions.
`Session` is the authority for bankroll changes. The CLI cannot manipulate
cards, wagers, or phases directly.

No repository, service, strategy, or event-bus traits are introduced. The
current requirements have only one implementation of each concern.

### Implementation boundaries

`Round` exposes read-only state to callers. Its action, insurance, dealer-play,
and settlement-consumption methods are crate-private and are called by
`Session`. This makes `Session` the only production path that can reserve a
wager or credit a settlement.

The console accepts buffered readers and writers. Unit tests exercise parsing
and I/O failures without process globals, while `main.rs` only constructs
entropy and locked standard streams and maps fatal errors to a nonzero exit.

## Domain model

The design uses enums and newtypes instead of stringly typed values and
behavioral boolean flags:

- `Suit`, `Rank`, and `Card`.
- `Hand` and `HandValue { total, is_soft }`.
- `Chips` with a private field and checked operations.
- `BlackjackPayout::{ThreeToTwo, SixToFive}`.
- `Soft17Rule::{Stand, Hit}`.
- `TableRules` with validated construction.
- `PlayerAction::{Hit, Stand, Double, Split}`.
- `RoundPhase`, `HandStatus`, and `RoundOutcome`.

There is one hand evaluator, one legal-action calculation path, and one
settlement calculation path.

## Randomness

The project depends on `rand` 0.10. The [official `StdRng`
documentation][rand-stdrng] defines it as a CSPRNG. `StdRng` is the single
game RNG, and construction uses [the `SeedableRng` API][rand-seedable].

At startup:

1. Open `/dev/random`.
2. Read exactly 32 seed bytes.
3. Construct `StdRng` with `SeedableRng::from_seed`.
4. If opening or reading `/dev/random` fails, seed `StdRng` through
   `SysRng`.
5. If both entropy sources fail, return a fatal typed error.

The fallback is written to stderr so the entropy source is visible. The
implementation uses the crate's sequence shuffle API and does not implement
a custom shuffle.

`StdRng` is treated as a CSPRNG but not as a reproducible algorithm across
crate releases. Tests use explicitly ordered shoes rather than relying on its
output sequence.

## State flow

```text
SessionSetup
  -> AwaitingBet
  -> InsuranceOffer or DealerPeek
  -> PlayerTurns
  -> DealerTurn
  -> Settled
  -> AwaitingBet
```

Typed phases reject actions that are invalid for the current state. Invalid
console input does not mutate game state.

## Console behavior

Startup prompts collect the bankroll and table configuration. Each round
shows:

- Current bankroll and wagers.
- Dealer upcard and hidden-card status.
- Every player hand, total, status, and active-hand marker.
- Only the actions legal for the active hand.

Commands accept concise and full forms such as `h` and `hit`. Invalid
input prints a specific constraint and reprompts. Quitting is allowed between
rounds, and end-of-file terminates cleanly.

`main.rs` constructs the RNG and CLI dependencies. It maps fatal errors to
a message on stderr and a nonzero exit status.

## Error handling

Production code avoids `unwrap`, `expect`, unchecked arithmetic, lossy
casts, unsafe code, and global mutable state.

Typed errors distinguish:

- Invalid table rules.
- Invalid or unaffordable wagers.
- Illegal player actions.
- Exhausted shoes.
- Arithmetic overflow.
- Entropy-source failure.
- Fatal input or output failure.

Console parse failures are recoverable. Internal invariants remain enforced
inside domain constructors and state transitions.

## Testing

Development follows red, green, refactor for each behavior group.

Unit and integration coverage includes:

- Ace totals, soft hands, naturals, and busts.
- Wager divisibility and checked payout arithmetic.
- Table-rule validation.
- Deck counts, card multiplicity, and shuffle integration.
- Hit, stand, double, split, resplit, and split-Ace transitions.
- Insurance and dealer peek behavior.
- Dealer hard and soft 17 behavior.
- Bust, loss, push, regular win, and both natural payouts.
- Bankroll conservation across every outcome.
- Console prompts, aliases, invalid-input recovery, and quitting.

Ordered shoes make round tests deterministic without mocking the RNG. The
process-level smoke test drives a complete round without asserting specific
random cards.

The completion gate runs:

```console
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release
make demo
```

Mutation checks must prove that tests detect incorrect card values, Ace
handling, payouts, dealer soft-17 behavior, and action legality.

## Acceptance criteria

- The console can configure and run repeated single-player rounds.
- Every requested action and settlement rule is executable through the CLI.
- Bankroll arithmetic remains exact and checked.
- Invalid commands and wagers reprompt without corrupting state.
- RNG seeding follows the required primary and fallback entropy paths.
- Domain logic is usable and testable without console I/O.
- Production code contains no unsafe blocks or panic-based error handling.
- All completion-gate commands pass with no warnings.

## Scope guard

The branch uses all three major-feature slots:

| Component | Worthiness | Decision |
| --- | ---: | --- |
| Blackjack rules engine | 1.18 | Discussed and approved |
| Bankroll session and CLI | 1.64 | Discussed and approved |
| Secure shoe and RNG | 4.43 | Implement now |

No backlog file exists. At design approval, implementation work had zero
changed lines, zero new files, and zero commits relative to `main`.

At design approval, the implementation target was below 1,500 production
lines. The 1,000-line scope review confirmed that the branch still contained
only the three approved components. No fourth major feature entered the
branch.

## Alternatives rejected

A single-module state machine was rejected because it would couple console
I/O, betting, round transitions, and settlement.

A command/event architecture was rejected because replay, alternate UIs, and
multiplayer are not requirements. Those abstractions would add indirection
without a third concrete use case.

[rand-stdrng]: https://docs.rs/rand/latest/rand/rngs/struct.StdRng.html
[rand-seedable]: https://docs.rs/rand/latest/rand/trait.SeedableRng.html
