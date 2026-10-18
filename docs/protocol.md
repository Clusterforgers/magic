# Protocol notes (M2b, M2e)

Design record for `mtg-protocol`. Rules of the house are in `CLAUDE.md`; engine notes in `engine.md`.

## Shape
- `ClientMessage`: Join, Ready, Answer, Chat, Concede, Resync. `ServerMessage`: Welcome, Decision, Events, Snapshot, Chat, Error, GameOver. Flat JSON tagged with `type`; `PROTOCOL_VERSION` rides in Join and Welcome and stays 1 (nothing is deployed, so the batch change broke no client).
- `StateView` is what one seat may know: a `TableView` (turn, priority, every player's life, mana and zone lists, battlefield, stack and exile lists, combat, result; everything but the objects) plus `objects`. On the wire it is one flat JSON object. `StateView::for_player(&GameState, PlayerId)` builds it from the same `TableView::of` that batches use, and is the only way a whole state leaves the server.
- `project_events(before, events, viewers, from_seq)` returns one `EventBatch` per viewer:
  - `events`: `EventView::Public | Hidden`, one per log event, for the log and animations. A client never folds them.
  - `table`: the whole `TableView` after the last event. It replaces the client's copy.
  - `objects`: every object visible after the batch that is new to the viewer or whose `ObjectView` differs from how they saw it before.
  - `gone`: ids the viewer could see before the batch and cannot see after it.
- The diff compares the viewer's objects for `before` and for the folded state; it is never read off the events. Several changes name no object: a lord leaving changes every creature's power and toughness, `DamageCleared` resets damage on all of them, a new turn lifts summoning sickness. The batch at `from_seq` 0 lists every visible object because no view precedes it.
- `StateView::apply(&mut self, &EventBatch)` is the reference client fold: replace the table, upsert `objects`, remove `gone`, keep `objects` sorted by id. `StateView::from_first_batch` starts a view from the batch at `from_seq` 0. The web ports `apply` to TypeScript in `web/src/play/view.ts` (see `web.md`).
- Bindings come from ts-rs: 66 files in `web/src/protocol/generated/`. Regenerate with `MTG_UPDATE_BINDINGS=1 cargo test -p mtg-protocol --test bindings`; the drift test fails when the committed files are stale. `mtg-core` and `mtg-engine` derive `TS` behind an optional `ts` feature that only this crate enables. ts-rs inlines the flattened table, so the TypeScript `StateView` is `TableView`'s fields plus `objects`.
- `DecisionKind::Mulligan` carries `free_mulligans`, `bottom_if_keep` and `bottom_if_mulligan` beside `mulligans_taken` and `can_mulligan`, so a client never derives the bottom count from the mulligans taken; `bottom_if_keep` is the `count` of the `BottomCards` decision that follows a keep.

## Visibility
- Libraries show a count to everyone. Hands list their cards only to the owner. Battlefield, stack, exile, graveyards and command zones are public. The view never carries the seed, RNG position or library order.
- `MulliganTaken` and `LibraryShuffled` are hidden even from their owner (they carry library order). Other seats see draws, opening hands and bottomed cards as counts without ids.
- The redaction `match` names every `Event` variant, so adding one forces a visibility decision. `zone_is_visible` is the single zone rule; a face-down rule belongs there once the engine has face-down cards.
- A batch's `table` and `objects` use that rule through the same code as a snapshot, so zone lists in `table` follow it too. `gone` only ever holds ids the viewer saw before: a hand shuffled into the library (a mulligan) is told as ids in `gone`, a drawn card arrives in full in `objects`, and a card that stays hidden appears nowhere, so another seat's draw changes only a count in `table`.

## Contract for `mtg-server`
- Pass `project_events` the state from before the batch was applied; it folds the events over a clone to get the state after and to judge what each object was.
- Send the batch, then the `Decision` it leads to, so the decision's ids are already in the client's view. Send a `Decision` only to the seat it names, a `Snapshot` on join and on `Resync`, and reject answers whose `DecisionId` is stale.
- Client rule: state = the `Snapshot`, then `apply(batch)` for each `Events` message in order. The next expected `from_seq` is the snapshot's `seq`, then the last batch's `from_seq + events.len()`. A batch with any other `from_seq` means the client missed something: send `Resync` and apply nothing until the new `Snapshot` arrives.
- A seat that joins the lobby is sent `Welcome` only (there is no engine yet), so it has no `Snapshot` when the game starts. Its first batch has `from_seq` 0 and lists every object, and it starts its state from that batch. A seat that joins a running game gets `Welcome`, `Snapshot`, its pending `Decision`.
- Never serialize `GameState` or a raw `Event` to a client.

- `PayOptions.can_cancel` says whether `PayChoice::Cancel` is legal, and `PayChoice::Complete(Payment)` sends part of a payment (floating mana and taps) for the engine to finish; a rejected answer leaves the decision pending. A cancelled cast produces no events, so the server sends no `Events` batch and stores nothing, only the caster's `Priority` decision again with a fresh id; the log never records a cast that was not paid.


## Hidden-information test
- `tests/hidden_info.rs` plays seeded 2, 3 and 4-player bot games with unique card names and searches every viewer's event batches (each answer; `events`, `table`, `objects` and `gone` one by one) and snapshots (every 20 answers) for hidden names and ids. A control test plants a hidden card in each part and checks the scan flags it.
- It skips JSON keys that hold seat numbers or counters (`SCALAR_KEYS`), so a new numeric field can fail it until its key is listed.

## Sync tests
- `tests/sync.rs`: in seeded 2, 3 and 4-seat games, every seat's view folded from the batches (after a JSON round trip) equals `StateView::for_player` on the true state after every answer. It also checks `gone` and `objects` against a separate statement of the visibility rule, and that no unchanged object is listed.
- `tests/batch_diff.rs`: hand-built tables through the real engine: an answer that changes one permanent lists one object; effects that name no object (`AllCreatures`, `CreaturesControlledBy`, an expiring `UntilEndOfTurn`, a `WhileSourceOnBattlefield` lord whose owner concedes) still list every creature they change. `tests/batch_visibility.rs`: a draw reaches only the drawer, a mulligan moves cards out of the owner's view and a new hand in, the first batch is complete, and `apply` copes with unsorted objects and unknown ids.
- `tests/transcript.rs` writes `web/src/protocol/fixtures/two-player-seat0.json` (`MTG_UPDATE_BINDINGS=1 cargo test -p mtg-protocol --test transcript`) and fails on drift. It holds two transcripts of seat 0, 40 batches each, in the order `GameActor` sends them (batch, decision, game over; one fresh id per decision): `messages` is a late join (Welcome, Snapshot, pending Decision, the end of the game with GameOver) and `lobby` is a start from seq 0 with mulligans. Each has the seat's true `expected` view. The Rust test and the vitest both fold them. `play()` in the test mirrors the actor, so change it with the actor's message order.

## Revisit
- `EffectCreated` is passed through as public; revisit when effects can reference hidden objects.
- No face-down cards, no spectator seat and no "waiting on player X" message yet.
- No lobby message: before the start a client only sees `Welcome`, so it cannot show who has pressed ready (the actor in `mtg-server` keeps readiness per seat and ignores late `Ready`). Sending `Snapshot { seq: 0 }` at game start would give lobby and late-join seats one rule and make `from_seq` 0 an ordinary batch.
- Decision ids are a per-actor counter that restarts at 1 after a resume; clients must not carry one across a reconnect to a restarted server (a fresh `Decision` always follows the snapshot).
- A 4-player board with 175 visible objects serializes to about 53 KB (bounded at 64 KiB in a test); snapshots do not grow with libraries or opponents' hands.
- A batch carries the whole table, so it costs 1.2 to 1.8 KB whatever the board: in seeded games 11 to 21% of a snapshot of the same moment (2 seats about 1.2 KB of 5.7 to 9.2 KB, 4 seats about 1.75 KB of 14.7 to 15.7 KB; bounded in `tests/batch_size.rs`). The table could become a diff later; `viewer`, `format` and `first_player` repeat in every batch.
- Every batch is projected from two full views per seat (before and after). Cache the previous view per seat in the actor if profiling shows it.
