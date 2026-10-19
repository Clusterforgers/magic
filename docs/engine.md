# Engine

Design notes for `mtg-engine` (the rules engine) and `mtg-testkit` (scenarios, bots and invariants). The overall design is in [PLAN.md](PLAN.md); the wire form of its types is in [protocol.md](protocol.md).

## Shape

- `Engine` offers `new`, `resume`, `apply`, `concede`, `decision`, `state`, `events`, `state_hash`, `add_continuous_effect`, `checkpoint`, `restore` and `preview_forecast`; `replay(setup, events)` rebuilds a state from a log.
- One `Event` enum and one `apply_event` fold: the state is the fold of the events.
- Each `Decision` kind carries its own legal options: Mulligan, BottomCards, Priority, PayMana, DeclareAttackers, DeclareBlockers, AssignDamage, Discard, and the questions of spells and abilities: ChooseTargets, ChooseMode, ChooseNumber, DivideAmount, ChooseCards, OrderCards, YesNo, ChooseOption, ChooseCosts, OrderTriggers, ChooseReplacement. `Answer` mirrors them. Only ChooseTargets is asked; the answer to each of the others is refused as `IllegalAnswer::Unsupported` until its task.
- `FormatRules` is a trait. `Normal` is the only implementation; it skips the starting player's first draw only in two-player games.
- `caps()` names the primitives the engine runs, in the vocabulary of the card support records (`cards-dsl.md`); `RUNS` in `caps.rs` is the one list. Today: the plain tap-for-mana ability, `Ability.Spell`, `Ability.Activated` with `Cost.Tap`, and the effects Draw, DealDamage, GainLife and LoseLife. A spell with `{X}` in its printed cost (`CardDefinition::x_in_cost`) is never castable until X costs land.
- `MulliganOptions` states the mulligan costs from `FormatRules::mulligan_rule` (`MulliganRule { hand_size, free_mulligans }`): `free_mulligans`, `bottom_if_keep` (the count the `BottomCards` decision asks for) and `bottom_if_mulligan`. A format with a free mulligan (Commander, CR 103.5c) only sets `free_mulligans`. Clients repeat these counts and never work them out.

## Determinism and recovery

