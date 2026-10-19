# Server

The `mtg-server` crate: one actor per game behind an axum REST and WebSocket transport, with Postgres persistence, accounts and the pace of play. The wire contract is in [protocol.md](protocol.md), the client in [web.md](web.md).

## Shape

- `spawn_game(id, setup, store)` starts a game in its lobby and `resume_game(store, id, setup)` rebuilds one from its log; both return a cheap-clone `GameHandle` (`join`, `send`, `leave`, `status`) and need a tokio runtime.
- One task per game owns the `Engine`, the `EventStore` and a bounded queue per seat. A full or closed queue drops that client, which reconnects and gets a snapshot, so the actor never waits for a client. The actor trusts the seat it is given: the transport maps a credential to one seat.
- A step checks the seat, the `DecisionId` and engine legality, applies the answer, projects for the connected seats, appends to the store, then sends the events, the next `Decision` (only to its seat) and `GameOver`. A store or projection failure restores the engine from its `Checkpoint` and answers `Error{Internal}` without details.
- The lobby has no engine until every seat has sent `Ready`; readiness is never stored and outlives a connection. The last `Ready` creates the engine and broadcasts the opening events from seq 0; if that fails, a repeated `Ready` retries.
- `Registry` holds the live games by random v4 `GameId`. A seat token is 256 random bits kept only as its SHA-256, and `authenticate` compares every seat so timing does not say which one matched. `MTG_MAX_GAMES` caps creation.
- The janitor sweeps every 30 s and drops finished games after 10 min, lobbies that never started after 1 h and started games with nobody connected for 24 h (`Retention`). It closes their sockets with 1001 and marks the record `expired` or `finished`.

## Transport

`Server::new(config)` keeps games in memory, `Server::open(config, pool)` also records them in Postgres, and `serve(listener, shutdown)` runs axum. Errors are `{error, message}`, and every response is `no-store` and `nosniff` because it can hold tokens and live state.

| Route | Behaviour |
|---|---|
| `POST /api/games` | `{format: "normal", seats: [{name, deck}]}` gives 201 `{game, seats: [{seat, token}]}`; 2 to 12 seats, `deck` a demo deck id, 16 KiB `application/json` body, unknown fields refused, server-random seed. |
| `GET /api/games/{id}` | `{started, seats, connected, ready, result}`; a malformed and an unknown id are one 404. |
| `GET /api/decks`, `GET /healthz` | Demo decks; `ok`. |
| `/api/auth/*`, `GET /api/me/games` | Accounts. |
| `GET /ws/{game}/{token}` | A seat's link: the token picks the seat before the upgrade. |
| `GET /ws/{game}` | The seat bound to the session cookie's user. |

- The origin guard (`origin.rs`) keeps other sites from acting with a visitor's cookie: no `Origin` (a bot) passes, otherwise it must be in `MTG_ALLOWED_ORIGINS` or name the `Host` authority, else 403 `forbidden_origin`. It covers both socket routes and every request that changes something (any method but GET, HEAD, OPTIONS); CORS answers listed origins only, with credentials.
- A socket checks the origin, then the credential (an unknown game, a malformed id and a wrong token share one 404), then expects `Join` first ([protocol.md](protocol.md), Sync contract): a wrong version gets `Error{UnsupportedVersion}`, a foreign game id `Error{UnknownGame}`, anything else `Error{BadMessage}`, each closing with 1002.
- A second socket for a seat replaces the first. A closing socket drops its queue and never calls `leave`, which would close the connection that replaced it.
- Shutdown (SIGTERM, ctrl-c) stops accepting, sends every socket 1001 and waits up to 5 s; what is still open then is abandoned, so a stalled request cannot hold the process.

| Limit per socket | Value |
|---|---|
| `Join` | within 10 s of the upgrade |
| Frame and message | 64 KiB |
| Messages | a burst of 40, then 20 per second |
| Malformed messages | 5 in a row, each answered `Error{BadMessage}` |
| Ping | every 20 s; closed after 60 s without a pong |

