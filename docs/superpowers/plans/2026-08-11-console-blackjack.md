# Console Blackjack Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Hello World binary with a production-quality, single-player console blackjack session implementing the approved table rules, exact bankroll accounting, and secure shuffling.

**Architecture:** Keep cards, hands, money, rules, shoes, rounds, sessions, and console I/O in one-way modules. `Round` owns game legality and transitions; `Session` owns the bankroll and ensures settlement is credited once; `cli` only parses and renders. Use concrete types rather than speculative traits, with deterministic ordered shoes at domain-test boundaries.

**Tech Stack:** Rust 2024 (MSRV 1.85), Cargo, `rand` 0.10, standard-library buffered I/O, Rust unit and integration tests, Clippy, rustfmt, GNU Make-compatible targets.

## Global Constraints

- Follow the iron law for every behavior: add one failing test, capture the
  failure, add the minimum implementation, pass the focused test, then
  refactor while green.
- Keep production code free of `unsafe`, `unwrap`, `expect`, unchecked money
  arithmetic, lossy casts, global mutable state, and panic-based error paths.
- Use `Chips(u64)` for all stakes and credits; never use floating point.
- Use one `StdRng` per session. Seed from 32 bytes read from `/dev/random`, with
  `SysRng` only as the fallback.
- Construct a fresh shuffled shoe for every round. Tests use ordered shoes and
  never assert a particular `StdRng` sequence.
- Do not add surrender, persistence, multiplayer, side bets, card-counting
  features, alternate UIs, repositories, services, event buses, or strategy
  traits.
- Keep each implementation commit focused and leave
  `cargo fmt --all -- --check`, Clippy, and the relevant tests green.
- Treat a branch diff above 1,000 lines as a scope-review trigger and 1,500
  implementation lines as a mandatory split point.

## Fixed Interfaces

The implementation tasks use these concrete interfaces. Visibility may be
narrowed when a type is only needed within its parent module, but behavior and
ownership must not change without updating the approved design.

```rust
pub enum Suit { Clubs, Diamonds, Hearts, Spades }
pub enum Rank { Ace, Two, Three, Four, Five, Six, Seven, Eight, Nine, Ten, Jack, Queen, King }
pub struct Card { rank: Rank, suit: Suit }

pub struct Hand { cards: Vec<Card> }
pub struct HandValue { pub total: u16, pub is_soft: bool }

pub struct Chips(u64);
pub enum Soft17Rule { Stand, Hit }
pub enum BlackjackPayout { ThreeToTwo, SixToFive }
pub struct TableRules {
    deck_count: u8,
    soft_17: Soft17Rule,
    blackjack_payout: BlackjackPayout,
}

pub enum EntropySource { DevRandom, System }
pub struct Shoe { cards: Vec<Card> }

pub enum PlayerAction { Hit, Stand, Double, Split }
pub enum RoundPhase { InsuranceOffer, PlayerTurns, DealerTurn, Settled }
pub enum HandStatus { Active, Standing, Busted }
pub enum RoundOutcome { Bust, Loss, Push, Win, Blackjack }
```

`Round`, `RoundSettlement`, and `Session` are opaque public structs whose
private storage is defined in Tasks 4, 6, and 7 as behavior emerges from tests.
`Round` reserves additional split, double, and insurance stakes through
checked calls supplied by `Session`. A round settlement is consumed when
credited, preventing duplicate bankroll credits.

---

## Task 1: Card and Hand Domain

**Files:**

- Create: `src/lib.rs`
- Create: `src/card.rs`
- Create: `src/hand.rs`
- Test: unit tests in `src/card.rs` and `src/hand.rs`

- [ ] **Red — define card-value and hand-evaluation behavior in tests**

  Add tests for rank values, display, hard totals, Ace demotion, soft totals,
  multiple Aces, natural detection, busts, and equal-value splitting. Start
  with this evaluator-driving test:

  ```rust
  #[test]
  fn three_aces_and_an_eight_total_twenty_one_soft() {
      let hand = Hand::from_cards([
          card(Rank::Ace),
          card(Rank::Ace),
          card(Rank::Ace),
          card(Rank::Eight),
      ]);

      assert_eq!(hand.value(), HandValue { total: 21, is_soft: true });
  }
  ```

  Run `cargo test hand::tests -- --nocapture` and capture the compile failure
  proving the module does not exist.

