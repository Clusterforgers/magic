# Engine notes (M2a)

Design record for `mtg-engine` and `mtg-testkit`. Rules of the house are in `CLAUDE.md`; the overall design is in `PLAN.md`.

## Shape
- `Engine::new/resume/apply/concede/decision/state/events/state_hash/add_continuous_effect/checkpoint/restore`, plus `replay(setup, events)`.
- `Decision` kinds carry their own legal options: Mulligan, BottomCards, Priority, PayMana, DeclareAttackers, DeclareBlockers, AssignDamage, Discard. `Answer` mirrors them.
- `MulliganOptions` states the mulligan costs from `FormatRules::mulligan_rule` (`MulliganRule { hand_size, free_mulligans }`): `free_mulligans`, `bottom_if_keep` (the count the `BottomCards` decision will ask for) and `bottom_if_mulligan`. Normal is the London mulligan with no free one; a format with a free mulligan (Commander, CR 103.5c; M5) only sets `free_mulligans`. Clients repeat these counts and never work them out.
- One `Event` enum (~35 variants) and one `apply_event` fold. State is the fold of events.
- `FormatRules` trait with `Normal` implemented; Commander is not started.

## Decisions
- Random choices use a ChaCha8 substream keyed by (seed, counter in state). Events carry the results, so replay needs no RNG.
- A half-finished cast (spell chosen, payment pending) lives outside game state; a rebuilt engine asks for priority again.

- Payment (`mana.rs`, `cast.rs`): `check_payment` and `complete_payment` take the permanents that are tapped and treat a permanent as one tap, whichever of its abilities is used (`ObjectTappedTwice`, `AlreadyTapped`), and `can_afford`/`auto_payment` match coloured needs to distinct permanents, so `auto_available` stays true only when a valid payment exists. `PayChoice::Complete(partial)` validates the partial parts, requires that the chosen mana fits inside the cost (`Wasted` otherwise), and pays the rest with `auto_payment` over the unspent pool and untouched permanents (`CannotComplete` if it cannot). `PayOptions.can_cancel` is true while nothing is paid; `Cancel` then drops the pending cast with no event, so the spell never left the hand and the log shows nothing of it (a rebuilt engine from the log asks for the same priority decision); with the flag false `Cancel` is `NotOffered`. `RandomBot::new` keeps its choices (cancel when allowed, automatic, exact; roughly a third of casts are cancelled) because seeded fixtures such as the web transcript replay its games; `with_partial_payments` (`PlayOptions::partial_payments` for `play_game_with`) adds `Complete`, and `payment_fuzz.rs` plays bot games with it, checking that after a cancel the state hash, the log, the next decision and a replay all agree and that a completed payment taps the chosen permanents.

- Answers are validated before any mutation. Internal fold failures roll back by replaying the log; `checkpoint`/`restore` expose that rollback so the server can undo a step it failed to persist.
- `concede` is legal at any time (CR 104.3a): `PlayerLost { Conceded }`, then the normal elimination path (objects leave, the turn ends if it was theirs, the game ends when one player remains). A half-finished cast is dropped. The mulligan stage runs the same game-end and housekeeping check as turns.
- An effect whose duration has ended stops applying at once; the later `EffectExpired` event only prunes it.
- Invariants: per-event checks run on every event; whole-state checks and the replay hash comparison run every 25 answers and at game end.
- Fuzz: 500 bot games each for 2, 3, 4 and 6 players (`MTG_FUZZ_GAMES` overrides the count). The dev profile builds core, engine and testkit at opt-level 3 to keep this near 17 s.

## Priority options
`PriorityOptions` lists `playable_lands`, `castable`, `mana_abilities` and `can_pass`, and says in `can_act` whether the holder can do anything useful: a land to play, a spell to cast or an ability that is not a mana ability. `castable` already means affordable, at the timing the card allows (sorcery timing needs the active player, a main step and an empty stack; an instant waits for nothing): the floating mana plus the untapped sources the player controls, each permanent counted once (`can_afford`). Mana abilities alone never count, because tapping a land with nothing to spend the mana on is not an action, so after the last land drop with nothing affordable the holder has nothing to do, and tapping that land does not change that. One function holds the rule (`priority::can_act`); `other_abilities` beside it is the seam for abilities that are not mana abilities, empty until a card has one. The server uses the flag to pass for a seat that asked for it and to pick who gets a response clock (`server.md`); `PlayOptions::auto_pass` in `mtg-testkit` applies the same rule in a bot game and is the reference the server tests compare with. `tests/priority_can_act.rs` is the table for the rule (empty hand, lands, land drop used, affordable and one-short creatures, two spells, colour, floating mana, summoning sickness, the opponent's turn, a busy stack) and plays bot games in which every idle holder is passed for.

## Combat forecast
`forecast::combat_forecast(&GameState) -> CombatForecast` says what combat damage would do if dealt now; it is derived from the state on every call and never stored. Per attacker it lists the damage it assigns (one packet per blocker in declared order, or one to the defender when unblocked) and the damage its blockers deal back; per creature in combat the damage it takes and whether it dies (damage marked plus expected damage against derived toughness, as in the state-based action); per attacked player the damage, the life after and whether that is lethal. Where several blockers share an attacker and the controller has not divided yet, the default is lethal damage to each blocker in turn and the rest on the last; once `DamageAssigned` is recorded the forecast uses the recorded division. `pending_combat_forecast` is the guarded form views use: it returns `None` before attackers are declared, with no attacker left, and once damage has been dealt (the combat state lingers until the end of combat, and a damage step that has granted priority is past its damage). Keywords join in `damage::strike_power` (first/double strike, power), `lethal_damage` (deathtouch) and `divide` (trample); `damage::damage_events` is the real counterpart, so change both together. A proptest in `mtg-testkit` checks forecast against reality in bot games.

## Rules checked against the Comprehensive Rules (2026-10-18)
- **Active player leaves mid-turn: engine is wrong.** The rules (multiplayer rule 800.4, the subrule about leaving during one's own turn) let the turn continue to completion without an active player: priority goes to the next player in turn order, or the top object resolves, or the step ends. The engine ends the turn at once. Fix before multiplayer Commander (see ROADMAP M5).
- Attacker divides damage freely among several blockers: correct. The Foundations update removed damage assignment order (509.2, 510.1c); lethal damage only matters for trample, which is not implemented yet.
- Damage to a planeswalker is only marked: a gap. Rule 120.3c turns it into removed loyalty counters, which need loyalty counters first.
- A mana ability resets the passes in a priority round: consistent with a literal reading of 117.4 (all players pass "without taking any actions in between"). No official ruling found; low impact, keep.

## Gaps (roadmap M3 onwards)
- No keywords, triggers, targeting or Commander, and no spell or ability effects beyond permanents entering: scry, search, tokens and the other keyword actions need the choice decisions listed in the M3 roadmap first.
- Layer dependencies (CR 613.8) are not modelled; only type, colour and power/toughness modifications work.
- No tokens or control change.
- The first-strike step exists but never occurs.
- Scenarios cannot start in the blockers or damage steps.
- The first-draw skip applies only to two-player games.
