# Engine notes (M2a)

Design record for `mtg-engine` and `mtg-testkit`. Rules of the house are in `CLAUDE.md`; the overall design is in `PLAN.md`.

## Shape
- `Engine::new/resume/apply/concede/decision/state/events/state_hash/add_continuous_effect/checkpoint/restore`, plus `replay(setup, events)`.
- `Decision` kinds carry their own legal options: Mulligan, BottomCards, Priority, PayMana, DeclareAttackers, DeclareBlockers, AssignDamage, Discard. `Answer` mirrors them.
- One `Event` enum (~35 variants) and one `apply_event` fold. State is the fold of events.
- `FormatRules` trait with `Normal` implemented; Commander is not started.

## Decisions
- Random choices use a ChaCha8 substream keyed by (seed, counter in state). Events carry the results, so replay needs no RNG.
- A half-finished cast (spell chosen, payment pending) lives outside game state; a rebuilt engine asks for priority again.
- Answers are validated before any mutation. Internal fold failures roll back by replaying the log; `checkpoint`/`restore` expose that rollback so the server can undo a step it failed to persist.
- `concede` is legal at any time (CR 104.3a): `PlayerLost { Conceded }`, then the normal elimination path (objects leave, the turn ends if it was theirs, the game ends when one player remains). A half-finished cast is dropped. The mulligan stage runs the same game-end and housekeeping check as turns.
- An effect whose duration has ended stops applying at once; the later `EffectExpired` event only prunes it.
- Invariants: per-event checks run on every event; whole-state checks and the replay hash comparison run every 25 answers and at game end.
- Fuzz: 500 bot games each for 2, 3, 4 and 6 players (`MTG_FUZZ_GAMES` overrides the count). The dev profile builds core, engine and testkit at opt-level 3 to keep this near 17 s.

## Rules to verify (agent was unsure)
- Active player eliminated mid-turn: the turn ends at once. If the first player concedes during the mulligans, turn 1 is abandoned and the next living seat takes turn 2.
- Attacker divides damage freely among several blockers (damage assignment order is not modelled).
- Damage to a planeswalker is only marked; loyalty counters do not exist yet.
- A mana ability resets the passes in a priority round.

## Gaps (roadmap M3 onwards)
- No keywords, triggers, targeting or Commander.
- Layer dependencies (CR 613.8) are not modelled; only type, colour and power/toughness modifications work.
- No tokens or control change.
- The first-strike step exists but never occurs.
- Scenarios cannot start in the blockers or damage steps.
- The first-draw skip applies only to two-player games.