- [ ] **Green — implement immutable card metadata and the single hand evaluator**

  Implement `Suit`, `Rank`, and `Card` as `Copy` value types with constructors,
  accessors, blackjack point values, and `Display`. Implement `Hand` with:

  ```rust
  pub fn new() -> Self;
  pub fn from_cards(cards: impl IntoIterator<Item = Card>) -> Self;
  pub fn push(&mut self, card: Card);
  pub fn cards(&self) -> &[Card];
  pub fn value(&self) -> HandValue;
  pub fn is_bust(&self) -> bool;
  pub fn is_two_card_twenty_one(&self) -> bool;
  pub fn split_value(&self) -> Option<u8>;
  ```

  Count every Ace as 1, then promote at most one Ace by 10 when the total stays
  at or below 21. Splitting compares blackjack point values, allowing unlike
  ten-value ranks to split.

- [ ] **Refactor and verify**

  Run `cargo fmt --all`, `cargo test card::tests`, `cargo test hand::tests`, and
  `cargo clippy --all-targets --all-features -- -D warnings`.

- [ ] **Commit**

  `git commit -m "feat: add card and hand domain"`

## Task 2: Exact Money and Validated Table Rules

**Files:**

- Create: `src/money.rs`
- Create: `src/rules.rs`
- Modify: `src/lib.rs`
- Test: unit tests in `src/money.rs` and `src/rules.rs`

- [ ] **Red — specify exact arithmetic and rule validation**

  Test zero wagers, insufficient funds, overflow, inexact ratios, deck counts
  outside 1–8, and payout-specific wager increments. Include:

  ```rust
  #[test]
  fn six_to_five_requires_ten_chip_wager_increments() {
      let rules = TableRules::new(
          6,
          Soft17Rule::Stand,
          BlackjackPayout::SixToFive,
      ).unwrap();

      assert_eq!(rules.validate_wager(Chips::new(20), Chips::new(100)), Ok(()));
      assert_eq!(
          rules.validate_wager(Chips::new(12), Chips::new(100)),
          Err(WagerError::InvalidIncrement { required: Chips::new(10) }),
      );
  }
  ```

  Run `cargo test rules::tests -- --nocapture` and retain the failing result.

- [ ] **Green — implement checked chip primitives and payout ratios**

  Give `Chips` `ZERO`, `new`, `value`, `checked_add`, `checked_sub`,
  `checked_mul`, and `checked_mul_ratio`. `checked_mul_ratio(numerator,
  denominator)` must reject denominator zero, overflow, and nonintegral results.
  Add typed `MoneyError`, `RuleError`, and `WagerError` values with `Display` and
  `Error` implementations.

  `TableRules` exposes deck count, soft-17 behavior, blackjack payout, maximum
  hands of four, payout numerator/denominator, wager increment, and a single
  `validate_wager` path.

- [ ] **Refactor and verify**

  Run `cargo fmt --all`, `cargo test money::tests`, `cargo test rules::tests`,
  and Clippy with warnings denied.

- [ ] **Commit**

  `git commit -m "feat: add exact betting rules"`

## Task 3: Secure Entropy and Fresh Shoes

**Files:**

- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `src/shoe.rs`
- Modify: `src/lib.rs`
- Test: unit tests in `src/shoe.rs`