| Close code | Meaning |
|---|---|
| 1001 | Server stopping, or the game left the registry |
| 1002 | Bad handshake or protocol error |
| 1007 | Text frame that is not UTF-8 |
| 1008 | Policy: rate limit, malformed run, no `Join` in time, no pong |
| 1009 | Frame too big |
| 1011 | A server message could not be serialised |
| 1013 | Dropped as too slow: reconnect |
| 4000 | Replaced by a newer socket of the same seat: do not reconnect on your own |

## Persistence

- `PgEventStore` implements `EventStore` over `game_events`, primary key `(game_id, seq)`. An append is one transaction under a per-game advisory lock that checks the log ends at `from_seq`, so a gap, a repeat or a lost race returns `OutOfOrder` and a failure midway leaves no rows. Reads check for gaps and undecodable events (`StoreError::Corrupt`).
- Migrations (`crates/mtg-server/migrations`) are tracked in `_mtg_server_migrations`, not `_sqlx_migrations`, which `mtg-db` owns, so both crates migrate one database in any order, repeatedly and concurrently. The table name is an on-disk contract; `mtg_server::migrate(&pool)` runs them.
- `games` holds each game's recipe, result and status (`lobby`, `running`, `finished`, `expired` when the janitor dropped it, `failed` when boot could not load it); `seats` holds each seat's name, token digest and account. `game_events` has no foreign key to `games`, so a log can be stored first.
- `Registry::create` writes the game and seats in one transaction before the actor exists, so a failed write creates nothing (503 `unavailable`). The actor's `Recorder` notes each seat's first join, the start and the end; a failed write is logged and never stops a game, because the log is the truth and boot repairs a status that lags behind it.
- `GameRecipe {format, seed, decks, fingerprints}` is the only way the server builds a `GameSetup`, for a new game and a resumed one, because a `GameSetup` holds an `Arc<dyn FormatRules>` and cannot be stored. Its JSON is an on-disk contract: unknown fields and variants are refused.

### Resume

- `Server::open` loads every `lobby` and `running` game oldest first, rebuilds the setup from the recipe, checks the seat rows against it and calls `resume_game` with the stored digests, so old tokens open the same seats. A game whose log shows it ended is only corrected to `finished`.
- A recipe, seat rows or log that do not load (a replay panic included) or a changed deck mark the game `failed` with the reason, and boot goes on; only an unreachable database stops it. A `failed` game is not retried (fix the row and set it to `running` by hand) and is not loaded, so its tokens, lookup and sockets answer 404 and `/api/me/games` omits it.
- A lobby returns with nobody ready and keeps its age: past `Retention::lobby_after` (1 h, by the database clock) it becomes `expired`. A running game gets a fresh 24 h without a connection. `MTG_MAX_GAMES` limits creation only.

### Deck check

- A new game's recipe stores a `DeckFingerprint` per seat: the SHA-256 of a canonical, versioned text of the ordered cards the engine is given (`v1:<64 hex digits>`). `Engine::resume` applies stored events as facts, so without it a log would replay under other cards without failing.
- `GameRecipe::build` recomputes it and fails with `RecipeError::DeckChanged` for the first seat whose cards differ in any field, order or size; resume marks the game `failed` before replaying anything. A recipe without fingerprints resumes unchecked, with a warning at every boot.
- It covers the definitions the engine is given, so an edited card record, a changed rules file or an engine that carries more of a card fails the game; a changed seed or format rule is not detected. A new `CardDefinition` field changes every fingerprint unless it is left out of the JSON while it holds its default (the pinned values in the tests fail otherwise).
- A deck of oracle ids (`DeckRef::Oracle`) is assembled by `CardSource` ([cards-dsl.md](cards-dsl.md), Printed side and rules); an id with no row fails with `CatalogError::NotFound`.

## Accounts

