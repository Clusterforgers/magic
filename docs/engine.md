# Engine

Design notes for `mtg-engine` (the rules engine) and `mtg-testkit` (scenarios, bots and invariants): the invariants of the engine and where it keeps its rules. Per-keyword and per-effect behaviour lives in the code (`keywords/`, `resolve/prim/`, `announce/`) and its rustdoc; the rules of the data are in [cards-dsl.md](cards-dsl.md), the wire form in [protocol.md](protocol.md).

## Shape

- `Engine` offers `new`, `resume`, `apply`, `concede`, `decision`, `decisions`, `apply_for`, `state`, `events`, `state_hash`, `add_continuous_effect`, `add_continuous_effect_from`, `add_rule`, `add_rule_from`, `checkpoint`, `restore` and `preview_forecast`; `replay(setup, events)` rebuilds a state from a log.
- One `Event` enum and one `apply_event` fold: the state is the fold of the events. Characteristics are derived from base values plus continuous effects (layers) and never mutated directly.
- A game has two or more seats: a seat is a `PlayerId(u64)` index, turn order and priority run seat by seat in APNAP order, commander damage is keyed by (commander, victim), and no set of players is a fixed-width mask, so memory is the only ceiling (`tests/game/many_seats.rs` plays 8 to 24 seats).
- Each `Decision` kind carries its legal options and has an `Answer` of the same name; options, answer and check live in `choose/<kind>.rs`. The kinds: Mulligan, BottomCards, Priority, PayMana, DeclareAttackers, DeclareBlockers, AssignDamage, Discard, and the questions of spells and abilities (ChooseTargets, ChooseMode, ChooseNumber, DivideAmount, ChooseCards, OrderCards, Scry, YesNo, ChooseOption, ChooseCosts, OrderTriggers, ChooseReplacement).
- `choose::sole_answer` is the one answer the rules leave a question whose options every player can see (a trigger's forced targets, the only mode, number, option or replacement, a No to a payment the player cannot make unless a hidden card decides it: `YesNoOptions::unseen`); other questions, and any that can be backed out of, have none. The table gives it for a seat out of Full Control ([server.md](server.md), Windows).
- `FormatRules` is a trait with two implementations, `Normal` and `Commander`; the engine reads the values a table can override through `RuleSet` ([House rules](#house-rules)), never from the trait.
- `caps()` names the primitives the engine runs, in the vocabulary of the support records ([cards-dsl.md](cards-dsl.md)); `RUNS` in `caps.rs` is the one list. A card is never Verified for a primitive not on it.
- `MulliganOptions` states the costs of the resolved mulligan rule (`free_mulligans`, `can_mulligan`, `bottom_if_keep`, `bottom_if_mulligan`), so clients never work them out.
- The opening asks every seat at once (`Engine::decisions`, `mulligan::open_decisions`): keep or mulligan, bottom cards and scry concern only the seat's own hand, so `apply_for` takes the answers in any order, the log keeps that order and a mulligan round waits for every answer. A seat whose final hand holds an `Ability::OpeningHand` card is then asked `ChooseCards` for the cards to put onto the battlefield (CR 103.6, `OpeningCardsPlayed`).
- Seams: effects are `resolve/prim/<effect>.rs` (one per `Effect` variant; one without an implementation is `ResolveError::Unsupported` with no `Caps` entry, so its card is never offered); keywords are `keywords/<name>.rs`, read through `keywords_of`; replacement is `replace::process` in `Engine::commit`, in front of the fold of every batch, and a game with no replacement effect (`Possible.replacements`) passes every batch unchanged.

## House rules

- `HouseRules` (`mtg-core`, `house.rs`) is what a game's creator sets in place of the format's values. Every field is an `Option` (`None` keeps the format's value) except `handicaps`, a list. `validate` names the first field out of range (`HouseRuleError`); `validate_for(seats)` also refuses a handicap for a missing seat. The fields and `validate` are the rules and their ranges.
- `GameSetup::validate` checks `GameSetup::house_rules` against its seat count (`SetupError::HouseRules`); the rules are copied into `GameState` and hashed beside the format name, so a replay needs the setup it was played under.
- `RuleSet` (`GameState::ruleset`, `GameSetup::ruleset`) is the format with the house rules over it, the one place the engine reads these values; a seat's values (`starting_life`, `mulligan_rule`, `max_mulligans`, `mulligan_options`) read its handicap first. `FormatRules` supplies what a format may change (`starting_life`, `mulligan_rule`, `skips_first_draw`, `max_hand_size` (`None` unlimited), `poison_threshold`). `max_mulligans` is at most the opening hand size plus the free mulligans.
- A new rule is one field of `HouseRules` with its range in `validate`, one method of `RuleSet` and its readers, a protocol/web entry ([protocol.md](protocol.md)) and a table test.
- Rules beyond a number: `turn_limit` ends the game after a round's last turn (`TurnState::round`; highest life wins, a tie draws); `winning_life` makes the others lose (`OpponentReachedLife`; several at once draw); `empty_library` `reshuffle_graveyard` logs `LibraryRefilled`; `deck` (`DeckRules`: `singleton`, `color_identity` (older key `colour_identity` still read), `deck_size`, `max_copies`) and `commanders_per_seat` relax `FormatRules::validate_seat`; `attack_range` limits `AttackOptions` (`RuleSet::defending_seats`); `open_hands`, `revealed_library_top` and `opening_hand_smoothing` change what is revealed or dealt ([protocol.md](protocol.md)); `mulligan_style` is `london`, `paris` or `vancouver` (Paris plus scry 1, `MulliganStatus::Scrying`); `mana_burn` logs `ManaBurned`; `commander_tax` and `commander_damage` feed the cost modifiers and `Commander::loss_condition`.
- Card rules on the same limits: `Rule::NoMaxHandSize` and `Rule::MaxHandSize` (`max_hand_size` in `rules.rs`), `Rule::NoLegendRule` (`legend::groups`), `Rule::UntapDuring` (`untap_during`), `Rule::UntapLimit` (`rmt4_untap_limit.rs`: the untap step asks which permanents untap and keeps the first chosen that every limit allows, CR 502.3) and `Rule::SkipDrawStep` (`next_step`, CR 614.10).

## Teams

Teams (CR 808, CR 810) are `HouseRules::teams` (`mtg-core`, `teams.rs`): a `TeamMode` and the team of every seat, checked by `validate_for(seats)` (every seat assigned, two teams or more, no team number skipped, Two-Headed Giant two teams of two). The engine reads them once, in `teams.rs` on `GameState`.

- `is_opponent(a, b)` is "another team" and every opponent read goes through it: `Ref::Opponents`, `Targetable::Opponent`, `Activators::Opponents`, hexproof, the attack range, defending players, crimes, gifts, enchant opponent, "an opponent lost life" and the win of an effect. A table without teams is a team per seat, so nothing changes there.
- `RuleSet::game_result` ends the game once every player left plays on one team: `GameResult::Team(n)`, its members who lost earlier included (CR 808.6). The turn limit names the team of the sole leader.
- `TeamMode::Teams` (N v N): each player takes their own turn, priority and combat as in any multiplayer game.
- `TeamMode::TwoHeadedGiant` shares turns (CR 805): `is_active` holds for the active player's teammates, so sorcery timing, "your turn" durations, upkeep triggers, foretell and the draw step count read the team; each active player untaps, draws and discards at cleanup, and plays their own lands (`Tally::LandsPlayed`). The turn passes to the next seat of another team (`next_turn_seat`), and the first draw is skipped as in a game of two. The active player declares the attacks of the team's creatures; both players of an attacked team are defending players and block any creature attacking the team.
- Shared life (CR 810.9): the team starts at `FormatRules::team_life` (30, Commander 60) or the table's starting life; handicaps do not apply. Each player keeps the total and the fold copies any change to the teammate (`share_life`), so damage, loss, gain and payments are the team's while the tallies stay individual. Poison counters count together against 15 (CR 810.10), and a player whose teammate lost loses with them (CR 810.8a), a concession included.

## Commander

`Commander` (`format/commander.rs`, CR 903) is a `FormatRules`; the commanders and what is counted for them are in `commanders.rs` and `GameState::commanders`.

- Setup: `PlayerSetup::commanders` start in the seat's command zone (one or a Partner pair). A format without a command zone refuses them (`SetupError::NoCommandZone`). `FormatRules::validate_seat` checks the seat under its `RuleSet` with one `DeckError` per rule: count, eligibility (`CardDefinition::can_be_commander`), pairing (`partners_with`), size 100, copies (`singleton_limit`) and colour identity ([deck-check.md](deck-check.md)).
- The partner keywords and `Rule.CanBeCommander` are deck-building rules with no hook in play; Partner with also adds a trigger (`keywords/partner_with.rs`). A pair's colour identity is the two together.
- Colour identity is derived: `CardDefinition::color_identity` is the union over all faces of colour indicator, cost colours, the mana symbols of every ability the engine runs and of the clauses left to the table (reminder text excluded), and the basic land types. A corpus test holds it equal to Scryfall's.
- Rules: 40 life, the London mulligan with the first mulligan free (CR 103.5c), the first draw skipped only at two players (CR 103.8), `ExtraZone::Command` and the public zone `Zone::Command(seat)`.
- A commander is an object id in `GameState::commanders`, fixed at setup and kept through every zone change, so its counts survive them; a token or copy is no commander. Commanders, casts, damage and refusals are hashed only once they hold something.
- Casting: `ways::of` offers a commander in its owner's command zone at the timing of its type. The tax is a cost increase in `announce/modifiers.rs`, before the reductions: `commander_casts` (counted by the fold of `SpellCast` from a command zone) times `RuleSet::commander_tax`.
- Return (CR 903.9): a commander in a graveyard or exile is a `YesNo` of the state-based actions in `flow.rs` after the legend rule (yes is `ZoneChanged` to `Zone::Command`, no is `CommanderStayed`). One that would go to a hand or library, or be drawn, is a replacement candidate (`Origin::Commander`); a drawn one moved is no `CardDrawn`. The returned commander is a new object.
- Commander damage (CR 903.10a): the fold of `DamageDealt` counts combat damage per (commander, player); `Commander::loss_condition` loses a player at `RuleSet::commander_damage` from one commander (`LossReason::CommanderDamage`), so a pair counts apart.

## Companion

A companion (CR 702.139, `companions.rs`) is a card a seat names before the game and keeps outside it; cards only carry `Keyword::Companion(DeckCondition)`.

- Setup: `PlayerSetup::companion`. `FormatRules::validate_companion` checks that the engine reads its companion ability and that the starting deck, commanders included, meets the condition (`DeckError::NotACompanion`, `CompanionCondition`); Commander also keeps it inside the commanders' colour identity. A scenario does not validate.
- The condition is `mtg_cards::companion::DeckCondition` (`Each`, `Distinct`, `ExtraCards`) over selectors that read the card alone; one evaluator (`DeckCondition::check` over `DeckCard`s) serves the engine and the deck check.
- State: `GameState::companions` keeps each seat's definition, the object id it will have in hand and a `CompanionStage` (`Hidden`, `Revealed`, `Brought`, hashed). Outside the game the card is no object, so no zone, invariant or effect sees it.
- Reveal: the opening reveals every companion after `FirstPlayerChosen` and before the first shuffle (`Event::CompanionRevealed`).
- The action: `PriorityAction::BringCompanion`, offered as `PriorityOptions::companion` while the card is revealed and outside, the holder could cast a sorcery and can pay {3} (no cost modifiers). It shares the announcement of suspend and foretell (`announce/companion.rs`, `Special::Companion`): `PayMana`, then `Event::CompanionBrought` creates the object in hand, with no stack.

## Attractions

Attractions (CR 717, `un33_attractions.rs`) are implemented once in the engine; cards carry `Effect::OpenAttraction`, `Effect::RollToVisit` and the triggers `Visit`, `RolledToVisit` and `OpenedAttraction`.

- Setup: `PlayerSetup::attractions` is the seat's Attraction deck, top first, each card with its `Lights` (`mtg-core`). Its cards are objects in the owner's command zone, face down as `FaceDown::Attraction`; `GameState::attraction_deck` keeps the order and `lights` the lights. The opening shuffles each deck after the libraries (`AttractionDeckShuffled`, CR 103.3a).
- Opening (CR 701.51) moves the top card onto the battlefield face up (`ZoneChanged`), which takes it out of the deck and reads as `OpenedAttraction`; a seat without a deck opens nothing.
- Rolling to visit (CR 701.52) is a `DieRolled` (with `GameRule::ExtraDie`) and `RolledToVisit` naming the player's Attractions with the result lit; each visit ability triggers (CR 702.159a) and counts in `TurnTally::AttractionsVisited`. The active player rolls as the precombat main phase begins while controlling an Attraction (CR 505.5), after Saga counters and rad mill.
- The junkyard (CR 717.6): an Attraction card that would go to a hand, library or graveyard goes to its owner's command zone face up instead (`Origin::Junkyard`, applying again to the same event). A token or copy has no lights and goes where it would.
- The table sees the deck as a count (`PlayerView.attraction_deck`) and the lights of every visible Attraction (`ObjectView.lights`).
- A roll triggers `VisitedAttraction` once per Attraction visited; `Effect::ClaimPrize` is `PrizeClaimed`, which triggers that Attraction's `Prize` abilities and every `ClaimedPrize`.

## Stickers

Stickers (CR 123, `un34_stickers.rs`) are implemented once in the engine; a sheet lists its stickers as `Ability::Sticker`, and cards carry `Effect::PutSticker`.

- Setup: `PlayerSetup::stickers` are the seat's sheets, outside the game. With more than three, the opening picks three at random after the Attraction decks are shuffled (`StickerSheetsChosen`, CR 103.2d); a game that skips the opening takes the first three.
- Putting a sticker on: the player picks a permanent they own of the effect's selector (`ChooseCards`) and an unused sticker whose tickets they hold (`ChooseOption`, sheet name first); then `CountersChanged` for the tickets, `StickerPut` and the sticker's effect. With nothing to pick or pay, nothing happens. A sticker is used once.
- The change is a `ContinuousEffect` without end on that permanent (`Selector::Objects`), so it and the record of stickers end when the permanent leaves: a name sticker adds words in layer 3 (`Modification::AddNameWords`, `Characteristics::name_words`; `GameState::name_of` is the name the rules read), an ability sticker grants in layer 6, a power/toughness sticker sets both in layer 7b; an art sticker changes nothing.
- The table sees each seat's sheets and used stickers (`PlayerView.sticker_sheets`) and a permanent's stickers (`ObjectView.stickers`).

## Determinism and recovery

- Random choices use a ChaCha8 substream keyed by (seed, counter in state). Events carry the results (a shuffle logs its whole order, a roll its result), so replay needs no RNG. A fold that draws a substream moves the counter on.
- A coin flip (CR 705) asks the flipper to call (`ChooseOption` of `ChoiceOption::Side`, heads first, which the clock answers); `CoinFlipped` logs the call, the side and `won`.
- A half-finished cast lives outside the state; a rebuilt engine asks for priority again. Backing out (`Answer::Undo` or a kind's `Cancel`, wherever `can_cancel`) drops it with no event: the card never left its zone and no source was tapped, so nothing is rewound (CR 727) or shown.
- Answers are validated before any mutation. A fold failure rolls back by replaying the log; `checkpoint`/`restore` expose that rollback so the server can undo a step it failed to persist.
- `concede` is legal at any time (CR 104.3a): `PlayerLost { Conceded }`, then the normal elimination (the player's objects leave at once, even mid-resolution, control effects they gave end, what they still control is exiled, the game ends at one player). A half-finished cast is dropped.
- A turn whose active player has left goes on without one (CR 800.4): `GameState::active_player` is `None` while `TurnState::active` keeps the seat. Untap, draw, attack declaration and the hand-size discard are skipped, priority starts with the next player (`active_or_next`), cleanup ends the until-end-of-turn effects, and the next turn goes to the next player; `TurnView.active_left` shows it.
- An effect whose duration has ended stops applying at once; the later `EffectExpired` only prunes it.

### Fail-soft resolution

- A card in state `Read`, `Forge` or `Unverified` runs unproven rules ([cards-dsl.md](cards-dsl.md)). When a resolution step of a source whose definition has `read` returns a `ResolveError`, the engine keeps the steps folded, drops the rest and logs `AbilityLeftToTable {source, controller, text}` (no state change), so the table resolves the ability by hand. For other cards the error stays an `EngineError` for the tests, except `Unsupported`, always left to the table.

### By-hand lines

- A clause the table plays by hand that is one triggered line whose event reads carries that event (`ManualClause.when`, read at assembly by `compile::by_hand_event`, never serialised, so no recipe fingerprint changes). When the event happens to a permanent with such a line, the engine logs `ByHandDue {source, controller, text}` (a note: no state change) once for each time, judged as a trigger is (`triggers::due`, before the abilities that trigger on the same events); a card off the battlefield has no due lines.

## Mana costs

- `ManaCost` (`mtg-core`) is a canonical list of `ManaSymbol`: sorted, generic merged, `{0}` dropped, colour pairs in printed order. Equal costs are equal values, `Display` then `parse` is the identity, and the list is the wire form (deserializing normalizes).
- `mana_value` counts `{2/W}` as 2 and every other choice as 1 (CR 202.3); `colors` gives a hybrid symbol both colours. A symbol the type does not model (snow, silver-border) is `ParseManaCostError::Unsupported`, never skipped.

## Payment

- Payment goes through resolutions (CR 601.2b): `ManaCost::resolutions()` lists every way to pay each hybrid and Phyrexian symbol as a plain cost plus life. `can_afford`, `auto_payment`, `check_payment` and `complete_payment` take the payer's life and try them; a payment is valid when some resolution takes exactly its life and mana.
- A permanent is one tap, whichever ability is used; coloured needs go to distinct permanents. `auto_payment` prefers mana to life, then least life, fewest taps, then taps of the colours the player has most of; it pays life only when no mana can and never to 0 or less, while a payment by hand may (CR 119.4).
- A payment has nothing to spare: no floating mana, tap or part of one can be left out and still cover the cost (`spent_of`). A source adds `count` mana per tap (`mana_plan.rs`); mana beyond the cost floats.
- Sources with costs (`ManaOption.cost`, `takes`, `repeat`, `tapless`) pay too; `PriorityOptions.mana_abilities` offers one that costs mana while something can pay it (paid automatically, floating mana first, `costly_mana.rs`), not those that take. `check_payment` orders the costs so each is paid from earlier mana (`schedule`, `Spent.steps`). Planners search backwards (`mana_cost::auto`, at most `CHAIN` costly sources), `mana_takes::solving` drops sources that cannot take and `mana_repeat::expand` copies a repeatable source per use; the exhaustive check in `mana.rs` must match the four payment functions.
- A mana ability usable only as an instant (`Timing::AsInstant`, `ManaOption.priority_only`) is offered at priority, never as a payment source.
- Restricted mana (`SpendOn`, `Restricted`, `spend.rs`) stays in `ManaPool` until it empties and is spent first; the payment functions count it only where the payment's `Spend` allows (`spend_tests.rs`), and an effect's `Pay` takes only unrestricted mana. Riders (`SpendOn::When`, `Snow`, `Uncounterable`) restrict nothing and fire when a cast spends the mana (`spend_rider.rs`, `mana_snow.rs`); `SpendFact::OnBattlefield` needs a named source on the battlefield; a special action pays as `SpendKind::Special` (`SpendOn::Special`, `OtherCosts`).
- Kept mana (`AddMana.lasts`: `Step`, `Combat`, `Turn`) is counted in `ManaPool.counts` and `kept`; a spend takes the soonest-going mana first, `ManaPoolEmptied.through` names the shortest lifetime emptied, and mana burn counts only what went.
- Convoke, improvise and delve (`help.rs`) help after the total cost is fixed; their offer (`Help`) excludes objects the cast's additional costs take; the `*_with` functions take it, checked by `help_tests.rs`.
- `PayChoice::Complete(partial)` validates the partial parts and pays the rest with `auto_payment` from what is left, or is refused. `PayOptions.can_cancel` holds while nothing is paid.
- `Payment.life` is 2 per Phyrexian symbol: even, at most 2 per symbol life can pay and at most the life total; `PayOptions.life` and `max_life` give the limit. A cast logs `LifePaid` (one event), then `SpellCast`, the taps and `ManaSpent`, all validated first.
- The spell keeps which sources paid (`Choices.spent_from`: each tapped source's traits and mana spent), read by `Amount::ManaSpent { from }` (Treasure, snow, Desert, Cave). The pool keeps the source kind of each unrestricted mana (`ManaPool.sources`) and spends known sources first.
- A keyword granted with `CostItem::OwnManaCost` (escape, scavenge, encore from Underworld Breach) reads each card's printed cost in `layers.rs`; replicate and emerge read the printed card at the cast, so they do not take it.

## Priority options

`PriorityOptions` lists `playable_lands`, `castable`, `mana_abilities`, `abilities` and `can_pass`, and says in `can_act` whether the holder can do anything useful.

- `castable` lists cards in hand and, for a way naming another zone, in a graveyard, exile, the library top or the command zone, affordable at their timing; a spell with targets needs enough legal targets for every specification (CR 601.2c). Split second (CR 702.61) empties `castable` and `abilities`; mana abilities, special actions and passing stay.
- `abilities` lists activated abilities that work from the card's zone, are not taken by the payment planner and could be activated now (`priority::other_abilities`). `Activated.by` names who may activate (controller by default, any player or opponents); the activator controls the ability, and a cost that taps or gives up the permanent is not offered to a non-controller. `Limit::TimesPerTurn(n)` counts uses in `Tallies::limit_uses`.
- `unplayable` names each hand and command-zone card no list offers, with its first reason (`Unplayable`); `unplayable::judge` judges them once and also fills `castable`. `suspendable` and `foretellable` list the special actions.
- `can_act` is true with a land, spell, suspend, foretell or ability to use; mana abilities alone never count (`priority::can_act`). The server uses it only to pass for a seat that asked ([server.md](server.md)); clocks and yields never read it, so pace never tells what a hand holds. `PlayOptions::auto_pass` in `mtg-testkit` applies the same rule and is the server tests' reference.

## Manual actions

A table plays any card, also one the engine runs only in part, with its controller doing by hand what the engine does not (`manual.rs`; [cards-dsl.md](cards-dsl.md), Compiler and Manual fallback).

- A card the engine does not run whole is cast and resolves without the clauses it leaves out: a spell goes to the graveyard, a permanent enters with the abilities the engine runs. `CardDefinition.manual` lists the left-out clauses per face (`ManualClause {text, reason}`), visible to every seat. An Aura with a clause left to the table waits unattached until attached by hand.
- `PriorityAction::Manual(ManualAction)` is the seam for the seat with priority; `Engine::manual(seat, &action)` takes it out of band from any seat at any decision, with no entry in `PriorityOptions`.
- `Engine::manual` runs only where the engine rests at a decision; it is refused before the first turn (`NotStarted`), after the game, for a seat out of it and while a replacement question holds a batch (`Settling`). Its events commit like an answer's. A pending cast or activation replays its answers and asks again from the first that no longer fits (`announce/again.rs`), or is rewound without a trace if its card, source or attacker was taken (`announce/anchor.rs`). State-based actions follow once no resolution or announcement is under way.
- Replay: an action is logged as `Event::ManualAction {player, action}` plus its events; only `Shuffle` and `Roll` draw randomness, logged whole. `manual::check(state, seat, &action)` gives the refusal (`ManualRefusal`, in `IllegalAnswer::Manual`), which changes nothing. An opponent's card, life and counters are changed by the opponent's own seat.
- `CopyToken {object, count}` makes 1 to 10 token copies of any battlefield permanent (copiable values, CR 707.2) under the acting seat.
- `CreateToken` with `ManualToken::Card {card, index}` makes a token one of the seat's visible cards makes (not in the library, not face down): `manual_tokens::card_tokens` lists them from `CardDefinition::made_tokens` (predefined, described and fixed-size; no copies or counted sizes), once per definition per process, sent as `TableView.tokens`.
- Actions are built from the effects' events (`ZoneChanged` with `ZoneCause::Moved`, `CountersChanged`, `LifeGained`, `TokenCreated`, `CardDrawn`, ...), so replacement, triggers and state-based actions treat them alike; a move by hand is no draw or discard. The action keeps the seat's priority and resets the round (`reset_passes`). Library looks and reveals are kept in `GameState.looked_by_hand` and `revealed_by_hand` (read by `is_shown_to`).
- The bots never take a manual action. `mtg-testkit` has `ManualScript`, `PlayOptions::manual_every`, `Driver::manual` and `manual_fuzz_setup`; the invariant `ShownByHand` holds that a card revealed from a hand stays identified to every seat while there.

## Spells and abilities

The rules of the pipeline are in [cards-dsl.md](cards-dsl.md) (Execution); this section says where the engine keeps them.

- Objects: a `GameObject` has a `kind` (`Card`, `Token`, `Ability`) and a zone-change counter (CR 400.7); `ObjectRef { id, counter }` names one incarnation. A stack ability holds an `AbilityObject` (`abilities.rs`): source, snapshot, rule index, an `Arc` to the rules and a trigger's cause.
- `ability_reader` is the one reader of an object's abilities (a test searches for other reads of `rules`). `abilities_of(state, id)` lists them with addresses (`AbilityRef { object, ability, source }`) and `ability(state, address)` reads one from the data, so a granted ability stays readable after its effect ends. A permanent has its printed abilities less those taken away, then those granted in layer order (`Granted(Grant)`, `PrintedKeyword`, `GrantedKeyword`) and keyword `adds_abilities`. Any kind may be granted (CR 613.1f); granted rules and replacements count in `may_hold`. Pending, delayed and stack triggers hold the ability itself (CR 603.10a, 113.7a).
- Counters and attachments live in `PermanentState`; `GameState::host_of` names the host only while it is the same object (CR 400.7). `CountersChanged` is refused for a change of nothing, removing more than there are and a non-permanent.
- Snapshots (`snapshot.rs`) are last known information, carried by `PutOnStack`, `Cause` and `StepDone`; the evaluator reads an object live while its counter matches and from the snapshot otherwise.
- Targets: `legal_targets(state, spec, source, controller)` (`choose/targets.rs`) serves choosing, `priority_options` and the recheck, and asks the keyword hook `protects`. A resolution's first step binds the targets still legal (`~legal:<index>`, CR 608.2b).
- Earlier targets (CR 601.2c, 115.3): a filter reads `Target(n)` as the targets already chosen for earlier specifications (`legal_from`'s `earlier`, `Scope::spec_bases`), so "target player shuffles up to three target cards from their graveyard" offers that graveyard; before they are chosen, what reads them is left out. `Targetable::Another` excludes earlier targets; `TargetSet::TotalManaValueAtMost` bounds the chosen set, with `TargetOptions::total` giving each target's mana value.
- Resolution: `stack::resolve_top` begins the top object. While `GameState.resolving` is set, `flow::next_in_turns` takes the next move from `resolve::step` before state-based actions and priority (CR 704.3), after the objects of departed players (`sba::departures_first`).
- Rebuilding: the journal is state, so a resumed engine stands at the same step and asks the same question. Every answer is checked against its options (`NotOffered`, `Duplicate`, `WrongCount`) and logged as `ChoiceMade`. A choice with more than one possible answer is always asked; one that must take everything is not.
- Random picks (modes, `ChooseAtRandom`, random discards, bottom orders) draw a substream and log the result; a pick among hidden cards reaches other seats as `HiddenEvent::PickedAtRandom` (count only).
- Library effects: libraries are listed bottom to top, and every card list in an event or answer reads top down. A step asking several players one kind of question reads the journal through `pick_selections`; one mixing questions walks them through `Questions` (`choose/ask.rs`). All players decide before any library changes.
- What a seat may identify beyond the zones it sees: `Resolving.shown` keeps the cards of `LibraryLooked`, `HandLooked`, `CardsFound`, `Scried` and `Surveilled` for their seat and of `Revealed` for all, while the resolution lasts (CR 400.7); `sight::offered_cards(state)` names the cards of the pending question. Both derive from the state alone.
- Destroying, exiling and countering name their objects with `Scope::live_objects` and move them in one step, each step carrying a snapshot; `Scope.departed` keeps a target an earlier step moved named. A token ceases to exist in `move_object`, where every zone change folds.
- Source memory (`memory.rs`): `GameState::memory` maps (source, slot) to a list of `Held` (object, player or number): `Effect::Remember` holds objects and players, `Effect::RememberValue` a number that `Amount::Remembered` sums, `Forget` empties the slot. A number lives for the resolution that wrote it. The `Remembered` event carries objects, players and `values`; memory is hashed only while non-empty, and an object's zone change ends its place in any other memory.
- Costs of activated abilities (`Price`, `price.rs`, parts in `parts/`) are paid on activation (`cast::activation_events`); a free or `{T}`-only one commits at its last answer, one with mana asks payment last. A permanent tapped or sacrificed for the cost is no source of its own mana. A random draw or discard (`parts/draw.rs`, `parts/random.rs`) needs enough cards for every payment at once, and logs `ChosenAtRandom` before the discards.
- `Pay` in an effect asks `YesNo`; a player who cannot pay gets the same question with `YesNoOptions::only_no`, so nothing shows it; a Yes is refused before anything is logged.
- Mana abilities: the plain `{T}: Add {X}` is a printed ability; any other stays in its card's rules, and `mana_shape` (`mtg-cards`) reads those the engine runs. A hand card that exiles itself for mana (`ManaOption::exile`) is a source like a permanent: offered at priority and for payments, counted for `castable` (never its own cast, `mana_takes::for_card`) and for an effect's "Pay ...?" (`YesNoOptions::unseen`); `auto_payment` spends it last, like a sacrifice.
- A mana ability of a shape `mana_shape` does not read (a discard, mill, untap or X in its cost, one working from a graveyard) is listed in `abilities` and announced like any ability; it resolves before anyone receives priority and its controller keeps priority (CR 605.3a, 117.3c; `r281_mana_at_priority.rs`). It is never a payment source.
- `Cost.OneOf` as the whole cost of an `Effect::Pay` ("unless they sacrifice a creature or pay 3 life") asks each named player in turn order for one cost they can pay or none (`r281_pay_one_of.rs`).
- `SpendFact::Kicked` mana pays a spell announced with a kicker (`r281_kicked_spend.rs`); castability reads the spell unkicked.
- Multi-face cards (`CardDefinition.layout`, `faces`), face-down objects (`GameObject::face_down`; `Destination::BattlefieldFaceDown`, `ZoneCause::PutFaceDown`, turned up only for morph or disguise), merged permanents (`merged.rs`), flip, prepare and Rooms (`dk1c5_rooms.rs`) are modelled in the files named; their visibility is in [protocol.md](protocol.md). `announce/face.rs` asks which face to cast (601.2a).
- `Effect::LockOrUnlockDoor` (`Event::DoorLocked`, `r281_lock_door.rs`): the controller picks the door; locking removes its characteristics, unlocking is the special action's `DoorUnlocked`.
- `Effect::TurnFaceDown` makes a face-up permanent a morph (`TurnedFaceDown`, CR 708.2a); `Effect::Harness` marks it harnessed (`Harnessed`, `Pred::Harnessed`), read by the intervening "if" of its ∞ trigger.
- A new mechanic enters through one seam: an `Effect` variant with its `resolve/prim` file and `Caps` entry, a keyword file, or an `Event` with its fold and a `redact` rule.

## Announcement

`announce/` is the one procedure that puts a spell, an activated ability or a triggered ability on the stack (CR 601.2, 603.3). The begun action (`Pending`) lives outside the state, so nothing is logged until payment and backing out is free. `announce(state, pending)` runs the steps of `Phase::ORDER` and returns the first question (`Next::Ask`), the events (`Next::Commit`) or `Next::Drop`. The steps: face (601.2a), modes, alternative cost, optional costs (before modes for entwine), cost parts (`ChooseCards`), X, targets, division, legality, total cost, payment.

- What a card asks beyond modes and targets is `Needs` (`Needs::of`). Its ways to cast come from `ways::of` (`Way`: alternative cost, label, mana paid instead, zone); with one usable way it is taken, with more `ChooseOption` asks.
- `Pending::accept` keeps an answer in `Choices` for the step that asked and refuses any other.
- Total cost: the mana cost or alternative cost, plus chosen optional costs and X per `{X}`, then increases and reductions (CR 601.2f); a generic reduction stops at zero. `PayMana` is asked only when the player can pay, and priority offers a spell at its modified cost (`announce::cheapest`). Life a modifier adds (`CostModifier.life`) is worked out from the announced targets and paid with the cost; a caster with less life is rewound (CR 119.4).
- `ActivationKind` (loyalty, equip, cycling, unearth) logs `TallyEvent::KindActivated(kind)` per activation. `Ability::FreeActivation` makes its keyword's cost {0} while its condition holds (`free_activation::cost`); `Ability::InstantActivation` lets sorcery-speed abilities of the kind be activated at instant speed.
- `Targetable::AtRandom` draws among the legal targets from the next substream without asking; `Choices.random_targets` counts the draws.
- X: `ChooseNumber` offers 0 to the largest payable X; the spell keeps it in its choices and its permanent in `PermanentState.x`.
- Rewind: a step that cannot be met rewinds the whole announcement (CR 601.2): nothing to offer for a mode or target, an unpayable X or total, an impossible division.
- Casting during a resolution (`Effect::CastFromZone`, CR 608.2g) uses the same procedure with a granted way; special actions (suspend, foretell, companion, unlock, ending an effect) share it as `Special`. An `UntilPaid` effect (CR 116.2c) is offered to its controller in `PriorityOptions.endable`; paying ends all of it (`e15g_paid.rs`), and `LoseThisAbility` removes the resolving ability.
- Alternative and additional costs, `Ability::CastOnly` and the keyword ways (`CastWay::*`) are read in `ways.rs` and `announce/required.rs`, with details in each `keywords/<name>.rs`.
- Assist (`announce/assist.rs`) follows the total cost: the caster picks a player, who picks an amount (`ChooseNumber`, no cancel, default 0) taken off the generic mana and paid in the caster's batch. Prototype is `GameObject::prototype`; Sneak puts the creature into the attack as it resolves (`sneak.rs`); Craft is a cost pick over battlefield and graveyard (`Kind::Craft`) and a return of the exiled card (`Ref::Arrived`).
- Splice (`keywords/splice.rs`, `announce/splice.rs`, CR 702.47): after the modes, `ChooseCosts` offers hand cards whose splice names the spell's kind and can be paid; their mana joins the total and their other parts are paid with the spell's (`splice::parts_paid`). The chosen cards (`CostTaken::spliced`, `Choices::spliced`) are revealed and stay in hand; `with_spliced` appends their targets and effects wherever the spell ability is read, so copies keep them. A splice cost that taps, untaps or moves the card, counts an X or takes counters is not offered.

## Replacement

The rules of the data are in [cards-dsl.md](cards-dsl.md) (Triggers, replacement and layers); this section says where the engine keeps them.

- `Engine::commit` is the one place events are applied: `replace::process`, then the fold and the log. A resolution step's batch is whole (its events and `StepDone`), so a step is applied or asked about as a unit.
- A question of the step (`ChooseReplacement`, yes or no, a colour or creature type) is the engine's decision: `Engine` holds the batch and answers (`replacing()`), and an answer reruns `process`, which asks the same questions in order. The answers are not logged; a batch the state cannot recompute (a land played, a cost) is lost with the engine, one it can (a resolution step, state-based actions, combat damage, a draw) is asked again.
- `process` gathers the replacement effects once per batch (`gather`) and looks at each item; candidates not yet applied to it are ordered by address. Applying changes the item or replaces it with new ones that carry the effects already applied. Each `ReplacementCandidate` carries its own `text`.
- `Instead`, `Plus` and `PreventAnd` run inline (`resolve/inline.rs`): the tree is walked as a resolution walks it, each step reading the state the earlier ones leave on a copy, and the events are folded together in order. The player the effect names (its controller by default) answers the steps' questions (CR 614.1, 616); a single declined `May` leaves the event. Steps that cannot run inline (modes, `Pay`, a cast, waiting for own events) are primitives the engine does not offer (`ReplaceBy.Instead.Modal`, `Pays`, `Casts`, `Waits`); the checker refuses an unread ability using one (`proposal/ri23_skipped.rs`).
- A replacement of its own object's move to a graveyard with no `from` works in every zone (`ability_reader::works_anywhere`). `IntoLibrary` sends the object to its owner's library; an added shuffle's order is drawn in `finish` from the library the earlier events leave.
- Entering untapped is ordered after the effects that tap the entering permanent (CR 616.1).
- A cause filter (`by` of `ReplaceWhen::Discarded` and `CountersPut`) reads the controller of the resolving or announced spell or ability (`triggers::cause_of`); a discard with no cause (cleanup) matches none.
- Rebound and paradigm exile a resolved card spell (`Origin::Rebound`, `Origin::Paradigm`) and set up the delayed trigger that casts it or a copy; paradigm only when none waits for that name.
- A mill is one event (`ReplaceWhen::Mill`): a batch's run of `ZoneCause::Milled` moves of one player's cards. `Multiply` and `Add` take more cards (`replace/e19_mill.rs`); each card's move is still replaced on its own.
- Shields are `RuleEffect`s (`Rule::PreventDamage`, `Rule::Regenerate`, `Rule::DiesInstead`) made by `Effect::Shield` and `Effect::Regenerate`; `ShieldShape` (mtg-cards) names those the engine plays and the validator makes any other a gap. `DiesInstead` with `leaves` covers any move off the battlefield.
- The testkit's state-based checks skip a decision of the step: its events are not applied yet.

## Triggered abilities

The rules of the data are in [cards-dsl.md](cards-dsl.md) (Triggers, replacement and layers); this section says where the engine keeps them.

- Detection: `Engine::commit` folds a batch, then calls `triggers::detect(pre, events, post)`. The steps of an `Effect::Together` fold one by one and are detected as one batch after the last (CR 603.2c, 603.10a); a rebuilt engine finds the batch start again (`recover_batch`). Events become `Happened` values. A player who loses in the same batch triggers nothing (CR 800.4a).
- A trigger carries its controller (the source's controller as it triggered), the event's subjects with snapshots (`Ref::Triggering`) and a source snapshot. `limits_spent` holds the (object, ability) pairs used this turn, cleared by `TurnBegan`.
- Turn counts are fold tallies (`Tally::*`, `TurnTally`), cleared with the turn. Damage taken is split into combat and noncombat; a sacrifice is logged as the permanent last was, for `Amount::Tally` of `Sacrificed`; a tap carries the permanent's taps this turn and whether it attacked (`TappedNotAttacking`); a turn's first discards are the batch holding all of the player's discards so far.
- The sweep runs in `flow::next_in_turns` after state-based actions and the game-end check, before every priority decision (CR 117.5); state triggers (`triggers/state.rs`) are looked for there only when a card has one.
- A trigger that triggers while others go on the stack waits for the next round (CR 603.3b). One without enough legal targets (CR 603.3d), or whose controller or source left the game, is removed (`TriggerRemoved`). Nothing goes on the stack in the untap step.
- An ability object keeps its trigger's cause, source snapshot and (delayed) bound names; the intervening "if" is `Scope::holds`. `DelayedTrigger` creates delayed triggers (`DelayedTriggerCreated`).
- Events: `AbilityTriggered`, `TriggersOrdered`, `TriggerRemoved`, `DelayedTriggerCreated`, `DelayedTriggerExpired`; visibility in [protocol.md](protocol.md).
- Evolve (CR 702.100d): +1/+1 counters put on a creature while its own evolve ability resolves are its `Evolved` action (`rmt8_turns::evolved`), with no event of its own.
- Haunt (CR 702.55): the keyword adds "when this dies" and `TriggerPattern::ResolvedToGraveyard`, each exiling the card haunting a target creature (`ZoneCause::Haunting`, `GameObject::haunting`); the card's ability works from exile on `Pred::HauntedBy`, judged against the creature as it last was.
- Opening hand (CR 103.6): the question lists `Ability::OpeningHand` (onto the battlefield) and `Ability::OpeningReveal` (revealed, each setting up a one-time delayed trigger). `Ability::OpeningHandWith` (Gemstone Caverns) is offered only to a non-starting player when it says so, begins with its counter and leaves the seat in `Opening` with `Player::opening_exile` cards owed until a second question exiles them.
- An attachment that becomes unattached (unattached, moved or leaving) triggers `TriggerPattern::Unattached`, with the permanent it left as the triggering object, judged before the batch.
- An Aura whose enchant restriction names a graveyard (`Enchant [Creature, In Graveyard]`, Animate Dead) targets a card there and resolves attached to it (`fold_attached`, `host_of` and `legal_host` accept that host for such an Aura only). On the battlefield the rest of its restriction applies; its own trigger returns the card (`Ref::Attached`) and attaches it, and a move to another creature is not refused.

## Combat and keywords

`keywords::keywords_of(state, id)` is the one reader of an object's keywords: printed in rule order, then those granted (layers). A keyword is implemented once, as a hook the engine asks where it decides (`keywords/<name>.rs`):

| Hook | Keyword | Read by |
|---|---|---|
| `restricts_blocking`, `restricts_blocked`, `answers_evasion` | flying, reach, fear, intimidate, skulk, shadow, horsemanship, landwalk | `stops_block_between` in `block_options` |
| `min_blockers` | menace | `block_options`, `check_blockers` |
| `forbids_attacking` | defender | `can_attack` |
| `ignores_summoning_sickness` | haste | `can_attack`, mana and `{T}` abilities, the views |
| `attacks_untapped` | vigilance | `attack_events` |
| `modifies_damage` (`DamageRules`) | first strike, double strike, deathtouch, trample, lifelink, wither, infect, toxic | `damage.rs`, the fold of `DamageDealt` |
| `adds_abilities` (a `defining` static) | changeling, devoid | `layers::outside` and the layers pass |
| `is_indestructible` | indestructible | state-based actions; the `Destroy` primitive |
| `restricts_targeting`, `restricts_player_targeting` | hexproof, shroud | `legal_targets` through `protects` and `protects_player`; a player's hexproof from a quality through `hexproof::ep23_player_hexproof_from` |
| `protects_from` (`Holder`, `Source`) | protection | `keywords::protected_from`: damage in `replace.rs`, `legal_host` in `attach.rs`, `stops_block_between`, `legal_targets` |
| `grants_timing` | flash | `priority_options` through `grants_instant_timing` |

- Keywords that are triggers or activated abilities (prowess, ward, equip, crew, cycling, ...) return DSL abilities from `adds_abilities`, one set per instance, read as printed abilities.
- Damage steps: there is a first-strike step when an attacker or blocker has first or double strike as blockers are declared (`damage::first_strike_step_needed`). An attacker divides damage freely among its blockers (`damage::division`); `DamageOptions.lethal` is toughness less marked damage, at most 1 from deathtouch, and `excess` goes to the attacked player or planeswalker under trample. Deathtouch marks the creature and state-based actions destroy it; lifelink is `LifeGained` in the batch of the `DamageDealt`, in combat and in `DealDamage`.
- Declarations: the engine lists `AttackOptions.required` and `BlockOptions.required` (CR 508.1d, 509.1c), `min_blockers`, limits and taxes (`rules.rs`, `attack_tax.rs`, `block_tax.rs`) and refuses a declaration that breaks them; a taxed one waits for payment. A creature with no player to attack need not attack, and a block requirement never binds a block that costs (CR 509.1d).
- Removal from combat (CR 506.4): a change of controller, an attacked planeswalker leaving (`Attacker::attacks_nothing`) and `Effect::RemoveFromCombat` fold into the combat state.
- Banding (`keywords/banding.rs`, CR 702.22): the declaration names bands (`Answer::DeclareExerting::bands`, `AttackerOption::banding`), checked and folded from `BandFormed` into `CombatState::bands` for the combat. Blocking one member blocks the band. A banding blocker's controller divides the attacker's damage; against a band with banding, the active player divides the blocker's damage (`DamageAssigned` names the blocker, `CombatState::blocker_assignments`).
- A token that enters attacking or blocking (CR 508.4, 509.1h) is folded from `TokenCreated.attacking` or `.blocking`: never declared, so nothing triggers; it blocks only an attacker still attacking its controller.

## Continuous effects

`Effect::Continuous` makes the effects of a resolved spell or ability (CR 611); the rules of the data are in [cards-dsl.md](cards-dsl.md) (Triggers, replacement and layers).

- A `ContinuousEffect` (`effects.rs`) in `GameState.effects` holds id, layer (none for a restriction), selector, modification, duration, timestamp and source. Creation fixes the named permanents as `Selector::Objects(Vec<ObjectRef>)` (CR 611.2c), which matches that incarnation only.
- `layers.rs` derives every permanent at once and caches the result until the next event (`LayersCache`, kept only across `layers::leaves_alone` events). Dependencies (CR 613.8) are chosen again after each effect applies.
- A static ability that names another zone affects the cards there, and an `AnyOf` of `In` zones affects each (`layers::ep22_zones_named`). Characteristic-defining abilities work in every zone. A copy effect is the first layer.
- Control: `controller_of` is the controller after layer 2; `GameObject::controller` the one it entered under.
- Text changes (CR 612, `text_change.rs`): `Modification::ChangeText {from, to}` applies in layer 3 to abilities, keywords and type-line subtypes (a printed subtype of the same kind stops counting). A word is a color, basic land type or creature type where the rules hold it as a value; mana, costs and names are no words. `ChooseValue` (`BasicLandType`, `ColorOrBasicLandType`, `LikeChosen(from)`) picks both words, named in `ChangeTextChosen`.
- Restrictions (`CantAttack`, `CantBlock`, `DoesntUntap`, `CantBeBlocked`, limits) have no layer; they are read through `characteristics` by `can_attack` and `block_options`.
- `ContinuousEffect::is_active` judges durations. `Engine::add_continuous_effect` and `add_continuous_effect_from` register an effect with no card, for tests.
- Designations: a suspected permanent's menace and "can't block" are in the base of layer 6 (`Held::suspect_cant_block`), so losing all abilities removes both (CR 701.60c); a solved Case (`Event::Solved`, CR 719.3) stays solved until it leaves. "As ~ is turned face up" counters (`ReplaceWhen::TurnedFaceUp`) come with the turn-up event (CR 708.8).
- Speed (CR 702.179, `speed.rs`): start your engines! gives speed 1 with the state-based actions; an opponent's life loss on the active player's turn marks the once-per-turn rise, given at the next state-based check without the stack; speed stays after the permanent leaves.
- Layer 7c: number counters change power and toughness, keyword counters grant the keyword. +1/+1 and -1/-1 counters annihilate in pairs (CR 704.5q).

## Control of players and spells

- Player control (CR 722, `player_control.rs`): `PlayerControlGained {player, by, extra_turn}` schedules it; the player's next `TurnBegan` starts it and the one after ends it. The latest wins, one whose controller left ends, and `extra_turn` (Emrakul, the Promised End) adds the extra turn. `GameState::player_controller` and `sees_as` read it.
- The engine asks the controlled player's decisions as before (`Engine::decision_owned`); `Engine::decision` shows each to its answerer (`player_control::answerer`), so routing by `Decision.player` is unchanged. `ControlSearches` (Opposition Agent) hands over a choice among the player's own library.
- The controller sees what the controlled player sees (CR 722.3): hand, face-down cards, cards shown and offered (`sight.rs`, `zone_is_visible_in`).
- Spell control (CR 110.2): a `Continuous` `SetController` naming a stack object logs `SpellControlGained`, so it resolves for the new controller and a permanent spell enters under them; `ChangeTargets` asks the new controller.
- `ExileFound` (Opposition Agent) exiles each card a search finds as it is logged (`search.rs`) and gives its controller `MayCast` from exile with `play` and `spend_as: AnyColor`; the step that would have moved the cards finds nothing.
- Cleave (CR 702.148): `keywords/cleave.rs`; the rule is in [cards-dsl.md](cards-dsl.md) (Keywords).
- An `{X}` only a cleave, madness, foretell or freerunning cost holds is announced for that cast and is 0 otherwise (CR 107.3b, `ways::r283_keyword_x`).
- Restart (CR 727, `restart.rs`): `RestartGame` logs `GameRestarted {by, kept}`, whose fold returns every card to its owner's library (commanders to the command zone) as new objects, removes every other object but the resolving one, resets the players and returns to `PreGame`; the opening runs with `by` first, and the kept cards still in exile enter under `by` before turn 1.

## Copies

A copy effect (CR 707) gives a permanent the copiable values of another object (`copies.rs`); the rules of the data are in [cards-dsl.md](cards-dsl.md) (Copies).

- Copiable values are a one-face `CardDefinition`. `PermanentState::copy` holds a `CopyEffect` (object, values, timestamp), begun by `Event::BecameCopy`; exceptions (CR 707.9b) apply when it is made (`CardDefinition::except`). A copy that left the battlefield takes its abilities from last known information (`Snapshot::copy`).
- Token copies are `TokenRef::CopyOf`; copies for a duration end with `CopyEnded` (`until_end_of_turn`, `until_next_turn_of`).
- Spell copies (CR 707.10) are `SpellCopied` tokens above the resolving object; the controller is asked once per copy about new targets. Ability copies clone the `AbilityObject` (`AbilityCopied`).

## Rules of the game

Rules that change how the game is played rather than the objects in it (`Effect::Rule`, `Ability::Rule`; [cards-dsl.md](cards-dsl.md), Triggers, replacement and layers).

- `rules.rs` gathers the rules in force once per state: resolved `Effect::Rule`s still lasting (`GameState.rule_effects`, `RuleCreated`, `RuleExpired`) and the static ones, whose references resolve whenever the state changes.
- Each rule has one reader at the place that decides it (`must_attack`/`must_block` in the declaration offer, `attack_tax.rs`, `block_tax.rs`, `Rule::MayCast` via `rules::cast_permissions`, `prohibit::forbidden`, `Rule::AdditionalLandPlays`, trigger doubling). `spend_as` prices a cast with `ManaCost::paid_by_any_mana`, or `paid_by_any_color` (keeps `{C}`). A rule with no reader does nothing.
- `rules::player_effects` lists the rules and keywords on one player as `PlayerEffect`s from the same gathered rules, for the table's pods.
- The legend rule stops for a player's permanents (`Rule::NoLegendRule`) or a selector's (`Rule::NoLegendRuleFor`, `rules::no_legend_rule_for`).
- `CombatQuirk`s of `GameRule::Combat` (`combat_quirks.rs`): assigns no combat damage, assigns by toughness, may assign as though unblocked (CR 510.1c; `DamageOptions.unblocked`), may stay tapped (CR 502.3).
- Join rules (`r286_joins.rs`, CR 508.1c, 509.1b): `AttackWithOthers`, `BlockWithOthers` (at least N others the selector picks, read with the creature as `This`), `AttackAlone` and `AttackWithAll` become `JoinRule`s of the options; the check refuses a declaration that breaks one, a bound creature is never required, and the web holds Confirm while one is broken.
- The untap step asks the active player one `ChooseCards` (min 0, `steps::untap_events`) of the tapped permanents that may stay tapped; the answer stays tapped, the clock untaps all, and with none there is no question.
- `Duration::WhileSourceTapped` ends an effect when its source untaps or leaves (CR 611.2b).
- `AttackOptions::required_attack` and `BlockOptions::required_blocks` declare only what the rules require, for the bots, the server's clock and the testkit's passive seats.
- A library top played revealed (`GameState::revealed_top`) is shown to every seat; `Rule::LookAtTop` to its owner alone.
- `GameRule::LifeTotalCantChange` (CR 119.7) stops gain, loss, lifelink and mana burn.
- A `MayCast` from its owner's hand for the card itself (`flash`) is gathered with the hand's rules and holds while its condition does; with `additional` costs (MayFlashCost) it is used only when a sorcery-speed cast is impossible, and the cast pays the extra.
- `CantLoseGame.life_only` spares only the loss for 0 or less life (CR 704.5a).
- `GameRule::RevealHand` shows the named hands to every seat, later draws included (`is_shown_to`, `sight::revealed_hand`).
- `ReplaceWhen::LoseGame` sees each `PlayerLost` (`Unit::Loss`); only `Instead` replaces it, and state-based actions check again.
- `ReplaceBy::Subtract` and `Cap` lessen damage as a replacement, not a prevention (CR 614.1a); `Half` halves rounded down; damage brought to 0 is not dealt.
- `GameRule::FlipTwoCoins` (CR 705.3): one call, each rule doubles the coins, on differing coins the flipper keeps a won or lost one (`ChooseOption`); one `CoinFlipped` logs the kept side and every coin (`coins`); only the kept coin wins or loses.
- `GameRule::ExtraDie`: each rule adds a die and drops the lowest; one substream, only the kept dice logged as `DieRolled`.
- A cost removing counters from its source records the number (`CostTaken::removed`) for `Amount::RemovedThisWay`; "Remove all" is settled at announcement.
- A spell's own cost modifier reads its announced X; another object's reads none.
- A repeatable additional cost (`AdditionalCost.repeatable`) is asked like a multikicker; `Amount::AdditionalPaid` counts the payments.
- `MoveCounters.up_to` asks per counter with a "Stop" option (`w2f_any_counters`); a `from` naming a kind of permanent has each give up to the count in turn.
- `Ref::Union` names two damage recipients as one ("you and/or permanents you control"): a shield over it covers either and spends one budget.
- `GameRule::ManaProduced` (`mana_replace.rs`, CR 106.6): `ManaChange` sets, multiplies or converts the mana of a tap, rewriting the options of `mana_ability::options` so planner and activation agree; never on the stack.
- `GameRule::KeepUnspentMana` keeps one type (or all) as steps end and `UnspentManaBecomes` converts the rest; `ManaPoolEmptied` names both. `SpendManaAs` and `PayLifeForMana` (`{B}` as `{B/P}`) change the total cost in `announce/total.rs` and the ways offered.
- `GameRule::DrawLimit` stops draws past a number per turn (`Tally::Drawn`); `OpponentsCantMakeSacrifice` exempts the player from opponents' sacrifices; `AbilitiesAsThoughHaste` lifts summoning sickness for `{T}` and `{Q}` abilities only (`keywords::sick_for_abilities`).
- `NewTargets::One` with `onto` changes one target to that object where legal; the effect's controller picks which (CR 115.7c).
- `ReplaceWhen::CountersPut.by` names who puts the counters (`cause_of`) and `effect` asks for a resolving spell or ability; `ReplaceBy::Half` halves them rounded down.
- `GameRule::MayCastRather` (airbend) gives each exiled card's owner a cast for another mana cost (`CastWay::Alternative`) and no other cost.
- `Ref::Granter` is the permanent whose static ability gave the running ability ("this Equipment"): `AbilitySource::granter`, carried by `Scope` and `Price`, found by a cost pick; nobody for an own ability or a departed granter.
- `Ref::OrController(r)` is the player `r` names or the controller of the permanent it names.
- `Pred::CanEnchantSource` holds for an Aura or Equipment card `attach::legal_host_entering` lets attach to the source.
- "Cast as though it had flash if you pay {N} more" is an `AltCast` (`CastWay::Alternative`, `flash`) for the printed cost plus {N}; a printed {X} has no such way.
- Morph, megamorph and disguise costs may have parts beyond mana; turning face up asks their picks before the mana (`announce/turn_up.rs`), and the permanent never pays for itself.
- `ReplaceBy::PayOrGraveyard` (Mox Diamond) takes a sacrifice or discard as the permanent would enter, or sends it to the graveyard; an unpaid land play still counts (`ZoneCause::LandPlayed`).
- `CostItem::Unattach` in a granted ability unattaches the giver; `CostItem::XAtLeast` bounds an activation's X from below.
- `CostItem::Waterbend` is generic mana that untapped artifacts and creatures help pay like improvise (`help::waterbend`); only an activated ability's cost offers the helpers, elsewhere it is an unpaid part.

## Phasing

A permanent that phases out (CR 702.26) is treated as though it does not exist until it phases in; `phasing.rs` holds the rules.

- A phased-out permanent is in `Zone::PhasedOut` beside the battlefield, still an object with its `PermanentState`, but nothing sees it as on the battlefield. `PermanentPhasedOut` and `PermanentPhasedIn` change the zone and `PermanentState::phased`; attached Auras, Equipment and Fortifications follow (`indirect`).
- The untap step begins with `phasing::in_events` for the active player (CR 502.1), then the Phasing keyword phases out.
- `TriggerPattern::PhasedOut` looks back; `PhasedIn` is judged after. An effect passes over a named permanent that has phased out. Phased-out permanents are public ([protocol.md](protocol.md)).

## Extra turns and phases

The turn sequence is the fixed run of steps (`turn.rs`, `steps.rs`) with two queues over it (CR 500.7, 500.8).

- `GameState::extra_turns` holds the extra turns to take (`ExtraTurn { player, skip_untap }`), the latest taken first; `TurnState::ahead` lists the phases still to begin, added ones first. A skipped untap step does not begin (CR 614.10); `GameRule::SkipExtraTurns` skips extra turns (`ExtraTurnSkipped`). `rmt8_turns` counts each player's turns (`Amount::TurnsTaken`), `rmt8_lives` the life totals at turn start (`Amount::LifeAsTurnBegan`), `rmt8_upkeeps` the two latest upkeeps, which `Pred::FoughtSinceYourLastUpkeep` compares with the turns a permanent last attacked, blocked and became blocked in. They are hashed only when used.

## Combat forecast

`forecast::combat_forecast(&GameState) -> CombatForecast` says what the combat damage still ahead would do if dealt now. It is derived from the state on every call and never stored.

- It plays the damage steps on a copy with the real code: `damage_events`, the fold, state-based actions, the game-end check and housekeeping, every batch through `replace::process`. A question is answered with its first option, as the server's clock does; `depends_on_choice` is set when another option would change the numbers.
- Per attacker: the damage it assigns; per creature in combat: damage taken and whether it leaves the battlefield; per attacked player: the damage, life after (with lifelink gain) and whether it is lethal.
- `pending_combat_forecast` is the guarded form views use: `None` before attackers are declared, with no attacker left and after the last damage.

### Preview

`Engine::preview_forecast(seat, &answer)` (`preview.rs`) is the forecast of a pending DeclareAttackers or DeclareBlockers answer without giving it.

- The answer goes through `answers::handle` (the validation and events of `apply`); the events pass the replacement step, fold onto a clone and `pending_combat_forecast` reads it.
- `PreviewError`: `Illegal`, `NotYourDecision`, `NotPreviewable`, `NoDecision`, `NoCombat`, `Engine` (the copy refused the events, a bug).
- It takes `&self`: nothing is logged, no random number drawn, the hash unchanged.

## Verification

- Invariants (`InvariantChecker`, `mtg-testkit`): per-event checks on every event; whole-state checks and the replay hash comparison every 25 answers and at game end. Every bot-game campaign runs under every invariant and replays a seeded game twice.
- Scenarios (`Scenario`, `Driver`) set up a state, script answers and assert; mechanics are tested over tables of cases at two to four seats. A new decision, event or view field comes with a table test, a fuzz campaign over a deck that uses it and a `hidden_info` case in `mtg-protocol` if it can leak.
- Fuzz: seeded bot games at 2, 3, 4 and 6 players over the seeds `0..n`. `MTG_FUZZ_GAMES` (`fuzz_games` in `tests/common/seeds.rs`) sets n for a long run (`MTG_FUZZ_GAMES=500 cargo test -p mtg-testkit --test fuzz`) and `MTG_FUZZ_FROM` shifts the seeds. Seeds run on one thread when `RUST_TEST_THREADS` is set, else on every core (`MTG_SEED_WORKERS` overrides).
- Runaway games: `Watchdog` counts the answers of a game and a turn and a bot game's permanents, and panics with seed, turn, last answers and decks. Bots stop casting and activating above `CROWDED_TABLE` permanents and `RandomBot::choose_in` holds back a repeated free activation that changes nothing. `PlayOptions::runaway_fails` ends a stopped game as `FailureKind::Runaway(report)` instead.
- Smoke gate: `smoke.rs` plays one card in bot games at 2 and 4 seats per seed (`smoke_card`, [cards-dsl.md](cards-dsl.md)) with bots that favour it, under `runaway_fails`, and counts its uses from the log.
- Crowded tables: an answer costs time in proportion to the permanents; the layers (`LayersCache`), `block_options`, attachments and trigger detection (`Watchers`) must not scan per permanent. `crowded_turn.rs` bounds this.
- `RandomBot::new` keeps its choices fixed (cancel when allowed, automatic exact payments) because seeded fixtures such as the web transcript replay its games; `with_partial_payments` and `with_symbol_choices` opt into more.
- Independent checks: the payment functions against an exhaustive search (`mana.rs`, `spend_tests.rs`, `help_tests.rs`), the forecast against bot games, colour identity against Scryfall, and a replay at every point against the live hash (`pipeline_log.rs`).
- `mtg-testkit` tests live in family binaries (`tests/<family>/<mechanic>.rs`: abilities, casting, combat, fuzz, game, mechanics, objects; `tests/fuzz/<mechanic>_fuzz.rs` for a bot-game campaign); each file opens with its `//! Goal:` line.

## Rules coverage

Deviations from the Comprehensive Rules:

- The deathtouch mark lasts until damage wears off at cleanup; CR 704.5h looks only at damage since the last check, so a creature indestructible when checked that loses it later in the turn would not die. Here it does.
- A creature deals damage in the regular step when it has double strike or no first strike at that step; CR 510.4 reads the keywords as the first step began (they differ only when a keyword changes between the steps).

Readings of the rules:

- The legend rule asks one question per name and player and puts the permanents away as each is answered, so two names are two batches (CR 704.3 makes them simultaneous).
- An Aura spell is rechecked as it resolves with the targeting rule, so a creature that gained hexproof stops it; an Aura entering uncast is not targeted and may enchant a hexproof creature.
- A state trigger (CR 603.8) is looked for where a player would receive priority, not continuously: a state true only during a resolution does not trigger it, and once triggered it goes on the stack whatever the state is by then.
- A card outside the battlefield has its printed abilities only: only a permanent gains or loses any. A static ability working from another zone has timestamp 0, and an ability working from a hand or library names its source to every seat as it goes on the stack ([protocol.md](protocol.md), Visibility).
- An additional triggering is not repeated for an ability that triggers only once each turn: the limit counts the ability, not the events.
- A creature that must block and could still be added to the declaration must be in it; the maximum of requirements (CR 509.1c) is judged by adding creatures, never by moving blockers.
- A granted static ability applies only in the layers after layer 6: the rules do not say what the earlier layers of a granted ability do.
- There is no damage assignment order (CR 509.2, 510.1c): lethal damage matters only for trample, whose excess needs lethal damage on every blocker, counting marked damage and deathtouch but not damage other creatures assign in the same step.
- A mana ability resets the passes in a priority round, a literal reading of CR 117.4.
- Triggers that cannot differ (one source, one ability) go on the stack in the order they triggered; no choice is visible.
- "A player wins the game" makes every other player lose at once (CR 104.2a, 104.2b); one effect naming two winners leaves no one and the game is a draw.
- The choice of how to pay a hybrid or Phyrexian symbol is made with the payment, after the total cost is known, not among the announcements of CR 601.2b.