- Random choices use a ChaCha8 substream keyed by (seed, counter in state). Events carry the results, so replay needs no RNG.
- A half-finished cast (spell chosen, payment pending) lives outside game state; a rebuilt engine asks for priority again.
- Answers are validated before any mutation. A fold failure rolls back by replaying the log, and `checkpoint`/`restore` expose that rollback so the server can undo a step it failed to persist.
- `concede` is legal at any time (CR 104.3a): `PlayerLost { Conceded }`, then the normal elimination path (the player's objects leave, the turn ends if it was theirs, the game ends when one player remains). A half-finished cast is dropped. The mulligan stage runs the same game-end and housekeeping check as turns.
- An effect whose duration has ended stops applying at once; the later `EffectExpired` event only prunes it.

## Mana costs

- `ManaCost` (`mtg-core`) is a canonical list of `ManaSymbol`: `Generic`, `Colored`, `Colorless`, `Hybrid` (`{W/U}`), `MonoHybrid` (`{2/W}`), `Phyrexian` (`{W/P}`), `HybridPhyrexian` (`{G/U/P}`), `ColorlessHybrid` (`{C/W}`) and `ColorlessPhyrexian` (`{C/P}`).
  - Canonical means sorted (generic, colourless, colours, then the symbols with a choice), generic symbols merged, `{0}` dropped and a colour pair stored the way cards print it (`{G/W}`, never `{W/G}`). Equal costs are equal values, and `Display` then `parse` is the identity.
  - The list is also the wire form (`Array<ManaSymbol>`); deserializing normalizes.
- `mana_value` counts `{2/W}` as 2 and every other choice as 1 (CR 202.3). `colors` gives a hybrid symbol both colours, a Phyrexian symbol its colour and `{C/W}` white (CR 105.2). `generic()` and `required(mana)` count the plain symbols only.
- `parse_faces` reads the card-level `A // B` text of split and adventure cards.
- Any other symbol (X, snow, the silver-border symbols) is `ParseManaCostError::Unsupported`, named and never skipped.

## Payment

- Payment goes through resolutions (CR 601.2b). `ManaCost::resolutions()` lists every way to pay each hybrid and Phyrexian symbol as a plain cost plus the life paid. Equal symbols are split among the ways, not permuted: Reaper King has 32, Boros Reckoner 4, a plain cost one.
- `can_afford`, `auto_payment`, `check_payment` and `complete_payment` take the payer's life total and try the resolutions, handing each to the plain-cost code. A payment is valid when some resolution takes exactly its life and its mana.
- A permanent is one tap, whichever of its abilities is used. `can_afford` and `auto_payment` match coloured needs to distinct permanents, so `auto_available` is true only when a valid payment exists.
- `auto_payment` prefers mana to life, then the least life, then the fewest taps, then taps of the colours the player has most of, so a hybrid symbol keeps the scarce colour untapped. It pays life only when no mana can, and never down to 0 or less.
- A payment chosen by hand may take all of one's life (CR 119.4), so `can_afford` and `castable` count it and `PayOptions.auto_available` is then false.
- `PayChoice::Complete(partial)` validates the partial parts and requires that the chosen mana fits inside the cost. It pays the rest with `auto_payment` over the unspent pool and untouched permanents (refused if it cannot), keeps the life already chosen and adds life only where no mana can pay the rest and some life stays.
- `PayOptions.can_cancel` is true while nothing is paid. `Cancel` then drops the pending cast with no event: the spell never left the hand and the log shows nothing of it, so a rebuilt engine asks for the same priority decision. With the flag false, `Cancel` is refused.
- `Payment.life` is the life paid, 2 per Phyrexian symbol (`#[serde(default)]`, so an answer without it pays none). It must be even, at most 2 per symbol that life can pay and at most the life total.
- `PayOptions.life` (the payer's life total) and `max_life` (2 per such symbol, within the life total, rounded down to pairs) mean a client never works out the limit.
- The cast's events are `LifePaid { player, amount }` first, one event for the whole payment, before the spell goes on the stack; then `SpellCast`, the taps and `ManaSpent`. `LifePaid` folds as life lost, not damage, and is refused for more than the life total. A life total of 0 or less ends the game through the usual state-based action. Everything is validated before any event.

## Priority options

`PriorityOptions` lists `playable_lands`, `castable`, `mana_abilities`, `abilities` and `can_pass`, and says in `can_act` whether the holder can do anything useful. `abilities` is left out of the wire form while empty.

- `castable` means affordable at the timing the card allows: sorcery timing needs the active player, a main step and an empty stack; an instant, or a card a keyword gives instant timing, waits for nothing. Affordable counts the floating mana plus the untapped sources the player controls, each permanent once (`can_afford`). A spell that needs targets is castable only when every target specification has enough legal targets (CR 601.2c).
- `abilities` lists the activated abilities of the player's permanents that are not mana abilities and could be activated now (`priority::other_abilities`): the engine pays `{T}` and nothing else for one, a tapped permanent or a summoning-sick creature cannot, `Timing::Sorcery` waits for sorcery timing, and each target specification needs enough legal targets.
- `can_act` is true with a land to play, a spell to cast or such an ability. Mana abilities alone never count: tapping a land with nothing to spend the mana on is not an action, so after the last land drop with nothing affordable the holder has nothing to do.
- One function holds the rule (`priority::can_act`).
- The server uses the flag only to pass for a seat that asked for it ([server.md](server.md)). The response clock and the yields never read it, so the pace of a table never tells what a hand holds.
- `PlayOptions::auto_pass` in `mtg-testkit` applies the same rule in a bot game and is the reference the server tests compare with.

## Spells and abilities

The rules of the pipeline are in [cards-dsl.md](cards-dsl.md) (Execution); this section says where the engine keeps them.

- Objects: a `GameObject` has a `kind` (`Card`, `Token`, `Ability`) and a zone-change counter the fold bumps on every move (CR 400.7); `ObjectRef { id, counter }` names one incarnation. An ability on the stack is a `GameObject` that holds an `AbilityObject` (`abilities.rs`): its source, a snapshot of it, its index among the source's rules, an `Arc` to those rules and a trigger's cause. It has no printed side and is dropped when it leaves the stack.
- Snapshots: `snapshot(state, id)` (`snapshot.rs`) is last known information. `PutOnStack`, `Cause` and `StepDone` carry them, and the value evaluator reads an object live while its counter matches and from the snapshot otherwise.
- Casting: `cast.rs` keeps a cast or activation that has begun (`Pending`) outside the state. `announce(state, pending)` returns the next question (`Next::Ask`), the events that complete the cast (`Next::Commit`) or `Next::Drop` for a cast that can no longer be made. Targets come first, then the payment; backing out before the payment leaves no event. An activation that costs only `{T}` commits at once.
- Targets: `legal_targets(state, spec, source, controller)` (`choose/targets.rs`) serves choosing, `priority_options` and the recheck on resolution, and asks the keyword hook `protects`. An answer is checked against the options it answers and each target is chosen once.
- Resolution: `stack::resolve_top` begins the top object of the stack. While `GameState.resolving` is set, `flow::next_in_turns` takes the next move from `resolve::step` before state-based actions and priority (CR 704.3), so a resolution is never cut by a priority round. A step is a pure function of the state and its journal; the events it returns end with `StepDone`, which clears the journal.
- Rebuilding: the journal is state, so an engine resumed from a log stands at the same step, and one resumed from the middle of a step asks the same question again.

Seams the other tasks fill in; each is complete as a type and refuses or does nothing until its task:

| Seam | Where | Until its task |
|---|---|---|
| Decision kinds | `choose/<kind>.rs`: options and answer types, `answer` | the answer is `IllegalAnswer::Unsupported` |
| Effects | `resolve/prim/<effect>.rs`, one per `Effect` variant | `ResolveError::Unsupported` and no `Caps` entry, so the card is never offered |
| Keywords | `keywords/<name>.rs`, read by `stops_block`, `protects` and `grants_instant_timing` | every hook answers "no" |
| Replacement | `replace::process` in front of the events of every step | the events pass unchanged |
| Capabilities | `RUNS` in `caps.rs` | what the support records may call Verified |

## Combat forecast

`forecast::combat_forecast(&GameState) -> CombatForecast` says what combat damage would do if dealt now. It is derived from the state on every call and never stored.

- Per attacker: the damage it assigns (one packet per blocker in declared order, or one to the defender when unblocked) and the damage its blockers deal back.
- Per creature in combat: the damage it takes and whether it dies (damage marked plus expected damage against derived toughness, as in the state-based action).
- Per attacked player: the damage, the life after and whether that is lethal.
- Several blockers on one attacker: until the controller divides, the default is lethal damage to each blocker in turn and the rest on the last. Once `DamageAssigned` is recorded the forecast uses the recorded division.
- `pending_combat_forecast` is the guarded form views use. It returns `None` before attackers are declared, with no attacker left, and once damage has been dealt (the combat state lingers until the end of combat, and a damage step that has granted priority is past its damage).
- Keywords join in `damage::strike_power` (first/double strike, power), `forecast::lethal_damage` (deathtouch) and `forecast::divide` (trample). `damage::damage_events` is the real counterpart; change both together.

### Preview

`Engine::preview_forecast(seat, &answer)` (`preview.rs`) is the forecast of a pending DeclareAttackers or DeclareBlockers answer without giving it.

- The answer goes through `answers::handle`, the same validation and events as `apply`. The events fold onto a clone of the state and `pending_combat_forecast` reads the clone. Nothing is re-implemented, so a preview cannot disagree with the forecast after the real answer.
- `PreviewError` says why there is none:
  - `Illegal` carries the `IllegalAnswer` that `apply` would return (an illegal block is an error, never a forecast);
  - `NotYourDecision`: the seat does not face the decision;
  - `NotPreviewable`: any other decision kind, damage assignment included;
  - `NoDecision`: the game is over;
  - `NoCombat`: an attack with nobody;
  - `Engine`: the scratch copy refused the events, which is a bug.
- The method takes `&self`, so no event is logged, no random number is drawn and the state hash cannot change. The cost is one state clone.

## Verification

- Invariants: per-event checks run on every event; whole-state checks and the replay hash comparison run every 25 answers and at game end. The checker keeps its own life history (`LifePaid` counts as well as damage).
- Fuzz: 500 bot games each for 2, 3, 4 and 6 players (`MTG_FUZZ_GAMES` overrides the count). The dev profile builds `mtg-core`, `mtg-engine` and `mtg-testkit` at opt-level 3 to keep the run short.
- `RandomBot::new` keeps its choices fixed (cancel when allowed, automatic and exact payments) because seeded fixtures such as the web transcript replay its games. `with_partial_payments` and `with_symbol_choices` (`PlayOptions::partial_payments`, `symbol_choices`) opt into `Complete` and into choosing how to pay hybrid and Phyrexian symbols.
- Payment: `mana.rs` proptests compare `can_afford`, `auto_payment`, `check_payment` and `complete_payment` with an independent symbol-by-symbol check over costs of every kind, permanents with several abilities and varying life totals. `payment_fuzz.rs` checks that after a cancel the state hash, the log, the next decision and a replay agree.
- Mana cost parsing: `mtg-import/tests/mana_costs.rs` checks the mana value and colours of every distinct `mana_cost` string of Scryfall's bulk data against Scryfall's own ([data-sources.md](data-sources.md)).
- Forecast and preview: a proptest in `mtg-testkit` checks the forecast against reality in bot games. `preview_matches_games.rs` checks that every preview equals the forecast read after the real answer, that spoiled answers are refused exactly as `apply` refuses them, and that a game played with previews has the same log and hash as one without.
- Spells and abilities:
  - `spell_targets.rs` and `abilities.rs` play Lightning Bolt from the corpus and a `{T}` ability through the whole path: choose, pay, respond, resolve, fizzle (CR 608.2b), cancel for free, and an ability that outlives its source and reads it as it was.
  - `spell_fuzz.rs` plays `MTG_FUZZ_GAMES` (500) bot games for 2, 3 and 4 players with decks of spells and abilities under every invariant, including zone-change counters that never fall and the replay hash; it checks that the games use the pipeline and that seeded games repeat.
  - `pipeline_log.rs` replays the log of such games at every point, rebuilds an engine from decision points and from the middle of a resolution, and checks that games without spells never contain a pipeline event.
  - `resolve/mod.rs` runs a step that asks with a fake primitive to show k + 1 runs, one logged answer each, and a replay equal to the live state; `fold/tests.rs` folds every new event; the protocol tests cover the redaction arms ([protocol.md](protocol.md)).
  - The corpus table in `corpus_cards.rs` fixes which cards are Verified (Forest, Grizzly Bears, Lightning Bolt, Llanowar Elves) and checks that the gaps a support record names are what `caps()` lacks.

## Rules coverage

Deviations from the Comprehensive Rules:

- When the active player leaves mid-turn, the engine ends the turn at once. CR 800.4 lets the turn continue without an active player: priority goes to the next player in turn order, or the top object resolves, or the step ends.
- Damage to a planeswalker is only marked. CR 120.3c turns it into removed loyalty counters, which need loyalty counters first.
- The first-strike step exists but never occurs.
- Layer dependencies (CR 613.8) are not modelled; only type, colour and power/toughness modifications work.

Readings of the rules:

- An attacker divides damage freely among several blockers. The Foundations rules removed damage assignment order (CR 509.2, 510.1c); lethal damage only matters for trample, which is not implemented.
- A mana ability resets the passes in a priority round, a literal reading of CR 117.4 (all players pass "without taking any actions in between"). No official ruling exists and the impact is low.

Not implemented: keywords, triggers, tokens, control change and Commander, and any effect beyond drawing, life and damage; costs other than mana and `{T}`; X, modes, divided amounts, kicker and other announcements. Scry, search and the other keyword actions need choice decisions first. Scenarios cannot start in the blockers or damage steps.
