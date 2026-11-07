# Protocol

Wire protocol of `mtg-protocol`: messages, per-seat state projection, hidden-information rules and the sync contract between `mtg-server` and its clients. Engine types are in [engine.md](engine.md), server behavior in [server.md](server.md). Field-level shapes live in the rustdoc of `crates/mtg-protocol/src/` and the generated types.

## Messages

- Both families are flat JSON objects tagged with `type` (`ClientMessage`, `ServerMessage` in `message.rs`). `PROTOCOL_VERSION` rides in `Join` and `Welcome`.
- `Manual {action}` is a `ManualAction` taken at any time by a seat in the game, apart from any decision ([server.md](server.md), Manual actions); `Answer` with `PriorityAction::Manual` still takes one while the seat holds priority. The engine refuses an action the state does not allow as `Error{IllegalAnswer}`.
- TypeScript bindings come from ts-rs into `web/src/protocol/generated/`. `MTG_UPDATE_BINDINGS=1 cargo test -p mtg-protocol bindings::` regenerates them, and the drift test fails while they are stale. `mtg-core` and `mtg-engine` derive `TS` behind an optional `ts` feature that only this crate enables.
- Optional fields are left out of the JSON while empty or false (TypeScript `field?:`), so the wire form of a game that never uses a feature is unchanged. A client treats absence as the empty value.

## State projection

- `StateView` is what one seat may know: a `TableView` (turn, priority, players, zone lists, combat, forecast, result) plus `objects`, as one flat JSON object. `StateView::for_player` is the only way a whole state leaves the server; it builds the table with the same `TableView::of` as batches. `GameState` and raw `Event` are never serialized to a client.
- A spectator is a viewer that is no seat: `spectator_viewer(state)` is the number of seats, which names no seat, and any viewer id past the last seat gets the same view. `TableView.spectator` is true in every table it gets. It sees every public zone, object and event a seat sees and no seat's hidden cards (no hand unless the table has open hands, no library, only counts).
- `project_events(before, events, viewers, from_seq)` returns one `EventBatch` per viewer:
  - `events`: `EventView::Public | Hidden`, one per log event, for the log and animations. A client never folds them.
  - `table`: the whole `TableView` after the last event. It replaces the client's copy.
  - `objects`: every visible object that is new to the viewer or differs from how they saw it before.
  - `gone`: ids the viewer could see before the batch and cannot see after it.
  - `shown`: the cards an event of the batch showed the viewer that the viewer cannot see after it, each as the `ObjectView` it had right after that event (a revealed hand that stays a hand, a card looked at and bottomed). `StateView::apply` ignores it.
- The object diff compares the viewer's objects before and after and is never read off the events, because changes can name no object (a lord leaving changes every creature, a new turn lifts summoning sickness). The batch at `from_seq` 0 lists every visible object.
- `StateView::apply` is the reference client fold: replace the table, upsert `objects`, remove `gone`, keep `objects` sorted by id. `StateView::from_first_batch` starts a view from the batch at `from_seq` 0. The web ports `apply` to `web/src/play/view.ts`.
- Values the client needs are worked out by the server and sent, never derived by the client from events or rules: commander tax and damage and the lethal amount (`PlayerView.commander_lethal`), `can_act`, `manual_owed`, `summoning_sick`, `targets_legal`, the worded `activated` abilities with their `ways`, `gives`, `combat_forecast`, the mulligan bottom counts.
- `TableView.house_rules` is the `HouseRules` the creator set ([engine.md](engine.md), House rules), the same for every seat.
- `ObjectView.manual` lists the clauses of an object's rules text that the engine does not run to every seat that sees the object; `ObjectView.manual_owed` is the flag by which the server holds the seat at its by-hand window ([server.md](server.md), Stops), so a client never reads the clauses to decide it. `partial` tells a `Partial` card from a `Manual` one; `read` marks a card that runs on a player's read. `AbilityLeftToTable` is public to every seat that sees the source.
- `ObjectView.identity` (`CardIdentity {oracle_id, printing_id?}`) lets a client find the card's image and rules text; the view carries no URL. It reaches only the viewers who identify the object. The server sets it on every card it reads from the database (`CardDefinition.identity`, never serialized, so deck fingerprints do not see it).
- An ability on the stack is an `ObjectView` of its own with `kind` `Ability`, named after its source, with its `targets`; a copy of a spell is `kind: Token` with `copy.of`. `targets_legal` has the shape of `targets` and follows the check of the resolution.
- `TableView.combat_forecast` is public, derived by `TableView::of` from `pending_combat_forecast`, so it rides in `table` and `apply` needs no extra rule.
- `DamageOptions`, `BlockOptions`, `AttackOptions` carry what a rule requires or limits (lethal damage, minimum blockers, required attackers, most attackers), so a client never computes it; the declaration must name the required creatures or the engine refuses it.
- A batch carries the whole table, which keeps the client fold trivial; projection builds two full views per seat per batch.

