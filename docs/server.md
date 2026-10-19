# Server

The `mtg-server` crate: one actor per game behind an axum REST and WebSocket transport, with Postgres persistence, accounts and the pace of play. The wire contract is in [protocol.md](protocol.md), the client in [web.md](web.md).

## Shape

- `spawn_game(id, setup, store)` starts a game in its lobby and `resume_game(store, id, setup)` rebuilds one from its log; both return a cheap-clone `GameHandle` (`join`, `send`, `leave`, `status`) and need a tokio runtime. `status()` carries the state hash and is for tests and admin views, never for clients.
- One task per game owns the `Engine`, the `EventStore` and a bounded queue per seat. A full or closed queue drops that client, which reconnects and gets a snapshot, so the actor never waits for a client. The actor trusts the seat it is given: the transport maps a credential to one seat.
- A step checks the seat, the `DecisionId` and engine legality, applies the answer, projects for the connected seats, appends to the store, then sends the events, the next `Decision` (only to its seat) and `GameOver`. A store or projection failure restores the engine from its `Checkpoint` and answers `Error{Internal}` without details.
- The lobby has no engine until every seat has sent `Ready`; readiness is never stored and outlives a connection. The last `Ready` creates the engine and broadcasts the opening events from seq 0; if that fails, a repeated `Ready` retries. Chat (500 characters at most) goes to the connected seats and is never stored.
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

- A decision gets its clock when issued (`Running::settle`, `Pace::clock_for`). A `Priority` decision of a seat that is not the active player gets `MTG_RESPONSE_SECS` (10 s), whether it can act or not, because a clock that read `can_act` would show what a hand holds; so do its `ChooseTargets` (a spell or a triggered ability) and `OrderTriggers`, which the table waits for just as long. `DeclareBlockers` of a defending seat gets `MTG_BLOCK_SECS` (45 s). Every other decision has none, and none of the active player's does.
- The clock ignores the mode, runs while the seat is away, stops with any answer and restarts when a game is resumed. `deadline_ms` carries the time left ([protocol.md](protocol.md), Decisions).
- At the deadline the actor answers with `Running::fall_back` through `Running::answer`, so a late answer is the ordinary `StaleDecision`. The answer is a pass, no blocks, no target (`Cancel` for a spell, which the rules take back at no cost; `Remove` for a triggered ability, which they take off the stack when its target cannot be chosen) or the triggers in the order they triggered. The loop selects between the next command and the pending `due` time, commands first, because a command already waiting was sent before the clock ran out. A failed save restores the engine and retries a second later.

### Dwell

- A seat that cannot act at a default window, other than its own main phase with an empty stack, and holds no stop, Full Control or yield is neither asked nor passed for at once, because a pass at once shows that it holds nothing. The windows are declare attackers and declare blockers, any window with an object of another seat on the stack (a spell, or an ability: a triggered ability is an object of its controller, the seat it was put on the stack for), and its own main phase with any object on the stack.
- The decision is held back (`Running::settle`): it is never sent, a join or `Resync` sends the snapshot alone and an answer for it is stale. At its `due` time `expire` passes through `fall_back` as a step, an append and a batch of its own, so every other seat sees the stream of a seat that could act and passed.
- The wait is uniform between 250 and 700 ms (`MTG_DWELL_MS`), drawn from a generator the operating system seeds, independent of the game seed and the state; every idle seat draws its own, so no wait depends on the seats before it. It is wall clock only: the log, event order and state hash are those of a game without a dwell. With `Dwell::OFF` (also `Pace::MANUAL`) `Dwell` is a pass at once.
- A stop at the window, Full Control or auto-pass off ends the wait and sends the decision at once; a yield passes at once; a concession that removes the seat ends it. Chat, `Resync`, a stale answer, a stop elsewhere and other seats' commands leave it.
- It hides whether a seat could act in a combat window or while a spell waits on the stack, in the batches and arrival times every other seat sees. It does not hide a seat's own main phase with an empty stack (what is played there is in plain sight, so a seat that can act is asked and one that cannot is passed at once), how long a seat that can act takes (a pause past 700 ms or a pass before 250 ms is its own), a pass at a response-clock deadline, or a choice of stops, Full Control or yield. A step with N idle seats costs N dwells, 475 ms on average and 700 at most each.