- `username_key` is the lower case and unique, so names are unique ignoring case. A session is the SHA-256 of its cookie token. Without `DATABASE_URL` every account endpoint answers 503 `accounts_unavailable` and games are guest-only.
- A username is 3 to 32 ASCII letters, digits, `_` or `-` (no Unicode, so no look-alikes); a password is 10 to 128 characters, compared byte for byte and never normalised. Passwords are Argon2id (`argon2` pinned at 0.6.0), stored as the PHC string so the cost travels with the hash; the default is the OWASP minimum (19 MiB, 2 passes, 1 lane). Hashing runs on `spawn_blocking` behind a semaphore (`AuthConfig::hash_jobs`, 4), so a burst cannot exhaust memory.

| Endpoint | Success | Failure |
|---|---|---|
| `POST /api/auth/register` | 201 `{id, username}` and a session | 400 `invalid_username` or `invalid_password` (the message states the rule, never the input); 409 `username_taken`; 403 `registration_closed` (decided before the body is read); 429 |
| `POST /api/auth/login` | 200 `{id, username}` and a fresh session | 401 `invalid_credentials`; 429 `too_many_attempts` with `Retry-After` |
| `POST /api/auth/logout` | 204 with or without a session; the cookie is cleared | |
| `GET /api/auth/me` | 200 `{id, username}` | 401 `unauthenticated`; a cookie the server does not honour is cleared |

Bodies and codes are in [protocol.md](protocol.md), REST bodies. A database failure is 503 `unavailable`, and a login it stops costs no throttle budget.

- Login does not reveal whether a username exists: an unknown name, a wrong password and a name or password that breaks the rules get one byte-identical 401, and an unknown name costs one Argon2 check against a stand-in hash. Registration does say "taken"; that cannot be avoided.
- The cookie `mtg_session` holds 256 random bits as hex and nothing else: `HttpOnly; SameSite=Lax; Path=/; Max-Age=2592000` (30 days), no `Domain`, plus `Secure` when `MTG_COOKIE_SECURE=1`. Expiry is absolute and judged by the database clock; an upkeep task purges expired rows hourly. A sign-in that presents a session ends it, so a browser holds one.
- Guessing is throttled in memory per process (`throttle.rs`) in 15-minute fixed windows: failed logins per client address (30) and per username (10, whether or not the account exists), registrations per address (10). A login counts when it starts, so parallel guesses cannot slip under the limit, and is handed back on success or a server failure. Over the limit the answer is 429, even for the right password. IPv6 counts by /64; past 50 000 counters, untracked keys go unlimited until a sweep frees room.
- The client address is the socket's peer. Behind a reverse proxy, `MTG_TRUSTED_PROXIES=n` takes the `n`-th entry from the right of `X-Forwarded-For`, which the nearest trusted proxy wrote, so forged entries to its left are ignored; with 0 the header is ignored, because anybody can send it.
- Passwords, tokens, digests and hashes never reach logs or `Debug` output; failed attempts are logged without the name tried.

### Seats and accounts