## Visibility

- Libraries show a count to everyone; hands list their cards only to the owner; battlefield, stack, exile, graveyards and command zones list their cards to everyone, except that a face-down card is identified to the seat that may look at it alone. The view never carries the seed, RNG position or library order.
- `zone_is_visible` is the single zone rule, used for snapshots and for a batch's `table` and `objects` alike; whether a listed card is identified is judged object by object (`Sight::look` in `project.rs`, asking the engine's `GameState::may_look_at`), so every face-down rule lives in one place.
- A seat also identifies the cards its resolution showed it and the cards it is asked about, wherever they are (`Sight`, the engine's `GameState::is_shown_to` and `sight::offered_cards`): looked at, found, scried or revealed cards, and the cards of a pending `ChooseCards`, `OrderCards` or `Scry`. Such a library card keeps zone `Library` and is in no zone list. It leaves the view (`gone`) when the resolution ends, the question is answered or the card changes zones. The same function builds snapshots and batches, so a seat that reconnects mid-question gets the same objects. The owner of a library learns nothing from another seat's look at it.
- The top card of a library whose owner plays with it revealed is public (`PlayerView.revealed_top`); a draw of it is still a `HiddenEvent::CardDrawn` to the other seats.
- House rules change what is revealed, through `zone_is_visible_in` and the engine's `revealed_top`, and nothing else: `open_hands` lists every hand to every view and makes hand events whole (library order stays hidden); `revealed_library_top` names the top of every library; `opening_hand_smoothing` shows the deal as one `LibraryShuffled` then `HandDealt` and the discarded hand appears nowhere; `mulligan_style` `vancouver` lists the scried card to that seat only.
- `redact` names every `Event` variant, so adding one forces a visibility decision. An effect that references a hidden object needs its own rule. The classes:
  - Hidden even from the owner because they carry library order: `MulliganTaken`, `LibraryShuffled`, `LibraryRefilled`.
  - Private to one seat: `LibraryLooked`, `HandLooked`, `CardsFound`, `Scried`, `Surveilled` reach the seat that did them as logged (`HandLooked` also the owner of the hand) and every other as `HiddenEvent` with counts (whole in a zone the viewer sees into).
  - Whole only when everything the event names is visible to the viewer: `ZoneChanged` (otherwise `HiddenEvent::ZoneChanged` with the zones and the cause), `ChoiceMade` (the answering seat always sees it), `StepDone`.
  - Whole only when the card they are about is visible: `AbilityTriggered`, `AbilityPutOnStack` of a triggered ability, `TriggerRemoved`, `DelayedTriggerCreated`, `AbilityLeftToTable`. A trigger whose source names a card in a zone the viewer cannot see becomes a `HiddenEvent`.
  - An ability whose source is a card in a hand or library names that card to every seat when it goes on the stack, as at a table; `tests/it/hidden_info.rs` allows those cards and no other hidden one.
  - `ManualAction` is public as logged, except a `Move` between places the viewer cannot see into (`HiddenEvent::ManualMove`); an `ArrangeTop` and a `Tutor` name cards only their seat knows (`ManualArrange`, `ManualTutor`); a `Reveal` and a `Note` name what every seat may know.
  - Public as logged: the other ability and resolution events, life, counters, tokens, `Entered`, `Attached`, `Paired`, mana events, `Revealed`, `Crewed`, and the like.
- A card a reveal named is known to every seat, so a choice among revealed cards reaches them whole.
- A keyword a static ability grants to a card in a hand is read in that card's `keywords` for its owner alone; the ability it adds names its source card when it goes on the stack, as a printed one does.
- Face-down objects (foretold, hidden and sealed cards, face-down spells and creatures, manifest, cloak) are listed in their zone for every seat. A sealed card (`ZoneCause::Sealed`, CR 406.3) is looked at by no seat, its owner included, unless a `LookAtExiled` rule in force names the seat. The seat that may look gets the whole `ObjectView` with `face_down` set; every other seat, a spectator included, gets owner, controller and zone and no characteristics, except a face-down creature, which shows its 2/2 characteristics and no name. Its events follow the same rule; when it changes zone it is a new object. A manifested or cloaked card is judged by the seat that controls it after the move (`ZoneCause::enters_under`), not by the owner of the library it came from.
- Phasing, prepared, Room doors, merged permanents, counters, attachments, monarch, initiative, day and night, extra turns, commander and player counters are public designations: every seat gets the same values and no id names an object the viewer cannot see.
- A companion is visible as existing to every seat; its `name` goes to the owner from the start and to every seat once revealed. `CompanionBrought` is public and the card is an object in the owner's hand from then on.
- `gone` holds only ids the viewer saw before. A hand shuffled into the library is told as ids in `gone`, a drawn card arrives in full, and a card that stays hidden appears nowhere, so another seat's draw changes only a count in `table`.
- Hidden-information tests: `tests/it/hidden_info.rs` plays seeded 2, 3 and 4-seat bot games with unique card names and searches every viewer's batches and snapshots for hidden names and ids. Control tests plant a hidden card in each part to prove the scan can fail. JSON keys that hold seat numbers or counters are skipped (`SCALAR_KEYS`), so a new numeric field fails the test until its key is listed. Each view family has a `tests/it/*_view.rs` or `*_hidden.rs` file at 2 to 4 seats and for a spectator.

## Sync contract

- A socket opens with one of two credentials, both checked before the upgrade: `/ws/{game}/{token}` (a seat's link; the token names the seat) or `/ws/{game}` with the `mtg_session` cookie (a signed-in user's own seat). The cookie rides the upgrade request because scripts cannot read an `HttpOnly` cookie; the server's `Origin` check applies.
- `Join` follows either way: `game` must equal the game in the address and `Welcome.player` says which seat the credential opened. A client never names its seat.
- The server passes `project_events` the state from before the batch; it folds the events over a clone to get the state after and to judge what each object was.
- It sends the batch, then the `Decision` it leads to, so the decision's ids are already in the client's view. A `Decision` goes only to the seat it names, and answers with a stale `DecisionId` are rejected.
- Client rule: state is the `Snapshot`, then `apply(batch)` for each `Events` message in order.
  - The next expected `from_seq` is the snapshot's `seq`, then the last batch's `from_seq + events.len()`.
  - Any other `from_seq` means the client missed something: send `Resync` and apply nothing until the new `Snapshot` arrives.
- `Snapshot.log` holds the latest events (at most `SNAPSHOT_LOG`) as the seat saw them, so a reload reads the game log again; the event at index `i` has position `seq - log.len() + i`. `project_log` builds it by replaying the events ahead of the window and judging each event against the state it happened in, so the views equal the batches a seat connected all along was sent. `Snapshot.names` holds the names of objects the seat saw during the window that the state after no longer names (`SNAPSHOT_NAMES`, oldest first out), taken only where the seat may identify the object; `project_window` builds both in one pass. A client applies the log with `mergeHistory` (what it holds stays; a snapshot is state, never a batch to animate).
- A seat that joins the lobby gets `Welcome` and the roster but no `Snapshot`; it starts from the first batch at `from_seq` 0. A seat that joins a running game gets `Welcome`, `Snapshot` and its pending `Decision`.
- Decision ids come from a per-game counter that restarts at 1 after a resume. A client never carries one across a reconnect to a restarted server; a fresh `Decision` always follows the snapshot.
- Tests: `tests/it/sync.rs` checks that every seat's folded view equals `StateView::for_player` on the true state after every answer; `transcript.rs` and `preview_fixture.rs` write the fixtures in `web/src/protocol/fixtures/` that the web tests replay (`MTG_UPDATE_BINDINGS=1 cargo test -p mtg-protocol transcript::`, likewise `preview_fixture::`). The transcript test's `play()` mirrors the actor's message order, so change them together, and mirrors a seat with auto-pass off.

## Lobby

- `Ready` says the seat is set to play and `NotReady` takes that back. Both are ignored once the game runs. A seat without a deck cannot press Ready (`Error{BadMessage}` to that seat alone); a ready seat keeps its deck until `NotReady`. The deck is chosen over REST, never over the socket.
- `Lobby { seats, needed }` is the roster before the start, sent after `Welcome` and to every connected seat when a seat joins or leaves or a ready flag changes. A message that changes nothing sends nothing, and the `Ready` that starts the game is answered by the opening `Events` instead. A client drops its lobby view when the first batch or a snapshot arrives.
- A seat's `deck` is `SeatDeck {name, format, manual_cards}`: the deck's name, format key and the number of copies the game does not run whole ([deck-check.md](deck-check.md), Decisions), never the name of a card.

## Decisions

- Every decision of a spell or ability carries `source`, the object that asks, and has an `Answer` of the same name. Prompt text (`text`, `wording`) is in the words of a card, written by the server; the web has prompts for each kind and the server has their clocks.
- A `Cancel` answer backs out of a cast with no event and needs `can_cancel`; a triggered ability cannot be backed out of, and its `ChooseTargets` answers `Remove` instead (CR 603.3d). A cancelled payment sends no `Events` batch, only the caster's `Priority` decision again with a fresh id. The log never records a cast that was not paid.
- `ChooseReplacement` goes to the player an event happens to when two or more replacement effects apply; the answer is not an event, and a replaced event is never in the log.
- `PriorityOptions.unplayable` lists, for the holder's own hand and command zone, each card the offer leaves out with a reason; only the holder gets options, so it tells nothing of another seat.
- In the opening every seat gets its own `Decision` at once, each with its own id and clock; whether a seat has kept is public in `PlayerView.mulligan`.
- `Decision.deadline_ms` is the time the seat has left, in milliseconds, when the message is sent (a duration, so the client counts from arrival); a decision sent again after a reconnect or `Resync` carries what is left. It is omitted when there is no clock. The kinds and lengths (`MTG_RESPONSE_SECS`, `MTG_BLOCK_SECS`, `MTG_THINK_SECS`, `MTG_ACTIVE_SECS`) are in [server.md](server.md), Response clocks.
  - The priority clock ignores `can_act`: a clock that depended on it would tell the table what a hand holds.
  - At zero the server answers for the seat (a pass, no blocks, keeping the hand) through the normal path; the decision id goes stale and the events arrive as a batch, so a client drops the prompt at zero and expects them.
- `Decision.show_after_ms` is when the client starts to show the countdown (`MTG_CLOCK_GRACE_SECS`); the clock runs from issue all the same. It follows the kind and the seat as the clock does, never whether the seat can act.
- A seat that holds an extension spends it at the deadline instead of being answered for: it is sent the same `Decision` again (same `id`) with the new `deadline_ms`, and every seat is sent `Bank { seat, bank }`. `PlayerView.bank` is public; the engine does not know it, the server writes it into every table.
- `PlayerView.connected` is public and never stored, hashed or put in the event log, so replays do not see it. The server writes it into every snapshot and batch and sends the other seats `Presence { seat, connected, deadline_ms? }` when a seat gains its first connection or loses its last, and again for an away seat when it is issued a decision or its clock is extended. A seat is never told of itself.

### Payments

- `PayOptions.can_cancel` says whether `PayChoice::Cancel` is legal. `PayChoice::Complete(Payment)` sends part of a payment for the engine to finish; a rejected answer leaves the decision pending.
- Mana that may be spent only on some things, and mana that stays past the step, are public facts of the pool and the events; payment help (convoke, improvise, delve) is offered beside the mana sources and its taps are public events. The rules are in [engine.md](engine.md), Payment.
- Life pays Phyrexian symbols: `PayOptions.life` is the payer's life and `max_life` the most this payment may take; the cast's events start with one public `LifePaid` before `SpellCast`.

## Pace of play

### Auto-pass

- `AutoPass { enabled }` maps to the Full Control mode `Held` when false and `Off` when true (a seat in `NextWindow` stays in it); the change is told as a `FullControl` message. `Welcome.auto_pass` is true unless the seat is in `Held`.
- A client must not rely on seeing every priority window: outside Full Control the server asks a seat only at its windows (rules in `windows.rs`, [server.md](server.md), Stops). The events of passes arrive in the batch of the answer that led to them, except the pass for a seat that cannot act in a combat window or with a spell on the stack, which comes in a batch of its own after a wait ([server.md](server.md), Dwell), so the pace does not tell the table what a seat holds.

### Yields

- `Yield { until }` (`Resolve`, `EndOfTurn`, `MyTurn`) asks the server to pass priority for the seat until something happens; `CancelYield` ends it and a new `Yield` replaces the old one. While a yield lasts the server answers each `Priority` decision of the seat with a pass, through the same path as an answer, and never sends it.
- It ends when an opponent puts an object on the stack the seat has not passed on yet; its target is reached (`Resolve` when the stack is empty, `EndOfTurn` when another turn has begun, `MyTurn` when another turn of the seat's own has begun); the seat holds a stop at a window (not in the step it asked at); or the seat leaves, the game ends, or `CancelYield` arrives. Other decisions (attackers, blockers, discard, payment, damage division) are always sent and leave the yield as it is.
- The end rules look at the table only, never at what the seat could do, so the pace reveals nothing about a hand (`yields.rs` in `mtg-server`). A client asks for `Resolve` only over a stack: over an empty one it ends at once and the same window returns with a new id.
- Yields live in memory per seat, so a resumed game has none. A seat that is out of the game or in Full Control, or a game that has not started or is over, gets `Error{BadMessage}`; turning Full Control on ends a yield.
- `Yielding { until }` (`null` for none) reports every change, whatever the cause; `Welcome.yielding` gives it at join. It follows the events of the step that changed it and precedes the `Decision` that follows. A client that sees `Yielding` with a yield drops the `Priority` decision it still shows. A yielding seat is sent no `Priority` decision, except when the server cuts a run of 500 passes in one step: the yield ends first and `Yielding` says so.

### Stops and Full Control

- A `Stop { side, step, keep }` is a step at which a seat asks to be given priority; `side` is `Own` or `Opponent`, `keep` false is a one-turn stop and true a lasting one. `SetStop { side, step, on, keep }` sets or clears it; a seat holds one stop per side and step. Only a step where a player gets priority can hold a stop: `Untap` and `Cleanup` are refused with `Error{BadMessage}`, which changes nothing and needs no resync.
- `FullControl { mode }` (`Off`, `NextWindow`, `Held`) sets how much of the game the seat plays by hand: `NextWindow` until the seat has passed once, `Held` until it turns the mode off. `EndTurnByHand { on }` and `AutoChoices { choices }` (`order_triggers`, `assign_damage`: the server answers with the plain answer; Full Control asks both anyway) are set and reported like the mode.
- `Stops { stops }` (the whole list, each side and step once) and `FullControl { mode }` report what the server holds whenever it changes, whatever the cause, after the events of the step that changed it and before the `Decision`. A request that changes nothing gets silence; one that only changes a held stop's `keep` is a change. `Welcome.stops`, `full_control`, `end_turn_by_hand` and `auto_choices` give the values at join.
- All of them live in memory per seat from the lobby on and survive reconnects and `Resync`; a resumed game starts with no stop and the server's default mode. A seat that leaves, and every seat once the game is over, loses them and is told before its decision or `GameOver`; later requests get `Error{BadMessage}`.
- The server asks a seat at a priority decision at every window in `NextWindow` and `Held`; at a stop whether the seat can act or not; with end turn by hand on, at its own end step (this never ends a yield); and at the default windows only if it can act (own main phases, declare attackers and blockers steps, any moment an object of another seat is on the stack). Elsewhere it passes for the seat, after the wait above except in its own main phase with an empty stack.
- The web saves pinned stops and the end-turn choice per account (`GET/PUT /api/me/prefs`, `UserPrefs`) and in `localStorage` for a guest (`play/flowPrefs.ts`, `play/usePrefsSync.ts`), and sends the default stops once per game at `Welcome` to a seat that holds none.
- Quick cast: a click on an instant or flash card in hand with no decision on screen sends `FullControl { NextWindow }` and casts at the next `Priority` decision if the card is in `castable`. `NextWindow` asks whatever the seat holds, so the table learns nothing (`pace_leak.rs`).

## Preview

- `Preview { decision, answer }` asks what combat damage would do if `answer` were given to the pending DeclareAttackers or DeclareBlockers decision `decision`, without giving it. `Forecast { decision, forecast }` is the reply, `forecast` a `CombatForecast` or null.
- The engine builds it (`Engine::preview_forecast`), so it equals the `combat_forecast` every table carries right after the same answer is really given. It describes the moment of the declaration; the batch that follows can run past it.
- Exactly one `Forecast` answers each `Preview`, to the seat that asked, in order. It is null when there is none to give (not that seat's pending decision, not an attack or block, the engine would refuse the answer, the seat previews faster than the server allows); a client cannot tell the reasons apart and does not need to.
- A preview never produces an `Error`: the stale-decision treatment of `Answer` (a notice and a `Resync`) would be wrong for advice that routinely races with the real answer. A seat can preview only its own pending legal options, so it learns nothing it could not learn by answering.
- A client matches replies to requests by order, keeps the reply to its latest request and ignores one whose decision is no longer pending. A `BadMessage` reply means the server does not know previews; the web client then stops asking.

## REST bodies

The account, card, deck, precon, game-browser and preference endpoints of `mtg-server` take and answer the types of `mtg_protocol::rest` (`rest/*.rs`), so the web gets them from the bindings. Rules that hold across them:

- Bodies refuse unknown fields; any type holding a password prints it as `<redacted>` in `Debug`; a `Joined` never prints its token.
- Each family has an error-code type (`AuthErrorCode`, `DeckErrorCode`, `LobbyErrorCode`, `DeckChoiceErrorCode`, `PreconErrorCode`) whose `error` string is snake_case; `as_str` is the wire text, `ALL` lists every code, and a test keeps `as_str`, serde and the generated union equal. `DeckChoiceErrorCode` is a type of its own so that clients built before it still handle every `LobbyErrorCode`.
- Limits (username and password length, deck and import sizes, page sizes, note lengths) are constants of `mtg_protocol::rest`, so server and tests share one number; ts-rs does not export constants, so the web repeats them for its forms and the server stays the judge.
- Timestamps are `YYYY-MM-DDTHH:MM:SSZ` (UTC); a list that shows an age sends `age_secs` at the moment of the answer so a client counts on from its own clock.
- A deck is named by its stored id, a built-in deck's id or `PRECON_DECK_PREFIX` (`precon:`) and the precon code wherever a request names a deck. A deck check that names a `game` or a `table` (`TableRules`) answers the verdict at that table, never both.
- A blocked deck answers `DeckBlockedBody` with the `Issue`s of the check; they name cards and go only to the account whose deck it is.
- Lobby rows and `GameInfo` carry no token, join code or link; a request that carries a seat token sends it as `Authorization: Bearer <token>`. `POST /api/games` has a hand-written body on both sides (`web/src/net/api.ts`); a `house_rules` value out of range is 400 `invalid_request` with the rule's name in `field`.
- The picture endpoints answer image bytes, not JSON ([server.md](server.md), Pictures): `404 no_image` means the web draws the text face, `502 image_unavailable` that the CDN failed. A client draws pictures through them, never from the Scryfall links in `ImageLinks`.
- `rest_*.rs` and `seat_deck.rs` in `crates/mtg-protocol/tests/it/` check each body's round trip, JSON keys, error-code spelling and bindings.