- [ ] **Red — specify deck composition, atomic draws, and both entropy paths**

  Add `rand = "0.10"`. Test 52 cards per deck, four copies of each rank per
  deck, invalid deck counts, first-provided-is-first-drawn ordered shoes,
  all-or-nothing
  multi-card draws, exact `/dev/random` seed use, fallback invocation after a
  short or failed read, and typed failure if both sources fail.

  Drive entropy through a private helper accepting `Read` plus a fallback
  closure, so tests force both branches without replacing the production RNG:

  ```rust
  #[test]
  fn short_primary_read_uses_system_fallback() {
      let mut fallback_called = false;
      let result = seeded_rng_from(
          &mut &[7_u8; 8][..],
          || {
              fallback_called = true;
              Ok(StdRng::from_seed([9_u8; 32]))
          },
      );

      assert!(result.is_ok());
      assert!(fallback_called);
  }
  ```

  Run `cargo test shoe::tests -- --nocapture` and capture red.

- [ ] **Green — implement one standard-library-backed CSPRNG construction path**

  Implement:

  ```rust
  pub fn seeded_rng() -> Result<(StdRng, EntropySource), EntropyError>;
  pub fn shuffled(deck_count: u8, rng: &mut StdRng) -> Result<Shoe, ShoeError>;
  pub fn ordered(cards_in_draw_order: impl IntoIterator<Item = Card>) -> Shoe;
  pub fn draw(&mut self) -> Result<Card, ShoeError>;
  pub fn draw_many(&mut self, count: usize) -> Result<Vec<Card>, ShoeError>;
  pub fn len(&self) -> usize;
  pub fn is_empty(&self) -> bool;
  ```

  Production `seeded_rng` opens `/dev/random`, uses `read_exact` for a 32-byte
  seed and `StdRng::from_seed`, then falls back to
  `StdRng::try_from_rng(&mut SysRng)`. `draw_many` checks capacity before
  mutation. Use `SliceRandom::shuffle`; do not implement shuffling manually.

- [ ] **Refactor and verify**

  Run `cargo fmt --all`, `cargo test shoe::tests`, full Clippy, and
  `cargo tree -d` to inspect the dependency graph.

- [ ] **Commit**

  `git commit -m "feat: add secure shuffled shoes"`

## Task 4: Round Deal, Peek, Hit, Stand, and Double

**Files:**

- Create: `src/round.rs`
- Modify: `src/lib.rs`
- Test: unit tests in `src/round.rs`

- [ ] **Red — drive initial round phases and core actions with ordered shoes**

  Test standard deal order (player, dealer upcard, player, dealer hole), dealer
  ten-value peek, immediate natural settlement, legal-action calculation, hit,
  stand, double affordability, exactly one double card, automatic stand after
  double, and no mutation on an illegal action or exhausted shoe.

  ```rust
  #[test]
  fn double_reserves_one_wager_draws_once_and_stands() {
      let mut bankroll = Chips::new(90);
      let mut round = round_with_player_cards(Rank::Five, Rank::Six);
      let mut shoe = Shoe::ordered([card(Rank::King)]);

      round.act(PlayerAction::Double, &mut bankroll, &mut shoe).unwrap();

      assert_eq!(bankroll, Chips::new(80));
      assert_eq!(round.player_hands()[0].wager(), Chips::new(20));
      assert_eq!(round.player_hands()[0].hand().value().total, 21);
      assert_eq!(round.player_hands()[0].status(), HandStatus::Standing);
  }
  ```

  Run the focused test and capture red.

- [ ] **Green — implement the smallest phase-safe round state machine**

  Add typed `RoundError`, `PlayerAction`, `RoundPhase`, `HandStatus`, an internal
  `HandOrigin`, and read-only `PlayerHand` accessors. Implement:

  ```rust
  pub fn deal(wager: Chips, rules: TableRules, shoe: &mut Shoe) -> Result<Round, RoundError>;
  pub fn phase(&self) -> RoundPhase;
  pub fn dealer_upcard(&self) -> Card;
  pub fn player_hands(&self) -> &[PlayerHand];
  pub fn active_hand_index(&self) -> Option<usize>;
  pub fn legal_actions(&self, bankroll: Chips) -> Vec<PlayerAction>;
  pub(crate) fn act(&mut self, action: PlayerAction, bankroll: &mut Chips, shoe: &mut Shoe) -> Result<(), RoundError>;
  ```

  Preflight funds, phases, legality, and required card count before changing
  bankroll or round state. Advance automatically past stood and busted hands.

