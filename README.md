# blackjackrs

A single-player console blackjack game with configurable casino rules, exact
whole-chip accounting, and a cryptographically secure shuffled shoe.

## Package information

| Property | Value |
| --- | --- |
| Package | `blackjackrs` |
| Version | `0.1.0` |
| Type | Library and binary application |
| Rust edition | 2024 |
| Minimum Rust version | 1.85 |
| Runtime dependency | `rand` 0.10 |
| Published to crates.io | No |
| Entry point | `src/main.rs` |

## Requirements

- Rust 1.85 or newer with Cargo
- GNU Make or a compatible implementation for optional convenience targets

Install Rust using the [official Rust installation instructions][rust-install].

## Build and run

Build the optimized executable:

```console
cargo build --release
```

Run an interactive session:

```console
cargo run --release
```

The executable is written to `target/release/blackjackrs` on Unix-like systems
and `target/release/blackjackrs.exe` on Windows.

Run `make demo` to feed a complete sample round through the actual binary. Card
output varies because every round uses a newly shuffled shoe.

## Table configuration

Startup prompts for the following values:

| Setting | Allowed values | Default |
| --- | --- | --- |
| Starting bankroll | Positive whole chips | Required |
| Deck count | 1 through 8 | 6 |
| Dealer soft 17 | Stand or hit | Stand |
| Natural payout | 3:2 or 6:5 | 3:2 |

Exact payouts determine valid betting increments:

- A 3:2 table accepts wagers divisible by 2 chips.
- A 6:5 table accepts wagers divisible by 10 chips.
- Insurance accepts zero through half the original wager and pays 2:1 profit
  when the dealer has blackjack.

The bankroll persists in memory across rounds and is not saved after exit.

## Commands

| Action | Commands | Availability |
| --- | --- | --- |
| Hit | `h`, `hit` | Active player hand |
| Stand | `s`, `stand` | Active player hand |
| Double | `d`, `double` | Funded two-card hand |
| Split | `p`, `split` | Funded equal-value pair, up to four hands |
| Quit | `q`, `quit` | Between rounds |

The rules allow double after split, resplitting up to four hands, and Ace
resplitting. Split Aces receive one card per hand unless another Ace permits a
legal resplit. A post-split 21 is a regular win rather than a natural.

## Randomness

Startup reads exactly 32 seed bytes from `/dev/random`. If that open or read
fails, the application seeds `rand::rngs::StdRng` through `SysRng` and writes a
fallback warning to standard error. If both sources fail, startup exits with a
nonzero status.

`StdRng` is a CSPRNG, but its output sequence is not portable across crate
releases. Tests use ordered shoes instead of fixed random output.

## Verify

Run the full local gate:

```console
make verify
```

The target executes:

```console
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release
```

## Repository layout

| Path | Responsibility |
| --- | --- |
| `src/card.rs` | Suits, ranks, cards, and display values |
| `src/hand.rs` | Hand storage, totals, softness, naturals, and busts |
| `src/money.rs` | Checked whole-chip arithmetic |
| `src/rules.rs` | Validated table configuration and wager rules |
| `src/shoe.rs` | Deck construction, entropy acquisition, and shuffling |
| `src/round.rs` | Actions, phases, dealer behavior, and settlement |
| `src/session.rs` | Bankroll ownership and repeated-round orchestration |
| `src/cli.rs` | Console parsing, prompts, and rendering |
| `src/main.rs` | Entropy and standard-stream construction |
| `tests/console_smoke.rs` | Compiled-binary process test |

## Design documentation

- [Approved architecture and rules][design]
- [TDD implementation plan][implementation-plan]
- [`StdRng` documentation][rand-stdrng]
- [`SeedableRng` documentation][rand-seedable]
- [`rand` source repository][rand-repository]

[design]: docs/superpowers/specs/2026-08-11-console-blackjack-design.md
[implementation-plan]: docs/superpowers/plans/2026-08-11-console-blackjack.md
[rand-repository]: https://github.com/rust-random/rand
[rand-seedable]: https://docs.rs/rand/latest/rand/trait.SeedableRng.html
[rand-stdrng]: https://docs.rs/rand/latest/rand/rngs/struct.StdRng.html
[rust-install]: https://www.rust-lang.org/tools/install
