# Engine

Design notes for `mtg-engine` (the rules engine) and `mtg-testkit` (scenarios, bots and invariants). The overall design is in [PLAN.md](PLAN.md); the wire form of its types is in [protocol.md](protocol.md).

## Shape

- `Engine` offers `new`, `resume`, `apply`, `concede`, `decision`, `state`, `events`, `state_hash`, `add_continuous_effect`, `checkpoint`, `restore` and `preview_forecast`; `replay(setup, events)` rebuilds a state from a log.
- One `Event` enum and one `apply_event` fold: the state is the fold of the events.
- Each `Decision` kind carries its own legal options: Mulligan, BottomCards, Priority, PayMana, DeclareAttackers, DeclareBlockers, AssignDamage, Discard, and the questions of spells and abilities: ChooseTargets, ChooseMode, ChooseNumber, DivideAmount, ChooseCards, OrderCards, YesNo, ChooseOption, ChooseCosts, OrderTriggers, ChooseReplacement. `Answer` mirrors them. ChooseTargets and OrderTriggers are asked; the answer to each of the others is refused as `IllegalAnswer::Unsupported` until its task.
- `FormatRules` is a trait. `Normal` is the only implementation; it skips the starting player's first draw only in two-player games.
- `caps()` names the primitives the engine runs, in the vocabulary of the card support records (`cards-dsl.md`); `RUNS` in `caps.rs` is the one list. Today: the plain tap-for-mana ability, `Ability.Spell`, `Ability.Activated` with `Cost.Tap`, `Ability.Triggered` with the seven `Trigger.*` patterns, `Ability.Limit` (once each turn, for triggered and activated abilities), the effects Draw, DealDamage, GainLife, LoseLife and DelayedTrigger, and the twelve keywords of combat ([Combat and keywords](#combat-and-keywords)). A spell with `{X}` in its printed cost (`CardDefinition::x_in_cost`) is never castable until X costs land.
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
| Keywords | `keywords/<name>.rs`, read through `keywords_of` by combat, damage, state-based actions, priority and targeting | the stub of a keyword without its task answers "no" |
| Replacement | `replace::process` in front of the events of every step | the events pass unchanged |
| Capabilities | `RUNS` in `caps.rs` | what the support records may call Verified |

## Triggered abilities

The rules of the data are in [cards-dsl.md](cards-dsl.md) (Triggers, replacement and layers); this section says where the engine keeps them.

- Detection: `Engine::commit` folds a batch of events, then calls `triggers::detect(pre, events, post)`. It copies the state before the batch only when the batch looks back (`triggers::needs_before`: a death, a discard, a move out of the battlefield, a spell or ability that ends without resolving); any other batch is judged by the state after it. Each `Triggered { source, ability, controller, cause, snapshot, delayed }` it returns is logged as `AbilityTriggered` right after the batch and joins `GameState.pending_triggers`. Replay folds the logged events and never detects. A game with no triggered ability and no effect that sets one up (`GameState::watches`) skips detection and the copy.
- What happened: events become `Happened` values. `ZoneChange` comes from `LandPlayed`, `SpellResolved`, `CreatureDied`, `CardDiscarded`, `CardDrawn`, `ZoneChanged` and the end of a spell that did not resolve; `Attacks` from each attacker of `AttackersDeclared`; `SpellCast` from `SpellCast`; `BecomesTarget` from the targets of `SpellCast` and `AbilityPutOnStack`; `StepBegins` from `StepBegan`. A move out of the battlefield is judged against the state before the batch (the permanents that were there, the objects as they were), every other event against the state after. One `Happened` triggers an ability once, so an `AnyOf` that fits two ways triggers once; two attackers or two deaths trigger it twice.
- A trigger carries its controller (whoever controlled the source as it triggered), the subjects of the event with snapshots (`Ref::Triggering`) and a snapshot of the source, so the ability can read both once they have left the battlefield.
- Limits: `limits_spent` holds the (object, ability) pairs used this turn and is cleared by `TurnBegan`. A triggered ability with `OncePerTurn` does not trigger again and an activated one is not offered again; a new object (CR 400.7) has its own.
- The sweep: `flow::next_in_turns` runs it after state-based actions and the game-end check and before the turn structure, so it precedes every priority decision (CR 117.5, 603.3) and every repeat of it follows state-based actions. It takes the first player in turn order from the active player who has a trigger waiting. A player with two or more of the current round asks `OrderTriggers` once (`TriggersOrdered` marks them ordered) unless they are all of one source and ability, which do the same in any order; the rest go on in the order they triggered. Each trigger then becomes a pending announcement like a cast (`Pending`, `What::Trigger`): its target specifications are asked one at a time (`TargetOptions.triggered`, no cancel), and `AbilityPutOnStack` with its `cause` takes it out of the queue and puts it on the stack with its choices logged.
- Rounds: a trigger that triggers while others are going on the stack, such as one for becoming a target, belongs to the next round (CR 603.3b). A trigger takes `GameState.trigger_round` when it triggers, and each trigger put on the stack raises the number.
- Removal: a trigger whose target specification has too few legal targets (CR 603.3d), one whose controller or source has left the game, and one whose controller chooses no target (`TargetChoice::Remove`) leave the queue as `TriggerRemoved`.
- Holds: nothing goes on the stack in the untap step; the sweep waits for the upkeep (CR 502.4). A trigger in the cleanup step gets a round of priority and cleanup begins again afterwards (CR 514.3a, `steps::cleanup`).
- Resolution: an ability object keeps its trigger's cause, source snapshot and (for a delayed one) the names it took. An intervening "if" is judged by `Scope::holds` when the event happens and once more when the ability begins to resolve; if it is false then the outcome is `ResolutionOutcome::Lapsed`. Targets are rechecked as for any ability and `Ref::Triggering` is available while choosing them.
- Delayed triggers: the `DelayedTrigger` effect logs `DelayedTriggerCreated` with the pattern, the effects, the names the resolution had bound and who controls it; the fold adds a `Delayed` to `GameState.delayed` whose rules are a triggered ability. `detect` tries delayed triggers after the printed ones. One that triggers once is removed when `AbilityTriggered` folds. One that does not lasts until the end of the turn it was set up in. `Delayed::lapsed` also covers the durations of the DSL (end of combat, the controller's next turn, the source leaving) and a controller who left the game; housekeeping logs `DelayedTriggerExpired` for those.

Events: `AbilityTriggered`, `TriggersOrdered`, `TriggerRemoved`, `DelayedTriggerCreated`, `DelayedTriggerExpired`. The protocol shows a trigger about a card the viewer cannot see (a card drawn) as its hidden form ([protocol.md](protocol.md)).

## Combat and keywords

`keywords::keywords_of(state, id)` is the one reader of the keywords an object has: the printed ones in the order of its rules, none for an ability on the stack. Layer 6 adds and removes keywords there (task (k)); `has_keyword`, `damage_rules`, `summoning_sick` and the other helpers of `keywords/mod.rs` are built on it. Each keyword is a `KeywordRules` value in `keywords/<name>.rs` with its entry in `RUNS`.

| Hook | Keyword | Read by |
|---|---|---|
| `restricts_blocking`, `answers_evasion` | flying, reach | `stops_block` in `block_options` |
| `min_blockers` | menace | `block_options`, `check_blockers` |
| `forbids_attacking` | defender | `can_attack` |
| `ignores_summoning_sickness` | haste | `can_attack`, mana and `{T}` abilities, the views |
| `attacks_untapped` | vigilance | `attack_events` |
| `modifies_damage` (`DamageRules`) | first strike, double strike, deathtouch, trample, lifelink | `damage.rs`, the fold of `DamageDealt` |
| `is_indestructible` | indestructible | state-based actions; a `Destroy` primitive asks `keywords::is_indestructible` |

- Blocking: a creature that needs several blockers is offered in `BlockOptions.min_blockers` only while that many of the defender's creatures could block it, and `check_blockers` refuses fewer (`IllegalAnswer::TooFewBlockers`).
- Damage steps: the first-strike step occurs when an attacker or a blocker has first strike or double strike as blockers are declared (`damage::first_strike_step_needed`). A creature with first strike alone deals damage in it, one with double strike in both steps and any other in the regular step (`strike_power`, CR 510.4). State-based actions run between the steps, because damage and the priority grant are one batch and the next move checks them first. Entering a step (`StepBegan`) forgets the attackers' divisions, so a double striker divides twice.
- Dividing (`damage::division`): an unblocked attacker deals all its damage to what it attacks and a blocked one to its blockers, all to the only one or as its controller divides among several. With trample the player or planeswalker attacked also receives the damage beyond lethal to every blocker, and all of it when no blocker is left. A controller is asked (`DamageOptions`) only when more than one division exists; the default is lethal damage to each blocker in order and the rest on the player attacked or else the last blocker (`default_shares`).
- `DamageOptions.lethal` is the lethal damage of each blocker: its toughness less the damage marked, and at most 1 from a deathtouch source (CR 702.2c). `excess` is the player or planeswalker attacked, with trample. The answer lists the share of each blocker only: without trample the shares add up to the power; with it they add up to at most the power, the rest goes to `excess`, and that needs at least `lethal` on every blocker (`IllegalAnswer::ExcessBeforeLethal`, CR 702.19b).
- Deathtouch: the fold marks a creature (`PermanentState::deathtouched`) when a source with deathtouch deals it damage, combat or not, and the state-based action destroys it (`DeathCause::Deathtouch` when lethal damage would not have). Indestructible: lethal damage and the mark do not destroy it; zero toughness does (CR 704.5f).
- Lifelink: `damage::dealt` follows each `DamageDealt` of a source with lifelink with `LifeGained` for its controller, in the same batch; combat and the `DealDamage` primitive both use it.
- The views show `summoning_sick` as `keywords::summoning_sick`, so a creature with haste is not shown as sick.

## Combat forecast

`forecast::combat_forecast(&GameState) -> CombatForecast` says what the combat damage still ahead would do if dealt now. It is derived from the state on every call and never stored.

- It plays the damage steps on a copy of the state with the code of the real game: `damage_events` deals a step, the events fold, and state-based actions, the game-end check and housekeeping settle before the next step. A keyword is in the forecast because it is in the damage, and the forecast cannot drift from it. An attacker whose controller has not divided yet deals the default division; once `DamageAssigned` is recorded the forecast uses the recorded one.
- Per attacker: the damage it assigns (one packet per blocker in declared order, or one to the defender when unblocked, and with trample one to the defender after the blockers) and the damage its blockers deal back. A creature that deals damage in both steps has both in one packet.
- Per creature in combat: the damage it takes and whether state-based actions destroy it (lethal damage or deathtouch, unless indestructible).
- Per attacked player: the damage, the life after, with the life a lifelink blocker gains counted, and whether that is lethal. The life an attacker's controller gains from lifelink is not listed.
- Steps ahead (`damage::passes_ahead`): both when a first or double striker is in combat and none is dealt yet, the regular one alone in the first-strike step once it has granted priority and in the regular step.
- `pending_combat_forecast` is the guarded form views use. It returns `None` before attackers are declared, with no attacker left, and once the last damage has been dealt (the combat state lingers until the end of combat, and a regular damage step that has granted priority is past its damage).

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
- Keywords of combat:
  - `keywords_attack.rs` and `keywords_damage.rs` are tables over attacker and blocker mixes: the flying, reach and menace matrices, defender, haste with `{T}` and mana abilities, vigilance, first and double strike, deathtouch, indestructible, lifelink and trample with prior damage, deathtouch and several blockers; refused answers leave the state hash as it was.
  - `keywords_fuzz.rs` plays 500 bot games for 2, 3, 4 and 6 players between creatures with up to three combat keywords under every invariant, state-based actions settled at each decision included (`check_settled`), and checks that the games use them.
  - `keywords_forecast.rs` checks the forecast taken before the first damage against everything that combat dealt, across both steps, in bot games, and that the games reach every keyword; `preview_matches_games.rs` runs its checks on these games too.
  - The bot (`RandomBot`) leaves a menace attacker unblocked unless it has enough blockers (`BlockOptions.min_blockers`) and gives a trampler lethal damage before the rest at random.
- Forecast and preview: a proptest in `mtg-testkit` checks the forecast against reality in bot games. `preview_matches_games.rs` checks that every preview equals the forecast read after the real answer, that spoiled answers are refused exactly as `apply` refuses them, and that a game played with previews has the same log and hash as one without.
- Spells and abilities:
  - `spell_targets.rs` and `abilities.rs` play Lightning Bolt from the corpus and a `{T}` ability through the whole path: choose, pay, respond, resolve, fizzle (CR 608.2b), cancel for free, and an ability that outlives its source and reads it as it was.
  - `spell_fuzz.rs` plays `MTG_FUZZ_GAMES` (500) bot games for 2, 3 and 4 players with decks of spells and abilities under every invariant, including zone-change counters that never fall and the replay hash; it checks that the games use the pipeline and that seeded games repeat.
  - `pipeline_log.rs` replays the log of such games at every point, rebuilds an engine from decision points and from the middle of a resolution, and checks that games without spells never contain a pipeline event.
  - `resolve/mod.rs` runs a step that asks with a fake primitive to show k + 1 runs, one logged answer each, and a replay equal to the live state; `fold/tests.rs` folds every new event; the protocol tests cover the redaction arms ([protocol.md](protocol.md)).
  - The corpus table in `corpus_cards.rs` fixes which cards are Verified (Forest, Grizzly Bears, Lightning Bolt, Llanowar Elves) and checks that the gaps a support record names are what `caps()` lacks.
- Triggered abilities:
  - `triggers/detect.rs` has a table per pattern over states and events built by hand (what is judged against which state, the controller, cause and snapshot, `AnyOf` once, a limit, an intervening "if", a delayed trigger); `triggers/queue.rs` and `triggers/delayed.rs` test the queue, the rounds, the order that is asked for and the durations; `wording.rs` the words of a specification and a pattern.
  - `triggers_stack.rs`, `triggers_order.rs`, `triggers_targets.rs`, `triggers_chain.rs` and `triggers_delayed.rs` play the whole path in `mtg-testkit`: a trigger on the stack that another seat answers, the order of two to four seats (APNAP, the order asked or not), a fizzle, a lapsed "if", a chain of deaths and triggers without priority between, the cleanup step, a player leaving with triggers waiting, and a delayed trigger that outlives its source. `triggers_fuzz.rs` plays `MTG_FUZZ_GAMES` (200: a game of triggers lasts about three times as many turns) bot games for 2, 3 and 4 players with decks of triggers and checks that the games use every part. The checker also fails a priority decision while a trigger waits.

## Rules coverage

Deviations from the Comprehensive Rules:

- When the active player leaves mid-turn, the engine ends the turn at once. CR 800.4 lets the turn continue without an active player: priority goes to the next player in turn order, or the top object resolves, or the step ends.
- Damage to a planeswalker is only marked. CR 120.3c turns it into removed loyalty counters, which need loyalty counters first.
- The deathtouch mark lasts until damage wears off at cleanup. CR 704.5h looks at damage dealt since the last state-based check, so a creature that is indestructible when checked and loses it later in the turn would not die; here it does. No effect removes a keyword yet.
- A creature deals damage in the regular step when it has double strike or no first strike at that step. CR 510.4 reads the keywords as the first step began, which differs only when a keyword is gained or lost between the steps.
- Layer dependencies (CR 613.8) are not modelled; only type, colour and power/toughness modifications work.

Readings of the rules:

- An attacker divides damage freely among several blockers. The Foundations rules removed damage assignment order (CR 509.2, 510.1c); lethal damage only matters for trample, whose excess needs lethal damage on every blocker.
- Lethal damage for trample counts the damage marked and deathtouch; it does not count damage other creatures assign in the same step, since a blocker blocks one attacker.
- A mana ability resets the passes in a priority round, a literal reading of CR 117.4 (all players pass "without taking any actions in between"). No official ruling exists and the impact is low.
- Triggers that cannot differ (one source, one ability) go on the stack in the order they triggered; the rules leave the order of such triggers to their controller, and no choice is visible.

Not implemented: keywords other than those of combat, tokens, control change and Commander, and any effect beyond drawing, life and damage; costs other than mana and `{T}`; X, modes, divided amounts, kicker and other announcements. Scry, search and the other keyword actions need choice decisions first. State triggers (CR 603.8) and abilities that work from another zone than the battlefield (`from`) are not implemented; a card that needs either is left to the table. Scenarios cannot start in the blockers or damage steps.
