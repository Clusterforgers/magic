# Server

The `mtg-server` crate: one actor per game behind an axum REST and WebSocket transport, with Postgres persistence, accounts and the pace of play. The wire contract is in [protocol.md](protocol.md), the client in [web.md](web.md).

## Shape

- `spawn_game(id, setup, store)` starts a game in its lobby and `resume_game(store, id, setup)` rebuilds one from its log; both return a cheap-clone `GameHandle` (`join`, `send`, `leave`, `status`) and need a tokio runtime. `status()` carries the state hash and is for tests and admin views, never for clients.
- One task per game owns the `Engine`, the `EventStore` and a bounded queue per seat. A full or closed queue drops that client, which reconnects and gets a snapshot, so the actor never waits for a client. The actor trusts the seat it is given: the transport maps a credential to one seat.
- A step checks the seat, the `DecisionId` and engine legality, applies the answer, projects for the connected seats, appends to the store, then sends the events, the next `Decision` (only to its seat) and `GameOver`. A store or projection failure restores the engine from its `Checkpoint` and answers `Error{Internal}` without details.
- The lobby has no engine until every seat has sent `Ready`; readiness is never stored and outlives a connection. The last `Ready` creates the engine and broadcasts the opening events from seq 0; if that fails, a repeated `Ready` retries. Chat (500 characters at most) goes to the connected seats and is never stored.
- The lobby roster (`ServerMessage::Lobby`) lists every seat and is sent after each change. An open seat is listed with an empty name, not connected and not ready, so it counts in `needed` and the game cannot start while one is open: nobody can join an open seat and nothing sent as one counts, so a `Ready` still on its way from a player removed a moment ago starts nothing.
- `Registry` holds the live games by random v4 `GameId`. A seat token is 256 random bits kept only as its SHA-256, and `authenticate` compares every seat so timing does not say which one matched. `MTG_MAX_GAMES` caps creation.
- The janitor sweeps every 30 s and drops finished games after 10 min, lobbies that never started after 1 h and started games with nobody connected for 24 h (`Retention`). It closes their sockets with 1001 and marks the record `expired` or `finished`. A host who abandons a lobby ends it sooner with `DELETE /api/games/{id}` ([Open games](#open-games)).

## Transport

`Server::new(config)` keeps games in memory, `Server::open(config, pool)` also records them in Postgres, and `serve(listener, shutdown)` runs axum. Errors are `{error, message}`, and every response is `no-store` and `nosniff` because it can hold tokens and live state.

| Route | Behaviour |
|---|---|
| `POST /api/games` | `{format: "normal", seats: [{name, deck}], visibility?, held?}` gives 201 `{game, code, seats: [{seat, token}]}`; 2 to 12 seats, `deck` a demo deck id, 16 KiB `application/json` body, unknown fields refused, server-random seed. `visibility` is `public` or `invite` (the default); `held` is how many seats from seat 0 get a token (1 to the seat count, every seat by default): the others are open, their `name` is never read and `seats` lists only the held ones. |
| `GET /api/games/{id}` | `{started, seats, connected, ready, result, visibility}`; a malformed and an unknown id are one 404. |
| `/api/lobbies`, `/api/join`, `/api/games/{id}/join`, `/leave`, `/seats/{n}/release`, `PATCH` and `DELETE /api/games/{id}` | The game browser: [Open games](#open-games). |
| `GET /api/demo-decks`, `GET /healthz` | The built-in decks; `ok`. |
| `/api/auth/*`, `GET /api/me/games` | Accounts. |
| `GET /api/cards`, `GET /api/cards/{oracle_id}` | Card search and one card; public. |
| `/api/decks`, `/api/decks/{id}`, `/api/decks/import`, `/api/decks/check`, `GET /api/decks/{id}/export` | Stored decks, import, export and the deck check; accounts. |
| `GET /ws/{game}/{token}` | A seat's link: the token picks the seat before the upgrade. |
| `GET /ws/{game}` | The seat bound to the session cookie's user. |

- The origin guard (`origin.rs`) keeps other sites from acting with a visitor's cookie: no `Origin` (a bot) passes, otherwise it must be in `MTG_ALLOWED_ORIGINS` or name the `Host` authority, else 403 `forbidden_origin`. It covers both socket routes and every request that changes something (any method but GET, HEAD, OPTIONS); CORS answers listed origins only, with credentials, allows GET, POST, PUT, PATCH and DELETE with the headers `Content-Type`, `Authorization` and `If-None-Match`, and exposes `ETag` and `Retry-After`.
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
| 4001 | The seat was taken away, because its player left or the host removed them: its token no longer opens anything, do not reconnect |
| 4002 | The host closed the game before it started: its links and code no longer open anything, do not reconnect |

## Persistence

- `PgEventStore` implements `EventStore` over `game_events`, primary key `(game_id, seq)`. An append is one transaction under a per-game advisory lock that checks the log ends at `from_seq`, so a gap, a repeat or a lost race returns `OutOfOrder` and a failure midway leaves no rows. Reads check for gaps and undecodable events (`StoreError::Corrupt`).
- Migrations (`crates/mtg-server/migrations`) are tracked in `_mtg_server_migrations`, not `_sqlx_migrations`, which `mtg-db` owns, so both crates migrate one database in any order, repeatedly and concurrently. The table name is an on-disk contract; `mtg_server::migrate(&pool)` runs them.
- `games` holds each game's recipe, result, status (`lobby`, `running`, `finished`, `expired` when the janitor dropped it or the host closed the lobby, `failed` when boot could not load it), visibility and the digest of its join code; `seats` holds each seat's name, state (`issued`, `open`, `claimed`), token digest (null while open) and account. `game_events` has no foreign key to `games`, so a log can be stored first.
- `Registry::create` writes the game and seats in one transaction before the actor exists, so a failed write creates nothing (503 `unavailable`). The actor's `Recorder` notes each seat's first join, the start and the end; a failed write is logged and never stops a game, because the log is the truth and boot repairs a status that lags behind it.
- `GameRecipe {format, seed, decks, fingerprints}` is the only way the server builds a `GameSetup`, for a new game and a resumed one, because a `GameSetup` holds an `Arc<dyn FormatRules>` and cannot be stored. Its JSON is an on-disk contract: unknown fields and variants are refused.

### Resume

- `Server::open` loads every `lobby` and `running` game oldest first, rebuilds the setup from the recipe, checks the seat rows against it and calls `resume_game` with the stored digests, so old tokens open the same seats. A game whose log shows it ended is only corrected to `finished`.
- A recipe, seat rows or log that do not load (a replay panic included) or a changed deck mark the game `failed` with the reason, and boot goes on; only an unreachable database stops it. A `failed` game is not retried (fix the row and set it to `running` by hand) and is not loaded, so its tokens, lookup and sockets answer 404 and `/api/me/games` omits it.
- A lobby returns with its visibility, join code and claims (names, tokens, accounts, open seats) and nobody ready, and keeps its age: past `Retention::lobby_after` (1 h, by the database clock) it becomes `expired`. A started game with an open seat does not load. A running game gets a fresh 24 h without a connection. `MTG_MAX_GAMES` limits creation only.

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
- Guessing is throttled in memory per process (`throttle.rs`) in 15-minute fixed windows: failed logins per client address (30) and per username (10, whether or not the account exists), registrations per address (10); join code misses per address (5) and requests for the lobby list per address (30) have a one-minute window each. A login counts when it starts, so parallel guesses cannot slip under the limit, and is handed back on success or a server failure. Over the limit the answer is 429, even for the right password. IPv6 counts by /64; past 50 000 counters, untracked keys go unlimited until a sweep frees room.
- The client address is the socket's peer. Behind a reverse proxy, `MTG_TRUSTED_PROXIES=n` takes the `n`-th entry from the right of `X-Forwarded-For`, which the nearest trusted proxy wrote, so forged entries to its left are ignored; with 0 the header is ignored, because anybody can send it.
- Passwords, tokens, digests and hashes never reach logs or `Debug` output; failed attempts are logged without the name tried.

### Seats and accounts

- `seats.user_id` is unique per game, so an account holds one seat of a game. A signed-in `POST /api/games` binds seat 0 to the creator, and the first signed-in join through an unbound seat's link binds that seat before the upgrade; both need a recorded game, and guests bind nothing. One `UPDATE .. WHERE user_id IS NULL AND NOT EXISTS (the user's other seat)` decides, so racing accounts get one winner and a bound seat never changes hands (`Registry::bind_seat`, `Binding`). A failed binding is logged and never stops the join.
- Binding adds a way in and takes none away: old links keep working, and another signed-in account that uses a bound seat's token plays like a guest and is not bound.
- `GET /ws/{game}` answers, after the origin check, 404 for an address that is not a game id, 503 `accounts_unavailable`, 401 `unauthenticated` (a bad cookie is cleared), then one 404 for an unknown game and for a game the user holds no seat in, so a stranger learns nothing about which games exist.
- `GET /api/me/games` lists the user's `lobby`, `running` and `finished` games, newest first, at most 100. It reads the database, so a finished game stays listed after the registry dropped it.

## Open games

A game has a visibility and each seat a state, so a visitor can claim an open seat by the game's id or with its join code ([ui-design.md](ui-design.md), Game browser and lobby).

- `Visibility` is `public` or `invite` (in JSON and `games.visibility`; `invite` by default, for old rows too). A public game in its lobby is listed; an invite game is reached only by its seat links and its join code. `GameOptions {visibility, held}` goes to `Registry::create_with`; `create` and `create_for` make invite games that hold every seat. `Registry::set_visibility(game, by, visibility)` is for the host (`HOST`, seat 0) and works before the start.
- A seat is `Issued` (its token was made with the game and is in the create response: the seat link), `Open` (no token, no account, an empty name) or `Claimed` (a join took it and was given a token). `held` counts the seats, from seat 0, issued at creation: the creator's is always one and the rest are open. A `CHECK` ties an open seat to a null `token_digest` and no account.
- `Registry::claim(game, entrance, SeatClaim {name, user})` gives the first open seat under a trimmed name of 1 to 32 characters (the caller checks for control characters), makes its token and returns `Seated {game, seat, token}`, the one time the token exists in clear. An account that holds a seat of the game is handed it again with `token: None` and opens it through its session; guests may take several seats. `Entrance::Listing` (a game id) reaches public games only and answers `NotFound` for others, as for an unknown id; `Entrance::Code` (a code resolved by `find_by_code`) reaches any game. `ClaimError` is `NotFound`, `Full`, `Started` (judged before `Full`), `BadName` or `Record`. A claim sets name, token and account; the seat keeps the deck it was made with.
- `Registry::release(game, seat, by)` opens a claimed seat again: the holder (`by == seat`) leaves, or the host removes a player. The token stops working, the account is free, the connection closes, `Ready` and the stops are reset and the table is told. `TableError` is `NotFound`, `NotAllowed`, `NotClaimed` (an issued or open seat), `Started` or `Record`.
- `Registry::close(game, by)` ends a game that is still in its lobby, for the host alone. In order: the actor promises that no `Ready` starts the game from then on (`GameHandle::close_lobby`; `TableError::Started` when it already runs, so a close and the last `Ready` have one winner, in the actor's order of commands); the record becomes `expired`, the status of a lobby the janitor drops, so a closed game is not loaded at boot, not listed under My games and frees its code (a failed write takes the promise back with `reopen_lobby` and changes nothing: `TableError::Record`); the entry leaves the registry, so the listing, the lookup, the code and every token stop reaching the game at once. Before the entry is dropped its `Removal` channel (`SeatAccess::removed`) says `ClosedByHost`, which is how a socket picks 4002 over the 1001 of a plain drop.
- Claims, releases, visibility changes, closes and `bind_seat` of one game run one at a time (`Entry::turn`). A claim writes the row first (guarded by `state = 'open'` and the lobby status, so racing claims get one winner), then names the seat in the actor (`GameHandle::occupy`, refused once the game runs), then the registry's slot. A release empties the seat in the actor first (`vacate`): a lobby with an open seat cannot start, so nothing begins under the change. A failed write leaves table, row and slot as they were.
- The join code is eight symbols of `ABCDEFGHJKLMNPQRSTUVWXYZ23456789` (capitals and digits without `I`, `O`, `0` and `1`; 40 bits) from the OS CSPRNG, returned in `Created::code` and in clear only there. The registry and `games.code_digest` keep its SHA-256 (unique among `lobby` and `running` games; none for games stored before codes). `Registry::find_by_code(typed, address, now)` ignores case, spaces and hyphens, compares every game's digest without stopping at a match and answers the game id or `None`; text that cannot be a code is the same `None`. Misses are counted per client network (`AttemptLimits::code_misses_per_ip`, 5, in `code_window`, 1 minute); a hit hands its count back, and over the limit the answer is `Err(Blocked)`, even for a right code.
- `Registry::lobbies()` returns the public games whose actor has not started, newest first, at most `MAX_LISTED_LOBBIES` (50), as `ListedGame {game, host, format, filled, total, created_at, age}`: `host` is seat 0's name, `filled` counts the seats that are not open, `created_at` is `YYYY-MM-DDTHH:MM:SSZ` (UTC). It holds no token, code, digest or link, and a full lobby stays in it.

### Game browser endpoints

`lobbies.rs` puts the registry's calls on HTTP. Bodies and codes are in [protocol.md](protocol.md), REST bodies; `POST /api/games` is in the Transport table.

| Route | Caller | Success | Errors |
|---|---|---|---|
| `GET /api/lobbies` | anyone | 200 `LobbyList` with an `ETag`; 304 when `If-None-Match` names it | 429 `rate_limited` with `Retry-After`; 503 `lobbies_unavailable` |
| `POST /api/games/{id}/join` `{name?}` | anyone; a session binds the seat | 200 `{game, seat, token}`, `token` null when the account already held a seat | 400 `invalid_request`, `invalid_name`; 404; 409 `full`, `started`; 503 |
| `POST /api/join` `{code, name?}` | anyone; a session binds the seat | as above, any game the code names | as above; 429 `too_many_attempts` with `Retry-After` |
| `POST /api/games/{id}/leave` | the holder of a claimed seat | 204 | 401; 404; 409 `not_claimed`, `started`; 503 |
| `POST /api/games/{id}/seats/{n}/release` | the host | 204 | 401; 403 `forbidden`; 404; 409 `not_claimed`, `started`; 503 |
| `PATCH /api/games/{id}` `{visibility}` | the host | 200 `{visibility}` | 400; 401; 403 `forbidden`; 404; 409 `started`; 503 |
| `DELETE /api/games/{id}` | the host | 204 | 401; 403 `forbidden`; 404; 409 `started`; 503 |

- The three routes that act on a seat need a credential and the others are open to anyone. A seat is named by `Authorization: Bearer <token>`, which needs no database, or else by the session cookie. A token wins over the session; another `Authorization` scheme (a proxy's) is not a token and is ignored. With neither, a server without a database answers 503 `accounts_unavailable` and one with accounts 401 `unauthenticated` (a stale cookie is cleared), as the other session routes do. A token or session that opens no seat of the game, an unknown game and a malformed id are one 404.
- `release` is for the host (seat 0) alone, even on the host's own seat; a player gives their seat back with `leave`. Only seats a join took can be given back: the host's seat, an open seat and a seat made with the game answer 409 `not_claimed`. A seat number over 11, or written with a sign or a leading zero, is a 404. Once the game starts, `leave`, `release`, `PATCH`, `DELETE` and a join that may reach the game answer 409 `started`.
- `DELETE` closes the game for the host alone, named by a token or the session as for `PATCH`; a seat that is not the host's answers 403 whatever state the game is in. The 204 comes after the game left the registry, so the listing, the lookup (404) and the code no longer find it, and every socket of the game, an invite game's too, is closed with 4002. A second `DELETE` finds no game (404). Like the other calls that act on a seat it is not throttled.
- A join checks the body and the name before it looks at the game or the code, so the answer to a missing name says nothing about either. A signed-in visitor needs no `name` and is named after the account unless one is sent; a guest needs 1 to 32 characters without control or invisible marks (400 `invalid_name`). Guests may take several seats of a game.
- An unknown id, a malformed id, an invite game reached by its id (started or not) and a wrong or malformed code are one byte-identical 404, so a visitor cannot find an invite game by trying ids. A code reaches any game, so a right code for a full or started game says 409 `full` or `started`, and a started game says `started` before `full`. The game's id comes back only in the `Joined::game` of a join that succeeds: no refusal and no 429 carries one, so a right code is the one way to learn the id of an invite game, and the page that joins by code opens the game from the answer.
- `POST /api/join` counts wrong codes per address (5 a minute, [Accounts](#accounts)); the sixth try is 429 with `Retry-After`, even for the right code. A bad body or name is refused before the code is counted. `GET /api/lobbies` counts every request, a 304 too, 30 a minute per address (`AttemptLimits::lobby_requests_per_ip` in `lobby_window`). The address is the one the throttle uses everywhere ([Accounts](#accounts), `MTG_TRUSTED_PROXIES`).
- The `ETag` is a strong tag: the SHA-256 of the rows (id, host, format, filled, total and `created_at`, each length-prefixed) cut to 128 bits, so it holds nothing secret and does not change as games age. `If-None-Match` is read as a list, with `W/` prefixes ignored and `*` matching any listing. A client that gets a 304 keeps its rows and counts their age on from the time it received them. `lobbies_unavailable` is for a registry that cannot list, which is a server shutting down.
- A seat that is taken away closes its connection: a removed or departing player's socket gets 4001 ([Transport](#transport)); the actor tells the socket apart from a replaced or dropped one by the seat being open (`GameStatus::open`). A game that leaves the registry closes every socket of every seat at once: 4002 when its host closed it, 1001 otherwise.

## Cards and decks

`Server::open_with(config, pool, cards)` takes the `CardSource` (database, rules files, `Caps`) that the games, the card endpoints and the deck check share; `Server::open` makes one. A server without a database answers 503 `cards_unavailable` and 503 `decks_unavailable`.

### Cards

Public and read-only (`card_api.rs`).

| Endpoint | Behaviour |
|---|---|
| `GET /api/cards` | `CardSearchPage {cards, total, page, per_page}` over the deck cards (tokens and emblems are never listed). |
| `GET /api/cards/{oracle_id}` | `CardDetail`: the hit, oracle text, keywords, faces, Scryfall's legality per format, printings (newest first), rulings (oldest first) and `reasons`, the clauses the game cannot run, each with a face and a span of that face's oracle text. A malformed and an unknown id are one 404; capitals in the id make no difference. |

| Parameter | Meaning |
|---|---|
| `q` | Free text: the name contains it (case and accents ignored) or the rules text matches it in web-search syntax (`"phrase"`, `-word`, `or`). With `q` the order is by relevance (name equal to `q`, name starting with it, name containing it, rules text only), then by name; without it by name. Ties break by oracle id, so every order is total and pages never overlap or skip. |
| `colors`, `colors_match` | A card's colors as letters of WUBRG in any case, or `C` for colorless cards; `colors_match` is `includes` (default), `exactly`, `within` or `overlaps`. |
| `identity` | A color identity in the same letters: a card matches when its identity lies within them, so `identity=BG` lists what a black-green commander may play and `identity=C` the colorless identities. |
| `type` | Words that must each appear in the type line (`legendary creature`). |
| `cmc` | A mana value (`3`), a range (`2-4`) or a minimum (`5+`). |
| `format` | A Scryfall format key (`commander`, `modern`): the card is legal or restricted there. |
| `page`, `per_page` | `page` counts from 1 up to 10 000; `per_page` defaults to 40 and a larger value is lowered to 100. The answer repeats both and carries the total. |

Blank values are no filter; an unknown parameter or a malformed value is 400 `invalid_request` whose message names it. A hit (`CardHit`) carries the oracle id, name, mana cost, mana value, type line, colors, color identity, the art of the card's default printing (the newest paper one; `images` holds the card's links or one entry per face) and `support`: the state `Library::support` gives with the server's rules and `Caps`.

### Decks

`decks.rs`, `deck_cards.rs` and `deck_import.rs`. Every route needs a signed-in account: 503 `decks_unavailable` without a database, then 401 `unauthenticated`. A deck of another account is a 404, like an unknown id. Bodies are `application/json` of at most 64 KiB (640 KiB for an import), the origin guard covers every verb but GET, and CORS allows PUT and DELETE. The tables `decks` and `deck_cards` belong to `mtg-db` (migration `20261019000007_decks.sql`, `DeckRepo`): `owner` is the account id with no foreign key, since `users` is the server's own table, so deleting an account has to delete its decks; a line's card is a foreign key and its printing is set to null if the printing goes.

| Endpoint | Behaviour |
|---|---|
| `GET /api/decks` | `DeckList {decks}`, the most recently saved first: id, name, format, timestamps, the derived `state`, `verified`, `changed`, `checked_at` and the `summary` of the check of the moment. |
| `POST /api/decks` | `{name, format, notes, cards: [{zone, oracle_id, printing_id?, count}]}` stores a deck: 201 `DeckView`. |
| `GET /api/decks/{id}` | `DeckView`: the lines (each a card as a search hit, in the art of the chosen printing), the `check` stored at the last save, the `report` of the moment and the derived `state`, `verified` and `changed`. |
| `PUT /api/decks/{id}` | Replaces name, format, notes and every line in one transaction, runs the check again: 200 `DeckView`. |
| `DELETE /api/decks/{id}` | 204. |
| `POST /api/decks/check` | `{deck}` (a stored deck) or `{format, cards}` (an unsaved list, under the rules of a save): the `DeckReport`, nothing saved. |
| `POST /api/decks/import` | `{text, source?, format?}`: `ImportResult`, nothing saved. |
| `GET /api/decks/{id}/export?format=text\|arena\|mtgo` | The deck as `text/plain` (`text` by default) with a `Content-Disposition` file name. |

- A save is refused with 400 `invalid_deck` for a name outside 1 to 100 characters or with control characters, notes over 10 000 characters, a format outside `SUPPORTED_FORMATS` (`commander`, `freeform` and the constructed formats), more than 250 lines or 250 cards, a count outside 1 to 999, an id that is no id, or a printing that is not the card's; with 400 `unknown_cards` for oracle ids that are no deck cards of the database. An account stores at most 100 decks (409 `deck_limit`); the count and the insert run under an advisory lock of the owner, so racing saves cannot pass it.
- Saving runs the check (`mtg_cards::deck::deck_report` with the server's rules and `Caps`) and stores `{deck_hash, coverage_hash, checked_at, summary}` in `decks.last_check`. The marker is never stored: every read derives it from that check and the report of the moment (`mtg_cards::deck::marker`, [deck-check.md](deck-check.md)). `verified` is true only when the stored check is clean, both hashes equal the ones computed now and the report of the moment is clean. An edit, other rules for a card of the deck or another `Caps` make the deck `unchecked` with `changed`; a ban or a card the engine no longer plays makes it `blocked` or `manual_play`; a check that was not clean never verifies. Reading writes nothing, so a deck whose cards the engine learned to play is marked by saving it again.
- Import reads a pasted list in any format `mtg-import` reads (`source`: `text`, `arena`, `mtgo`, `mtgo_dek`, `moxfield`, `archidekt`; detected when absent; `format: commander` makes Moxfield's first line the commander), at most 100 000 characters and 1 000 lines. The answer holds the `cards` it resolved (a printing when the set and number name one of the resolved card), the `unresolved` lines (`unknown_card` or `ambiguous_name`, with suggestions that are never applied), the `problems` (lines that are no entries) and the `ignored` parts (the maybeboard, Moxfield's stickers). Only the names of the list are looked up, so no card index is built per request.
- Export writes split cards as the format does (`A /// B` for Arena, `A/B` for MTGO) and other multi-faced cards by their front face; printings are written where the format has them. A deck the format cannot hold, MTGO text with a commander and nothing else, answers 422 `export_failed`.

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

- A decision gets its clock when issued (`Running::settle`, `Pace::clock_for`). A `Priority` decision of a seat that is not the active player gets `MTG_RESPONSE_SECS` (10 s), whether it can act or not, because a clock that read `can_act` would show what a hand holds; so do its `ChooseTargets` (a spell or a triggered ability), `OrderTriggers`, `YesNo`, `ChooseOption`, `ChooseMode`, `ChooseCards` and `OrderCards`, which the table waits for just as long. `DeclareBlockers` of a defending seat gets `MTG_BLOCK_SECS` (45 s). Every other decision has none, and none of the active player's does.
- The clock ignores the mode, runs while the seat is away, stops with any answer and restarts when a game is resumed. `deadline_ms` carries the time left ([protocol.md](protocol.md), Decisions).
- At the deadline the actor answers with `Running::fall_back` through `Running::answer`, so a late answer is the ordinary `StaleDecision`. The answer is a pass, no blocks, no target (`Cancel` for a spell, which the rules take back at no cost; `Remove` for a triggered ability, which they take off the stack when its target cannot be chosen) or the triggers in the order they triggered. A seat's own `Remove` is refused with `IllegalAnswer`: a player with a legal target has to choose one, so only the clock removes a trigger.
- The answers to the questions of effects: `YesNo` is No; `ChooseOption` is the first option; `ChooseMode` is `Cancel` when `can_cancel` allows backing out of the cast, else the first mode (the first `min` modes when a choice needs more, the first repeated when the list is shorter and a mode may repeat). A mode is named by its `index`, not its place in the list. `ChooseCards` is the first `min` cards of `from` in the order offered (none for a `min` of 0, all of them when fewer are offered; `min <= max`, so never more than the most) and `OrderCards` is the cards in the order given; both always have an answer. A choice with no option or no mode to give has no answer: the clock stops and the decision waits. The clock and the answer read the kind and its options alone, never the seat, its hand, stops or Full Control, so they show nothing a client does not already hold. The loop selects between the next command and the pending `due` time, commands first, because a command already waiting was sent before the clock ran out. A failed save restores the engine and retries a second later.

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
| `MTG_RESPONSE_SECS` | 10 | Clock on a non-active seat's priority, target, trigger-order, yes-or-no, option, mode, card and card-order decisions; `0` for none, at most 86 400. |
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
- Clocks, the dwell and the leak test run on tokio's paused time; `tests/pace_leak.rs` plays two games that differ by one hidden card, in combat windows and with a spell or a triggered ability on the stack, and requires the other seat to be sent the same messages in the same batches; `tests/response_clock_effects.rs` covers the clocks of the target and order decisions through the actor. No card asks a yes-or-no, option, mode, card or card-order decision yet, so their clocks and answers are tables in `pace.rs` (every seat, every setting, every shape of options).
- Transport tests (`tests/net`) use a real listener on port 0, raw TCP for hostile HTTP and tokio-tungstenite for sockets; `FlakyStore` fails writes on demand.
- `tests/pg_cards_api.rs` (Postgres, real HTTP, the importer's Scryfall fixture) checks searches by text, color, identity, type, mana value and format with their exact results and order, paging, the page cap, each kind of 400, the hit, the card page, the support state under an engine that runs everything and one that runs nothing, and the server without a database; `tests/pg_decks.rs` (the rules corpus with assigned legalities) checks access (503, 401, foreign origins, another account's deck), the saved deck read back and verified, manual and illegal decks, the marker removed by an edit that bypasses the check, another engine, other rules and a ban and restored by saving (reads write nothing), the check endpoint, every refused save, the cap of 100 decks, printings, import line by line, and every export read back through the importer. Unit tables cover the query string, the deck rules and the import.
- Open games: `visibility.rs`, `open_seats.rs` (claims, releases, racing claims, a release racing the last `Ready`, entrances, the roster, the start closing every change), `join_codes.rs` (typing, misses, the throttle) and `lobby_listing.rs` (what is listed, order, the cap, no secret, the request limit); on Postgres `pg_visibility.rs`, `pg_join_codes.rs` and `pg_open_seats.rs` (rows written, racing claims, failed writes, a lobby with claims after a restart).
- The game browser over real HTTP (`tests/net/lobby.rs` makes games with open seats): `lobby_http.rs` (what is listed and in what order, the cap, the `ETag` and 304 in every spelling, the tag ignoring age, the request limit and its window, a scan of every answer for tokens, codes and links), `join_http.rs` (a guest by id and by code however typed, the game's id in the answer and only a right code naming it, the byte-identical 404 for unknown, malformed, invite and wrong-code, `full` and `started`, names, bad bodies, the wrong-code throttle per address and its window), `seat_http.rs` (leave, release and `PATCH` for holder, host and stranger, the visibility the lookup reports, `not_claimed`, `started`, the foreign origin on every route, CORS for the new methods and headers), `ws_removed.rs` (4001 for a removed or departing player, 4000 and 1013 unchanged), `game_close.rs` (the close by the host of a public and an invite game, gone from the listing, lookup, code and tokens at once, 4002 on every socket, a joiner refused, `started`, a stranger and an unknown id sharing one 404, the registry's `Removal` reasons, and a close racing the last `Ready` over many rounds) and, on Postgres, `pg_join_http.rs` (accounts joining by id and by code, the repeat join with a null token and the game named, an account in an invite game reaching it by its code and not by its id, the session as credential, a refused write changing nothing) and `pg_game_close.rs` (the `expired` record, no return after a restart, the host's session and a joiner's account, 4002 for session sockets, a close the database refuses leaving the lobby able to start).
- Postgres tests (`pg_*.rs`) need `docker compose up -d db` and use `#[sqlx::test(migrations = false)]` with `migrate`, because the default would use `_sqlx_migrations`.
