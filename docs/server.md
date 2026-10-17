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
- `GameId::random()` (uuid v4) names every new game.
- Persisting the `GameSetup` is the store's job: it holds an `Arc<dyn FormatRules>`, so the stored record must be a serializable recipe (format, seed, decks).

## Tests
- Bot clients play through the handle only; the actor's log equals the same game run on a bare engine (2 to 4 seats), 30 concurrent games finish, a reconnect continues to the uninterrupted result, and a failing store leaves the game unchanged.

## Transport
- `Server::new(config).serve(listener, shutdown)` runs axum; the `mtg-server` binary reads `MTG_BIND` (127.0.0.1:8080), `MTG_ALLOWED_ORIGINS` (comma list) and `MTG_MAX_GAMES` (100). `ServerConfig::fixed_seed` is for tests only and is never read from the environment.
- REST: `POST /api/games` {format:"normal", seats:[{name, deck}]} gives 201 {game, seats:[{seat, token}]} (2 to 12 seats, 16 KiB `application/json` body, unknown fields refused, the seed is always server-random); `GET /api/games/{id}` gives {started, seats, connected, result}; `GET /api/decks`; `GET /healthz`. Errors are `{error, message}`.
- `Registry` holds the games: tokens are 256 random bits from the OS and only their SHA-256 is kept (never logged); a cap on games; a janitor drops finished games after 10 min, lobbies that never started after 1 h and started games nobody has been connected to for 24 h.
- `GET /ws/{game}/{token}`: the token picks the seat before the upgrade (unknown game, malformed id and wrong token share one 404); `Origin` must be listed or match `Host` (no `Origin` is a bot and passes). Then `Join` within 10 s (wrong version gets `Error{UnsupportedVersion}`, a mismatching game `Error{UnknownGame}`), `handle.join(seat)`, and one text frame per `ClientMessage`/`ServerMessage`.
- Limits per socket: frames at most 64 KiB, a burst of 40 messages then 20 per second, five malformed messages in a row, ping every 20 s and a close after 60 s without a pong. A second socket for a seat replaces the first.
- Close codes: 1001 server stopping or game removed, 1002 bad handshake, 1007 not UTF-8, 1008 policy (rate, malformed run, no join, no pong), 1009 frame too big, 1013 dropped as too slow (reconnect), 4000 replaced by a newer socket of the same seat (do not reconnect on your own). A closed socket drops the seat's queue and never calls `leave`.
- Shutdown (SIGTERM or ctrl-c): stop accepting, send every socket 1001, wait up to 5 s for all of it; a stalled request cannot hold the process.
- Tests (`tests/{ws_games,ws_refusals,ws_limits,rest,registry,shutdown}.rs`, helpers in `tests/net/`) use a real listener on port 0, raw TCP for hostile HTTP and tokio-tungstenite for sockets.