- [ ] **Refactor and verify**

  Extract private phase-advance helpers only where they remove duplicated state
  transitions. Run focused round tests, all unit tests, rustfmt, and Clippy.

- [ ] **Commit**

  `git commit -m "feat: add core blackjack round actions"`

## Task 5: Split, Resplit, and Split Aces

**Files:**

- Modify: `src/round.rs`
- Test: unit tests in `src/round.rs`

- [ ] **Red — specify split transaction and Ace restrictions**

  Test matching values across ten-value ranks, nonmatching rejection, bankroll
  reservation, two replacement cards, independent hand order, double after
  split, resplitting up to four hands, rejection at the limit, one-card split
  Aces, resplit Aces when the added card is an Ace, and post-split 21 remaining
  non-natural.

  ```rust
  #[test]
  fn split_aces_take_one_card_each_and_stand() {
      let mut round = round_with_player_cards(Rank::Ace, Rank::Ace);
      let mut bankroll = Chips::new(90);
      let mut shoe = Shoe::ordered([card(Rank::Nine), card(Rank::King)]);

      round.act(PlayerAction::Split, &mut bankroll, &mut shoe).unwrap();

      assert_eq!(round.player_hands().len(), 2);
      assert!(round.player_hands().iter().all(|hand| {
          hand.hand().cards().len() == 2 && hand.status() == HandStatus::Standing
      }));
  }
  ```

  Run each new split test before implementation and capture red.

- [ ] **Green — add split origin and deterministic insertion rules**

  Split the active pair in place, insert its sibling immediately after it,
  reserve exactly the original hand wager, and deal one replacement to each
  hand. Mark all descendants as split-origin. For split Aces, stand after one
  card unless an Ace pair can legally resplit; expose only `Split` in that
  exceptional state.

- [ ] **Refactor and verify**

  Keep all action legality in `legal_actions`; `act` must consume that same
  decision rather than duplicate predicates. Run round tests, all tests,
  rustfmt, and Clippy.

- [ ] **Commit**

  `git commit -m "feat: add blackjack hand splitting"`

## Task 6: Insurance, Dealer Play, and Settlement

**Files:**

- Modify: `src/round.rs`
- Test: unit tests in `src/round.rs`

- [ ] **Red — specify insurance, dealer rules, and every outcome**

  Test insurance offered only for an Ace, zero-to-half validation, reservation,
  2:1 profit, loss, dealer natural ending the round after peek, hard 17 stand,
  S17 stand, H17 hit, dealer bust, no dealer draw after all player busts, and
  bust/loss/push/win/3:2 natural/6:5 natural credits.

  Include bankroll conservation assertions and this behavioral split:

  ```rust
  #[test]
  fn dealer_soft_seventeen_follows_table_configuration() {
      let standing = settle_dealer([Rank::Ace, Rank::Six], Soft17Rule::Stand);
      let hitting = settle_dealer([Rank::Ace, Rank::Six], Soft17Rule::Hit);

      assert_eq!(standing.dealer_card_count(), 2);
      assert!(hitting.dealer_card_count() > 2);
  }
  ```

  Run focused tests and capture red for every outcome group.

- [ ] **Green — implement one dealer loop and one settlement calculation path**

  Implement:

  ```rust
  pub(crate) fn place_insurance(&mut self, amount: Chips, bankroll: &mut Chips) -> Result<(), RoundError>;
  pub(crate) fn play_dealer(&mut self, shoe: &mut Shoe) -> Result<(), RoundError>;
  pub fn settlement(&self) -> Option<&RoundSettlement>;
  pub(crate) fn take_settlement(&mut self) -> Option<RoundSettlement>;
  ```

  `RoundSettlement` carries per-hand `RoundOutcome`, per-hand credits,
  insurance credit, and checked total credit. Classify natural blackjack only
  for the initial unsplit two-card hand. Skip dealer draws when all player hands
  bust. Store settlement once and allow it to be taken once.

