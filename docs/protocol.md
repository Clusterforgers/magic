# Protocol notes (M2b)

Design record for `mtg-protocol`. Rules of the house are in `CLAUDE.md`; engine notes in `engine.md`.

## Shape
- `ClientMessage`: Join, Ready, Answer, Chat, Concede, Resync. `ServerMessage`: Welcome, Decision, Events, Snapshot, Chat, Error, GameOver. Flat JSON tagged with `type`; `PROTOCOL_VERSION` rides in Join and Welcome.
- `StateView::for_player(&GameState, PlayerId)` is the only way game state leaves the server. `project_events(before, events, viewers, from_seq)` returns one `EventBatch` per viewer (`EventView::Public | Hidden`).
- Bindings come from ts-rs: 65 files in `web/src/protocol/generated/`. Regenerate with `MTG_UPDATE_BINDINGS=1 cargo test -p mtg-protocol --test bindings`; the drift test fails when the committed files are stale. `mtg-core` and `mtg-engine` derive `TS` behind an optional `ts` feature that only this crate enables.

## Visibility
- Libraries show a count to everyone. Hands list their cards only to the owner. Battlefield, stack, exile, graveyards and command zones are public. The view never carries the seed, RNG position or library order.
- `MulliganTaken` and `LibraryShuffled` are hidden even from their owner (they carry library order). Other seats see draws, opening hands and bottomed cards as counts without ids.
- The redaction `match` names every `Event` variant, so adding one forces a visibility decision. `zone_is_visible` is the single zone rule; a face-down rule belongs there once the engine has face-down cards.

## Contract for `mtg-server`
- Pass `project_events` the state from before the batch was applied; it folds the events over a clone to judge what each object was.
- Send a `Decision` only to the seat it names, a `Snapshot` on join and on `Resync`, and reject answers whose `DecisionId` is stale.
- Never serialize `GameState` or a raw `Event` to a client.

## Hidden-information test
- `tests/hidden_info.rs` plays seeded 2, 3 and 4-player bot games with unique card names and searches every viewer's event batches (each answer) and snapshots (every 20 answers) for hidden names and ids.
- It skips JSON keys that hold seat numbers or counters (`SCALAR_KEYS`), so a new numeric field can fail it until its key is listed.

## Revisit
- `EffectCreated` is passed through as public; revisit when effects can reference hidden objects.
- No face-down cards, no spectator seat and no "waiting on player X" message yet.
- A 4-player board with 175 visible objects serializes to about 53 KB (bounded at 64 KiB in a test); snapshots do not grow with libraries or opponents' hands.