- `seats.user_id` is unique per game, so an account holds one seat of a game. A signed-in `POST /api/games` binds seat 0 to the creator, and the first signed-in join through an unbound seat's link binds that seat before the upgrade; both need a recorded game, and guests bind nothing. One `UPDATE .. WHERE user_id IS NULL AND NOT EXISTS (the user's other seat)` decides, so racing accounts get one winner and a bound seat never changes hands (`Registry::bind_seat`, `Binding`). A failed binding is logged and never stops the join.
- Binding adds a way in and takes none away: old links keep working, and another signed-in account that uses a bound seat's token plays like a guest and is not bound.
- `GET /ws/{game}` answers, after the origin check, 404 for an address that is not a game id, 503 `accounts_unavailable`, 401 `unauthenticated` (a bad cookie is cleared), then one 404 for an unknown game and for a game the user holds no seat in, so a stranger learns nothing about which games exist.
- `GET /api/me/games` lists the user's `lobby`, `running` and `finished` games, newest first, at most 100. It reads the database, so a finished game stays listed after the registry dropped it.

## Pace of play

At each priority decision the actor asks the seat, passes for it at once or passes for it after a short wait. Every pass takes the path of the seat's own answer (`Running::answer`), so it is checked, logged, saved and projected like a click.

### Windows

`windows::verdict` returns `Ask`, `Pass` or `Dwell` from the step, whose turn it is, the stack, the seat's stops, mode and yield, and `can_act`; auto-pass off is the `Held` mode. A seat that is passed for is never sent the decision, and a decision that is not a priority decision is always asked. The first matching rule wins:

| # | Condition | Verdict |
|---|---|---|
| 1 | Full Control (`NextWindow`, `Held`) | Ask at every window |
| 2 | A running yield (a stop of the seat ends it, except in the step it was asked at) | Pass at once |
| 3 | A stop of the seat for this side and step | Ask, whether it can act or not |
| 4 | A default window and the seat can act | Ask |
| 5 | A default window and the seat cannot act | `Dwell`; pass at once in its own main phase with an empty stack |
| 6 | Anything else | Pass at once, whatever the seat holds |

The default windows are both main phases on the seat's own turn, declare attackers and declare blockers on any turn, and any step with an object of another seat on the stack. `Own` stops are for the seat's turns, `Opponent` stops for any other seat's.

### Stops and Full Control

- Stops and modes live in the actor per seat (`Stops`), never stored: a reconnect keeps them, a resumed game starts every seat in the default mode with no stop. Setting one takes no engine step, so nothing is saved and the pending decision keeps its id.
- A one-turn stop goes once the game has left its step: a step entered and left in one go counts, one never entered does not. `NextWindow` returns to `Off` when the seat passes, itself or at its clock or dwell.
- Turning Full Control on ends a running yield and `Yield` is refused while it is on; turning it off at a priority decision that the windows then pass for passes at once, as a started yield does.

### Yields

`Yield` and `CancelYield` set and end a per-seat yield (`Yields`), never stored. Its end rules ([protocol.md](protocol.md), Yields) are all in `Active::ends_at` and read the table alone, never `can_act`, so a seat that can respond and one that cannot are passed for and stopped at the same moments. If a started yield ends at once (`Resolve` over an empty stack) the decision is issued again with a new id, so a client that dropped its prompt is never left without one.

### Passes and saves

- An answer, the passes after it and the save are one unit (`publish_with`): copies of the yields and stops, `Running::skip_idle` (the passes, appended to the step), one projection, one append, one batch per seat. The copies replace the seats' own only once saved, so a failed save restores the engine and keeps yields, stops and modes.
- A run of passes stops at the first decision the windows ask or that waits for a dwell, at any other decision, at the game's end and after `MAX_IDLE_PASSES` (500), where the decision is sent as it is and its seat's yield ends with a notice. `start_up` settles a resumed game before any client connects.

### Response clocks

- A decision gets its clock when issued (`Running::settle`, `Pace::clock_for`). A `Priority` decision of a seat that is not the active player gets `MTG_RESPONSE_SECS` (10 s), whether it can act or not, because a clock that read `can_act` would show what a hand holds; `DeclareBlockers` of a defending seat gets `MTG_BLOCK_SECS` (45 s). Every other decision has none.
- The clock ignores the mode, runs while the seat is away, stops with any answer and restarts when a game is resumed. `deadline_ms` carries the time left ([protocol.md](protocol.md), Decisions).
- At the deadline the actor answers with `Running::fall_back` (a pass, or no blocks) through `Running::answer`, so a late answer is the ordinary `StaleDecision`. The loop selects between the next command and the pending `due` time, commands first, because a command already waiting was sent before the clock ran out. A failed save restores the engine and retries a second later.

### Dwell

- A seat that cannot act at a default window, other than its own main phase with an empty stack, and holds no stop, Full Control or yield is neither asked nor passed for at once, because a pass at once shows that it holds nothing. The windows are declare attackers and declare blockers, any window with an object of another seat on the stack, and its own main phase with any object on the stack.
- The decision is held back (`Running::settle`): it is never sent, a join or `Resync` sends the snapshot alone and an answer for it is stale. At its `due` time `expire` passes through `fall_back` as a step, an append and a batch of its own, so every other seat sees the stream of a seat that could act and passed.
- The wait is uniform between 250 and 700 ms (`MTG_DWELL_MS`), drawn from a generator the operating system seeds, independent of the game seed and the state; every idle seat draws its own, so no wait depends on the seats before it. It is wall clock only: the log, event order and state hash are those of a game without a dwell. With `Dwell::OFF` (also `Pace::MANUAL`) `Dwell` is a pass at once.
- A stop at the window, Full Control or auto-pass off ends the wait and sends the decision at once; a yield passes at once; a concession that removes the seat ends it. Chat, `Resync`, a stale answer, a stop elsewhere and other seats' commands leave it.
- It hides whether a seat could act in a combat window or while a spell waits on the stack, in the batches and arrival times every other seat sees. It does not hide a seat's own main phase with an empty stack (what is played there is in plain sight, so a seat that can act is asked and one that cannot is passed at once), how long a seat that can act takes (a pause past 700 ms or a pass before 250 ms is its own), a pass at a response-clock deadline, or a choice of stops, Full Control or yield. A step with N idle seats costs N dwells, 475 ms on average and 700 at most each.

## Previews
- `ClientMessage::Preview` is answered by the actor itself (`preview.rs`, `Running::preview`) with one `Forecast` to the sender. The decision id must be the pending one for that seat; then `Engine::preview_forecast` judges the answer. Everything else gives `forecast: null` and never an `Error` (a stale id, a foreign one, an answered decision, a non-combat decision, an answer the engine refuses, no game yet), because the stale-decision treatment of `Answer` would raise a notice and a resync in a client whose preview raced with its real answer. Nothing is applied, projected, stored, logged or sent to another seat, so the event log, the store and the state hash stay as they were (tests compare all three).
- Rate limit: a token bucket per seat inside the actor (`PreviewLimits`; `PREVIEW_BURST` 20, then `PREVIEWS_PER_SECOND` 10), kept for the life of the game so a reconnect does not refill it. A preview over the limit is answered with a null forecast without touching the engine, so the client is never left waiting. Real answers have no bucket and queue behind previews like any message; the previews the bucket lets through cost one clone of the state each (5 to 8 µs), at most 20 in a burst, so a flood cannot starve an answer (a test sends 150 previews and an answer straight after). The socket limit (40 messages, then 20 a second) applies to every frame, previews included.
- Who may preview: every socket is bound to one seat by its credential before the upgrade and must open with `Join`, so there are no unseated or spectator sockets to refuse; a `Preview` as the first frame is refused like any other message (`Error{BadMessage}`, close 1002), and a message for a seat the game does not have is ignored.

## Tests
- `tests/pg_accounts.rs` (Postgres, real HTTP): the whole sign-up, sign-in, who-am-I and sign-out cycle and the cookie's exact attributes with and without `Secure`; username and password limits as a table; case-insensitive uniqueness (also enforced by the database); malformed bodies; digests and hashes only in the tables; byte-identical failed logins; throttling per username, per address behind a trusted proxy, with a forged `X-Forwarded-For`, and registrations; closed registration; expiry, purge and replacement of sessions; foreign origins on every POST; CORS credentials; the server without a database; a failing database; racing registrations; the logs searched for secrets. Unit tests cover the rules (`credentials.rs`), hashing (`passwords.rs`), the cookie (`cookie.rs`), the counters with an injected clock (`throttle.rs`), the forwarded-address rule (`auth.rs`) and the environment (`config.rs`).
- `tests/pg_user_games.rs` (Postgres, real sockets): seat 0 bound to a signed-in creator and nothing bound for guests or stale cookies; the first signed-in visitor claiming a seat, a second account and a second seat of the same account refused (also when racing); the five outcomes of `Registry::bind_seat`; the session route refusing no cookie, a bad cookie, a stranger, an unknown game, a malformed id, another site and a seat token in place of the cookie; the list (lobby, running, finished, other seats and usernames, newest first, expired and failed hidden, 100 at most, 401/405/503); finished games listed after the registry dropped them; a whole game over sockets by a signed-in user who rejoins through the session alone after a disconnect and after being replaced; guests and stale cookies playing a whole game by their links; bindings and sessions surviving a restart, including a resumed running game; an account deleted by hand; a game that was never recorded. `Registry::bind_seat` and `games_of` without a database are covered in `tests/registry.rs`.
- `tests/pg_resume_decks.rs` (Postgres, the deck check on boot): an unchanged deck resumes at every boot with its log and logs no warning; a table of changes (a card's power, toughness, cost, name, type or abilities, two cards trading places, the list reversed, one card fewer or more, a land swapped for a creature, no cards, the third or second of three seats) each fail the game with "the deck changed" and the seat, in the record and in the log, while a control game resumes; a running game is refused before its log is read, keeps its events, and comes back once the fingerprint is right and the status is set by hand; a recipe without `fingerprints` resumes, warns and plays to the end; through real HTTP and sockets a failed game is a 404 on the lookup and both socket routes and is not in `/api/me/games`. The fingerprint itself (pinned values, the canonical text, every card field and every kind of list change, the stored text) is covered by the unit tests in `recipe/fingerprint.rs` and `recipe.rs`.
- `tests/store_contract.rs` runs one contract (round trip, empty appends, gaps and repeats, game isolation, racing appends with exactly one winner, whole-batch races) against `MemoryStore` and `PgEventStore`; `pg_store.rs` covers real game logs, `read_from`, a failing batch, damaged rows and an unreachable database; `pg_migrations.rs` runs both migrators in each order, repeatedly and concurrently; `pg_game.rs` plays and resumes whole bot games on Postgres; `pg_records.rs` checks what creating, joining, starting and finishing write (digests only, atomic creation, 503 when the database is down); `pg_resume.rs` restarts the server on the same pool (handle bots and real sockets with the old tokens, finished, lobby, expired, janitor-dropped and damaged games, token isolation).
- `tests/auto_pass.rs` (the actor's log equals a bare engine game under the same rule, with the default clocks on and on a paused clock, the setting off delivers idle decisions, mid-game toggling, `Welcome`, lobby setting, the run stopping at other decisions, a table that plays itself to the end, a failed save, resume, the cap), `yields.rs` (a yield at every window and its stops, the same for a seat that can respond and one that cannot, other decisions delivered, cancel, a yield replacing another, resume, a concession, losing, a failed save, the cut run, bot games where one seat yields each time it would pass), `response_clock.rs` (who gets a clock and how long, an idle window of a non-active seat too, expiry as a normal pass or no blocks, a late answer is stale, an answer in time stops the clock, a reconnect is told the time left, the clock runs while the seat is away, a failed save at the deadline, a resume restarts it), `ws_pace.rs` (the wire shapes over a socket) and `pg_pace.rs` (a game on Postgres with auto-pass and clocks restarts and ends as an uninterrupted one; the setting is forgotten with the process). The older suites run with `Pace::MANUAL`, which `roomy()` and `bot_config()` set, so they still compare games with the bare engine.
- `tests/lobby_roster.rs` (through the handle): the roster on connect and after each join, hang-up, `Leave`, `Ready` and `NotReady` for 2, 3 and 6 seats, silence for repeats and for a seat that takes another's place, `needed`, names from the registry, a ready seat that leaves staying ready, a lookup or chat line that finds a closed queue first, a client dropped for a full queue, `NotReady` stopping the last `Ready`, late `Ready`/`NotReady` after the start, a lobby and a running game resumed from a store. `ws_lobby.rs` (real sockets): the exact JSON, the order of the frames, the opening events instead of a roster at the start, hang-up and polite close, the REST `ready` before and after a socket, strangers and a socket that never joined hearing nothing. `pg_lobby.rs` (Postgres): a restarted lobby keeps its names and has nobody ready.
- Windows: `windows.rs` (unit: the verdict for every combination of turn side, the 13 steps, stack, stop, mode, yield and `can_act`, a row for each kind of window, the default windows spelled out by hand, a window read from the stack, a stop ending a yield, Full Control leaving a yield alone) and `tests/windows.rs` (through the handle: the default windows for a seat that can act and one that cannot, an object of another seat as a window, a stop asking a seat that cannot act, one-turn and lasting stops over several turns, a stop set while its step runs and one set after it, a seat that leaves, a stop ending a yield, `Held` and `NextWindow` over a turn, `NextWindow` ending at the seat's pass or the clock's, the refused yield, Full Control ending a yield, the auto-pass mapping, a server that starts in `Held`, a mode turned off at a window the rules pass, failed saves, the same clock for every delivered window, and 2 to 4 seat bot games, with and without stops and Full Control chosen by the bots, whose log, hash and result equal the testkit reference under the same rule).
- Dwell: `dwell.rs` (unit: `250-700`, one number and `0` read and everything else refused, waits inside their range and reaching both ends, an even spread, each wait independent of the one before it), `running.rs` (unit: a seat that cannot act in its own main phase with an empty stack is passed for at once, the run stops at a window where a seat waits, a held-back decision is not in a snapshot and is stale to an answer, the dwell keeps or moves with a concession, waits do not follow the game seed) and `tests/idle_dwell.rs` (paused time, through the handle: the declare attackers and declare blockers windows and a spell on the stack wait and pass as batches of their own while the steps between pass at once, the own main phase with an empty stack costs no time, a seat that can act, a stop, Full Control and a yield are not made to wait, tables of 4 and 6 idle seats where every seat waits a dwell of its own, the commands that end a dwell and those that leave it alone, a failed save at the end of a dwell, bot games of 2 to 4 seats and a resumed game that write the log of the game without a dwell). `tests/pace_leak.rs` is the leak test above, with the attacker or the defender holding the card and the other seat able to act or idle, the same comparison with the dwell off, and the own main phase.
- Stops and Full Control: `stops.rs` (unit: a stop held exactly in the steps where a player gets priority on both sides, untap and cleanup refused when set and when cleared, a stop held once with the latest `keep`, a fixed order whatever the keeps, seats independent, `replace` naming the seats whose list or mode changed (a changed `keep` too, and not a repeat), `left` clearing one-turn stops exactly when their step is left (a table, a step entered and left in one go, several turns), `passed` ending `NextWindow` only, auto-pass as every mode but `Held`, `drop_finished` for a seat that left and a finished game), `tests/stops.rs` (through the handle, in the lobby and in a running game: the table over all thirteen steps and both sides with both keeps, silence for repeats, a lasting and a one-turn stop told with their keeps, an upgrade and a downgrade by a repeated `SetStop` told as the list, the whole list told in order whatever the keeps, only the seat itself told, each mode, a reconnect and a `Resync`, lobby to game, a resumed game, no engine step taken (log, status and the pending decision id unchanged), a concession told after its events, a finished game told before `GameOver` and then refusing, a failed save, and 2 to 4 seat bot games with every stop and Full Control held that equal the bare reference game) and `ws_stops.rs` (the exact JSON over a socket; a refused step and malformed asks leave the socket open and the stops alone).
- Bot clients play through the handle only; the actor's log equals the same game run on a bare engine (2 to 4 seats), 30 concurrent games finish, a reconnect continues to the uninterrupted result, and a failing store leaves the game unchanged.