- [ ] **Refactor and verify**

  Centralize checked payout ratios in `money`/`rules`, not `round`. Run all
  round and money tests, rustfmt, full Clippy, and all tests.

- [ ] **Commit**

  `git commit -m "feat: settle blackjack rounds"`

## Task 7: Session Bankroll and Round Orchestration

**Files:**

- Create: `src/session.rs`
- Modify: `src/lib.rs`
- Test: unit tests in `src/session.rs`

- [ ] **Red — specify ownership, fresh shoes, and exactly-once crediting**

  Test starting-wager reservation, unaffordable/invalid bet rejection without
  mutation, insurance and action delegation, bankroll persistence, automatic
  settlement credit, refusal to start overlapping rounds, fresh shoe creation
  per round, no duplicate credit after repeated observation, and game-over when
  no valid minimum wager is affordable.

  ```rust
  #[test]
  fn completed_round_is_credited_exactly_once() {
      let mut session = deterministic_winning_session(Chips::new(100));

      session.start_round(Chips::new(10)).unwrap();
      session.act(PlayerAction::Stand).unwrap();
      let after_win = session.bankroll();
      session.finish_round().unwrap();

      assert_eq!(session.bankroll(), after_win);
  }
  ```

  Run `cargo test session::tests -- --nocapture` and capture red.

- [ ] **Green — make Session the sole bankroll mutation boundary**

  Implement a production constructor accepting rules, bankroll, and `StdRng`,
  plus a crate-private ordered-shoe constructor for tests. Provide:

  ```rust
  pub fn bankroll(&self) -> Chips;
  pub fn rules(&self) -> TableRules;
  pub fn round(&self) -> Option<&Round>;
  pub fn can_place_minimum_wager(&self) -> bool;
  pub fn start_round(&mut self, wager: Chips) -> Result<(), SessionError>;
  pub fn place_insurance(&mut self, amount: Chips) -> Result<(), SessionError>;
  pub fn act(&mut self, action: PlayerAction) -> Result<(), SessionError>;
  pub fn finish_round(&mut self) -> Result<Option<RoundSettlement>, SessionError>;
  ```

  Starting a round constructs a fresh shoe, atomically reserves the wager, and
  deals. After every operation, settle and credit when the round becomes
  complete. `finish_round` is idempotent after the settlement has been removed.

- [ ] **Refactor and verify**

  Ensure `Session` contains no CLI strings and `Round` cannot publicly mutate a
  bankroll. Run session tests, all tests, rustfmt, and Clippy.

- [ ] **Commit**

  `git commit -m "feat: add persistent blackjack sessions"`

## Task 8: Console Adapter and Process Entry Point

**Files:**

- Create: `src/cli.rs`
- Replace: `src/main.rs`
- Modify: `src/lib.rs`
- Test: unit tests in `src/cli.rs`

- [ ] **Red — drive prompts, parsing, rendering, recovery, and termination**

  Test required positive bankroll, defaults, all configuration choices, short
  and full action aliases, insurance input, invalid input reprompt without
  mutation, legal-actions-only rendering, dealer hole-card hiding, settlement
  output, quit between rounds, rejection of quit mid-round, EOF termination,
  output failure, and startup fallback notice on stderr.

  ```rust
  #[test]
  fn invalid_action_reprompts_without_mutating_the_round() {
      let input = b"dance\nhit\n";
      let mut output = Vec::new();
      let mut errors = Vec::new();
      let mut cli = deterministic_cli_at_player_turn();

      cli.play_turn(&mut &input[..], &mut output, &mut errors).unwrap();

      assert!(String::from_utf8(output).unwrap().contains("Unknown action"));
      assert_eq!(cli.session().round().unwrap().player_hands()[0].hand().cards().len(), 3);
  }
  ```

  Run `cargo test cli::tests -- --nocapture` and capture red.