## Previews

The actor answers `Preview` itself (`preview.rs`) with one `Forecast`; `Engine::preview_forecast` judges the answer for the seat's pending decision. Everything else gives `forecast: null` and never an `Error`, because the stale-decision treatment of `Answer` would raise a notice and a resync in a client whose preview raced its real answer. A token bucket per seat (`PREVIEW_BURST` 20, then `PREVIEWS_PER_SECOND` 10) lives as long as the game, so a reconnect does not refill it; a preview over the limit gets a null forecast without touching the engine. Answers have no bucket, and a preview costs one state clone, so a flood cannot starve an answer.

## Configuration

Blank values count as unset; any other malformed value refuses to start (`ConfigError`).

| Variable | Default | Meaning |
|---|---|---|
| `MTG_BIND` | `127.0.0.1:8080` | Listen address, `ip:port`. |
| `MTG_ALLOWED_ORIGINS` | none | Comma list of `http(s)://host[:port]` origins besides the server's own host (sockets, state-changing requests, CORS). |
| `MTG_MAX_GAMES` | 100 | Most games alive at once, at least 1. |
| `MTG_AUTO_PASS` | on | `1`, `on`, `true` or `0`, `off`, `false`. Off starts every seat in `Held`. |
| `MTG_RESPONSE_SECS` | 10 | Clock on a non-active seat's priority, target and trigger-order decisions; `0` for none, at most 86 400. |
| `MTG_BLOCK_SECS` | 45 | Clock for declaring blockers; same range. |
| `MTG_DWELL_MS` | `250-700` | Dwell: a range, one number for a fixed wait, `0` for none; at most 5000. |
| `MTG_REGISTRATION` | `open` | `closed` refuses new accounts; existing ones keep working. |
| `MTG_COOKIE_SECURE` | 0 | `1` marks the session cookie `Secure`; set it behind TLS. |
| `MTG_TRUSTED_PROXIES` | 0 | Reverse proxies in front of the server, 0 to 16. |
| `DATABASE_URL` | unset | Postgres URL, never logged. Set, the binary connects (10 connections), migrates and calls `Server::open`; unset or blank, it warns and keeps games in memory without accounts. `.cargo/config.toml` defaults it to the local database (`docker compose up -d db`). |
| `RUST_LOG` | `info` | Log filter. |

`ServerConfig` holds more values (queues, socket limits, retention, hash cost) that only tests set; `fixed_seed` is never read from the environment.

## Testing

- Actor tests drive games through `GameHandle` with bot and manual clients (`tests/common`) and compare the log, state hash and result with the same game on a bare engine (`mtg-testkit`); `roomy()` runs under `Pace::MANUAL`, `windowed()` under the default windows alone.
- `Windows` in `mtg-testkit` is the window rule written plainly and apart from the server; `tests/windows.rs` compares the actor with it for 2 to 4 seats, with stops and modes the bots choose.
- Clocks, the dwell and the leak test run on tokio's paused time; `tests/pace_leak.rs` plays two games that differ by one hidden card, in combat windows and with a spell or a triggered ability on the stack, and requires the other seat to be sent the same messages in the same batches; `tests/response_clock_effects.rs` covers the clocks of the target and order decisions.
- Transport tests (`tests/net`) use a real listener on port 0, raw TCP for hostile HTTP and tokio-tungstenite for sockets; `FlakyStore` fails writes on demand.
- Postgres tests (`pg_*.rs`) need `docker compose up -d db` and use `#[sqlx::test(migrations = false)]` with `migrate`, because the default would use `_sqlx_migrations`.
