# Server notes (M2c)

Design record for the `mtg-server` game actor. Rules of the house are in `CLAUDE.md`; the wire contract is in `protocol.md`.

## Shape
- `spawn_game(id, setup, store)` starts a game in its lobby; `resume_game(store, id, setup)` rebuilds one from its log. Both return a cheap-clone `GameHandle` (`join(seat) -> Receiver<ServerMessage>`, `send(seat, ClientMessage)`, `leave(seat)`, `status()`) and must run inside a tokio runtime.
- One task per game owns the `Engine`, the `EventStore` and a bounded queue per seat. A full or abandoned queue drops that client, which reconnects and receives a snapshot; the actor never waits for a client.
- Lobby: only joins and `Ready`. The last `Ready` creates the engine and broadcasts the opening events from seq 0. A resumed game goes straight to running; one with an empty log returns to the lobby.
- One step: check seat, `DecisionId` and engine legality, run `apply` or `concede`, project for the connected seats, append to the store, then send the events, the next `Decision` (only to the seat it names) and `GameOver`. A store or projection failure restores the engine from its `Checkpoint` and answers `Error{Internal}` without details.
- Join and `Resync` send `Welcome`, a `Snapshot` and the seat's pending decision. The game waits for disconnected seats; there is no auto-pass.

## Contract for the transport
- The actor trusts the seat it is given. The transport must map a credential to exactly one seat and never let a client name its own.
- A replaced socket drops its receiver instead of calling `leave`, which closes whatever connection the seat has at that moment.
- `status()` exposes the state hash and is for tests and admin views, never for clients.
- `GameId` has no random constructor yet (uuid `v4` is off); the transport adds it.
- Persisting the `GameSetup` is the store's job: it holds an `Arc<dyn FormatRules>`, so the stored record must be a serializable recipe (format, seed, decks).

## Tests
- Bot clients play through the handle only; the actor's log equals the same game run on a bare engine (2 to 4 seats), 30 concurrent games finish, a reconnect continues to the uninterrupted result, and a failing store leaves the game unchanged.