- [ ] **Green — implement a buffered I/O adapter and thin `main`**

  Implement token parsing as small pure functions and:

  ```rust
  pub fn run<R: BufRead, W: Write, E: Write>(
      input: &mut R,
      output: &mut W,
      error: &mut E,
      rng: StdRng,
      entropy_source: EntropySource,
  ) -> Result<(), CliError>;
  ```

  Prompt for bankroll, deck count (default 6), soft-17 rule (default stand), and
  payout (default 3:2). Loop through bet, optional insurance, player actions,
  settlement, and the next bet. Accept `h`/`hit`, `s`/`stand`, `d`/`double`, and
  `p`/`split`; allow `quit` only between rounds. Treat EOF as a clean exit.

  `main` obtains entropy, locks stdin/stdout/stderr, calls `cli::run`, prints a
  fatal typed error to stderr, and returns `ExitCode::FAILURE` on failure.

- [ ] **Refactor and verify**

  Keep rendering and parsing private to `cli`; no rule decision may be inferred
  from user input. Run CLI tests, all tests, rustfmt, and Clippy.

- [ ] **Commit**

  `git commit -m "feat: add interactive blackjack console"`

## Task 9: Documentation, Dogfood Demo, and End-to-End Coverage

**Files:**

- Replace: `tests/hello_world.rs` with `tests/console_smoke.rs`
- Modify: `Cargo.toml`
- Modify: `README.md`
- Modify: `Makefile`
- Modify: `docs/superpowers/specs/2026-08-11-console-blackjack-design.md`

- [ ] **Red — replace the obsolete process contract**

  Delete the Hello World assertion and add a process test that pipes startup
  configuration, a bet, `0`, `stand`, and `quit` to the compiled binary. Assert
  success plus stable prompt/round/settlement markers; never assert random
  cards. Run `cargo test --test console_smoke -- --nocapture` and capture the
  failing result before adapting the process output.

- [ ] **Green — make docs and Make targets exercise the real application**

  Update the package description. Rewrite README package information, rules,
  configuration, commands, exact wager increments, entropy behavior, build,
  run, verification, and module layout. Link only official Rust and `rand`
  documentation.

  Change `make demo` to feed a complete noninteractive input stream such as:

  ```make
  demo:
	@printf '100\n\n\n\nquit\n' | $(CARGO) run --quiet
  ```

  Update the approved design only if implementation clarified a contract; do
  not broaden scope.

- [ ] **Refactor and verify the full completion gate**

  Run, in order:

  ```console
  cargo fmt --all -- --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test --all-targets --all-features
  cargo build --release
  make demo
  ```

  Perform explicit mutation checks by temporarily changing one behavior at a
  time and proving focused tests fail for card values, Ace promotion, payout
  ratios, soft-17 behavior, and action legality. Restore each mutation using
  `apply_patch` and rerun its focused test before proceeding.

  Scan production code with:

  ```console
  rg -n 'unwrap\(|expect\(|unsafe|todo!|unimplemented!|panic!' src
  git diff --check main...HEAD
  ```

  Review `git diff --stat main...HEAD`; stop for scope review above 1,000
  implementation lines and split before 1,500 implementation lines.

- [ ] **Commit**

  `git commit -m "docs: document console blackjack workflow"`

## Final Review and Pull Request

- [ ] Invoke the Rust review checklist over `main...HEAD`, with special focus on
  ownership boundaries, exhaustive state transitions, exact arithmetic, error
  contexts, public API size, panic paths, and deterministic tests.
- [ ] Execute the mandatory post-implementation protocol in order: capture
  proof-of-work and iron-law evidence, update project docs, update the Makefile
  dogfood target, update README, and review/update test coverage.
- [ ] Re-run the full completion gate after every review fix.
- [ ] Record evidence as `[E1]`, `[E2]`, and later references with exact command,
  exit status, and relevant output; report `PASS`, `FAIL`, or `BLOCKED`.
- [ ] Confirm `git status --short`, review the complete diff, and verify no
  unrelated files or generated artifacts are staged.
- [ ] Push `codex/console-blackjack` and create a ready GitHub pull request whose
  body summarizes architecture, supported rules, exact money/RNG decisions,
  test evidence, and remaining exclusions.
