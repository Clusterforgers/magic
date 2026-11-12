# Server

The `mtg-server` crate: one actor per game behind an axum REST and WebSocket transport, with Postgres persistence, accounts and the pace of play. Wire contract: [protocol.md](protocol.md); client: [web.md](web.md). Routes are registered in `server.rs` and bodies are `mtg-protocol` types (REST bodies); this file holds the rules behind them.

## Shape

- `spawn_game(id, setup, store)` starts a game in its lobby and `resume_game(store, id, setup)` rebuilds one from its log; both return a cheap-clone `GameHandle`. `status()` carries the state hash and is for tests and admin views only.
- One task per game owns the `Engine`, the `EventStore` and a bounded queue per seat; a full or closed queue drops that client, which reconnects for a snapshot, so the actor never waits. The transport maps a credential to one seat and the actor trusts it.
- A step checks the seat, the `DecisionId` and engine legality, applies the answer, projects for the connected seats, appends to the store, then sends the events, the next `Decision` (to its seat only) and `GameOver`. A store or projection failure restores the engine from its `Checkpoint` and answers `Error{Internal}` without details.
- Snapshots and batches are built per `PlayerId`; the spectator viewer (`Seats::spectator`, the number of seats) gets the spectator view (`StateView::for_spectator`). While a spectator is connected each batch is projected for it once and queued to every spectator (`Seats::watch`), with presence, `Deciding` and `GameOver`. A spectator is no seat: never in `connected`, no clock, leaves without a word.
- The lobby has no engine until every seat has sent `Ready`; readiness is not stored and outlives a connection. The last `Ready` creates the engine and broadcasts the opening events from seq 0; on failure a repeated `Ready` retries. Chat (at most 500 characters) goes to the connected seats and is not stored.
- The roster (`ServerMessage::Lobby`) lists every seat and is sent after each change. An open seat is listed with an empty name, not connected and not ready, so it counts in `needed` and blocks the start; nothing sent as an open seat counts.
- Presence (`presence.rs`): after every command the actor sends each change in the connected seats to the other connected seats as `Presence` ([protocol.md](protocol.md), Decisions); never stored or hashed.
- Deciding (`Running::deciding_news`): after a step's decisions are sent, the actor broadcasts `Deciding` when the pending decisions (a held-back dwell as `Priority`) differ from what the table was last told; a joining client hears it after its snapshot.
- A game made from a recipe keeps one deck per seat in the lobby (`SeatDecks`, [Choosing a deck](#choosing-a-deck)): a `Ready` from a seat without one is answered `Error{BadMessage}` to that seat alone and changes nothing. A game from a bare setup (`Registry::register`, `spawn_game`) needs no decks.
- `Registry` holds the live games by random v4 `GameId`. A seat token is 256 random bits kept only as its SHA-256; `authenticate` compares every seat so timing does not tell which matched. `MTG_MAX_GAMES` caps creation.
- The janitor sweeps every 30 s and drops finished games after 10 min, unstarted lobbies after 1 h and started games with nobody connected for 24 h (`Retention`), closing their sockets with 1001 and marking the record `expired` or `finished`.

## Transport

`Server::new(config)` keeps games in memory, `Server::open(config, pool)` also records them in Postgres, `serve(listener, shutdown)` runs axum. Errors are `{error, message}`; every response is `no-store` (pictures set their own) and `nosniff`, since it can hold tokens and live state. JSON is gzipped for clients that accept it (`CompressionLayer`, default predicate).

- `POST /api/games` takes `{format, seats, visibility?, held?, house_rules?}` (a seat may be a bot, [Bot seats](#bot-seats)) (at most `GAME_BODY_LIMIT`, 256 KiB; unknown fields refused; a server-random seed) with two or more seats; `visibility` is `public` or `invite` (default); the first `held` seats get a token (default all), the rest are open and absent from `seats`. A seat's `deck` is a demo deck id (Normal), `precon:<code>` or, for seat 0, the creator's stored deck, else chosen later. Errors: 400 `unknown_deck`, 422 `deck_blocked` (no known card; legality only warns), 400 `unsupported_format`.
- `GET /api/games/{id}` reports the lobby facts (format, started, seats, connected, ready, result, visibility, manual play, house rules when set); a malformed and an unknown id are one 404.
- The origin guard (`origin.rs`) covers both socket routes and every method but GET, HEAD and OPTIONS, so other sites cannot act with a visitor's cookie: a request without `Origin` (a bot) passes, else the origin must be in `MTG_ALLOWED_ORIGINS` or name the `Host` authority, or it is 403 `forbidden_origin`. CORS answers listed origins only, with credentials.
- `GET /ws/{game}/{token}` is a seat's link; `GET /ws/{game}` the seat bound to the session's user; `GET /ws/{game}/watch` a signed-in spectator's (Spectators). A socket checks the origin, then the credential (one 404 for unknown game, malformed id and wrong token), then expects `Join` ([protocol.md](protocol.md), Sync contract): a wrong version gets `Error{UnsupportedVersion}`, a foreign game `Error{UnknownGame}`, else `Error{BadMessage}`, each closing with 1002.
- A second socket for a seat replaces the first. A closing socket drops its queue and never calls `leave`, which would close its replacement.
- Shutdown (SIGTERM, ctrl-c) stops accepting, sends every socket 1001 and waits up to 5 s, then abandons what is still open, so a stalled request cannot hold the process.

Limits per socket: `Join` within 10 s of the upgrade; frames and messages of 64 KiB; a burst of 40 messages, then 20 per second (the client paces under it, `web/src/net/pacer.ts`); 5 malformed messages in a row, each answered `Error{BadMessage}`; a ping every 20 s and a close after 60 s without a pong.

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
| 4001 | The seat was taken away (its player left or the host removed them): its token opens nothing, do not reconnect |
| 4002 | The host closed the game before it started: links and code open nothing, do not reconnect |

## Persistence

- `PgEventStore` implements `EventStore` over `game_events` (primary key `(game_id, seq)`). An append is one transaction under a per-game advisory lock that checks the log ends at `from_seq`: a gap, repeat or lost race returns `OutOfOrder` and a failure leaves no rows. Reads detect gaps and undecodable events (`StoreError::Corrupt`). There is no foreign key to `games`, so a log can be stored first.
- Migrations (`crates/mtg-server/migrations`) are tracked in `_mtg_server_migrations`, not `mtg-db`'s `_sqlx_migrations`, so both crates migrate one database in any order, repeatedly and concurrently. The table name is an on-disk contract; `mtg_server::migrate(&pool)` runs them.
- `games` holds the recipe, result, status (`lobby`, `running`, `finished`, `expired` when dropped by the janitor or closed by the host, `failed` when boot could not load it), `failure`, `failure_reason`, visibility and join code digest. `seats` holds name, state (`issued`, `open`, `claimed`), token digest (null while open), account and `hidden` (the account removed the failed game from its list).
- `Registry::create` writes the game and seats in one transaction before the actor exists, so a failed write creates nothing (503 `unavailable`). The actor's `Recorder` notes each seat's first join, the start and the end; a failed write is logged and never stops a game, because the log is the truth and boot repairs a lagging status.
- `GameRecipe` is the only way to build a `GameSetup`, which holds an `Arc<dyn FormatRules>` and cannot be stored. Its JSON is an on-disk contract: unknown fields and variants are refused and a new field is left out at its default, so older recipes read unchanged. It is written at creation and rewritten once at the start with the chosen decks.
- `DeckRef::Oracle` names library, commanders and companion by oracle id with each line's printing, which only labels the card (`ObjectView.identity.printing_id`) and is outside fingerprint and state hash. A Commander seat without a deck holds a stand-in (`DeckRef::Demo {id: "commander-stand-in"}`, a legendary creature and 99 Forests, or `commander-stand-in-<n>` for another size) that cannot be ready and lets the table pass its deck check before anybody chooses.

### Resume

- `Server::open` loads every `lobby` and `running` game oldest first, rebuilds the setup from the recipe, checks the seat rows against it and calls `resume_game` with the stored digests, so old tokens open the same seats. A game whose log shows it ended is only corrected to `finished`.
- A recipe, seat rows or log that do not load (a replay panic included), a changed or lost deck or card and a started game with an unheld seat mark the game `failed`; only an unreachable database stops boot. A `failed` game is not retried (fix the row and set `running` by hand) or loaded, so its tokens, lookup and sockets answer 404.
- A failure has two texts: `failure`, the operator's detail (error, seat, fingerprints), also logged; and `failure_reason`, one sentence for the players from `Cause` (`registry/cause.rs`) with no internals. My games shows "This game could not be restored, so it cannot continue." for a `failed` game without a reason.
- A lobby returns with its visibility, join code and claims, nobody ready, and its age: past `Retention::lobby_after` (database clock) it becomes `expired`. A started game with an open seat does not load; a running one gets a fresh 24 h without a connection. `MTG_MAX_GAMES` limits creation only.
- Lobby choices are not stored until the start: a restarted lobby reads its recipe, so a seat returns on its built-in deck, and a seat whose recipe names a stored deck has none until it chooses again.

### Deck check

- A new game's recipe stores a `DeckFingerprint` per seat: the SHA-256 of a canonical, versioned text of the ordered engine cards (`v1:<64 hex digits>`) plus commanders and companion. `Engine::resume` applies stored events as facts, so without it a log would replay under other cards silently.
- `GameRecipe::build` recomputes it and fails with `RecipeError::DeckChanged` for the first seat whose cards differ in any field, order or size; resume marks the game `failed` before replaying anything. A recipe without fingerprints resumes unchecked, with a warning at every boot.
- It covers the engine's definitions, so an edited card record, rules file or `card_rules` row, or an engine that carries more of a card, fails the game at the next boot; a running game keeps its definitions. Seed and format rules are not covered. A new `CardDefinition` field must be left out of the JSON at its default, or every fingerprint changes.
- A deck of oracle ids is assembled by `CardSource` ([cards-dsl.md](cards-dsl.md)); an id with no row fails with `CatalogError::NotFound`.

### House rules

- `house_rules` (`HouseRules`, [engine.md](engine.md), House rules) is optional per rule; an absent rule keeps the format's. A value out of range, or a handicap beyond `seats`, is 400 `invalid_request` before anything is made, with the rule in `field` (`handicaps[1].starting_life`); an unknown rule or a wrong type is 400 without one.
- The rules are part of `GameRecipe` (`games.recipe`, `jsonb`), left out while none is set; a migration renames the older key `deck.colour_identity` to `color_identity`, which still reads. Resume and replay play under them; a stored recipe with rules out of range fails with `RecipeError::Setup`.
- A game checks its decks with the rules when it starts (`GameSetup::validate`); the server judges a chosen deck and the lobby's deck check with the same rules ([Decks](#decks)). The lobby row and the table view show the rules to every seat.

## Accounts

- `username_key` is the lower case and unique, so names are unique ignoring case. A session is the SHA-256 of its cookie token. Without `DATABASE_URL` every account endpoint answers 503 `accounts_unavailable` and games are guest-only. A database failure is 503 `unavailable`, and a login it stops costs no throttle budget.
- A username is 3 to 32 ASCII letters, digits, `_` or `-` (no look-alikes); a password 10 to 128 characters, compared byte for byte, never normalised. Passwords are Argon2id PHC strings, so the cost travels with the hash (default the OWASP minimum: 19 MiB, 2 passes, 1 lane), hashed on `spawn_blocking` behind a semaphore (`AuthConfig::hash_jobs`) so a burst cannot exhaust memory.
- Login hides whether a username exists: an unknown name, a wrong password and a rule-breaking input get one byte-identical 401, and an unknown name costs one Argon2 check against a stand-in hash. Registration does say "taken". `register` answers 403 `registration_closed` before the body when `MTG_REGISTRATION=closed`.
- An unparseable body on a password route is 400 `invalid_request` with one fixed message quoting nothing; other routes name the missing or unknown field. Messages state the rule, never the input. Passwords, tokens, digests and hashes never reach logs or `Debug`; failed attempts are logged without the name tried.
- The cookie `mtg_session` holds 256 random bits as hex and nothing else: `HttpOnly; SameSite=Lax; Path=/; Max-Age=2592000` (30 days), no `Domain`, plus `Secure` when `MTG_COOKIE_SECURE=1`. Expiry is absolute and judged by the database clock; an upkeep task purges expired rows hourly. A sign-in that presents a session ends it, so a browser holds one.
- Guessing is throttled in memory per process (`throttle.rs`): per 15 minutes, failed logins per address (30) and per username (10, existing or not) and registrations per address (10); per minute, join code misses (5) and lobby list requests (30) per address. A login counts at its start and is returned on success or server failure; over the limit is 429 with `Retry-After`, even for the right password. IPv6 counts by /64; past 50 000 counters new keys go unlimited until a sweep.
- The client address is the socket's peer. Behind a reverse proxy, `MTG_TRUSTED_PROXIES=n` takes the `n`-th entry from the right of `X-Forwarded-For`, which the nearest trusted proxy wrote, so forged entries to its left are ignored; with 0 the header is ignored.
- `GET /api/auth/options` is the one public hint about the instance: whether registering is open.

### Admin

- `users.is_admin` marks the accounts that make the others. `MTG_ADMIN_USERNAMES` and `MTG_ADMIN_USERNAME` name them, ignoring case (an invalid name refuses the start); at each start `Server::open` sets `is_admin = (username_key = ANY(names))` on every row, so an unlisted account loses admin. A name without an account logs a warning; there is no bootstrap path ([deploy.md](deploy.md)).
- Every admin route reads `is_admin` from the database for the session's account on each request, so a demotion applies at once. The checks run in this order, all before the body is read: 503 `accounts_unavailable`, 401 `unauthenticated` (a cookie the server does not honour is cleared), 403 `forbidden` for a non-admin.
- `POST /api/admin/users` follows `register`'s rules and errors whether registration is open or not, starts no session and shares the Argon2 semaphore, under a per-admin budget (`AttemptLimits::admin_creates_per_account`; a refused try costs nothing, a taken name counts) apart from the per-address registration count.
- `POST /api/admin/users/{id}/password` and `DELETE /api/admin/users/{id}` answer 404 `unknown_account` and 409 `admin_account` (an admin's password changes only in Account settings) before the body. A reset sets a fresh hash and deletes the account's sessions in one transaction (`Accounts::reset_password`); removal is `Accounts::delete_user`, then `Registry::forget_user`. Both use the admin's creation budget and log both account ids.
- `Accounts::delete_user` and `reset_password` lock the account's row first (`SELECT ... FOR UPDATE`) and stop with `Outcome::Admin` or `Outcome::Gone`, so a promotion or a second removal cannot slip in between the route's check and the change.
- The settings routes (change password, delete account) act on the session's account and need its current password, checked and counted like a login; a rule-breaking one is refused as wrong, unhashed and uncounted. A password change deletes the account's other sessions in the same transaction.
- No admin account can be deleted: `DELETE /api/auth/account` answers 409 `admin_account` for one before the body is read and counts no password try ([deploy.md](deploy.md) covers a mistyped name).
- Deleting an account is one transaction (`Accounts::delete_user`): the owner's lock, `delete_all_of` on `DeckRepo`, `FixMarkRepo` and `ReadRepo` (a card only it approved needs a read again), then the `users` row, which cascades to sessions and unbinds seats (`ON DELETE SET NULL`). `decks.owner` and `card_fix_marks.user_id` have no foreign key, hence the explicit deletes. Game records keep the seat names.
- After the commit `Registry::forget_user` treats the account as leaving its live games: a lobby seat a join took is released, a lobby it hosts is closed, and any other seat keeps its token without an account, so the game waits on its clock as for any absent player. A seat the database refuses to change is logged and counted in `Departure::failed`. The username is free again.

### Seats and accounts

- `seats.user_id` is unique per game. A signed-in create binds seat 0 to the creator, and the first signed-in join through an unbound seat's link binds that seat; guests and unrecorded games bind nothing. One guarded `UPDATE` decides (`Registry::bind_seat`), so racing accounts get one winner and a bound seat never changes hands. A failed binding is logged and never stops the join.
- Binding adds a way in and takes none away: old links keep working, and another signed-in account using a bound seat's token plays like a guest and is not bound.
- `GET /ws/{game}` answers, after the origin check, 404 for an address that is not a game id, 503 `accounts_unavailable`, 401 `unauthenticated` (a bad cookie is cleared), then one 404 for an unknown game and for a game the user holds no seat in.
- `GET /api/me/games` lists the user's `lobby`, `running`, `finished` and `failed` games from the database, newest first, at most 100. A failed game shows its `failure_reason` to its seat holders only; lookup, sockets and the game browser answer 404 for it.
- `DELETE /api/me/games/{id}` hides a failed game from the user's list (204) by marking their seat `hidden`; the record and the other players keep it, so a repaired (`running`) game shows for all. A game not listed, not failed or already removed is 404.

## Open games

A game has a visibility and each seat a state, so a visitor can claim an open seat by the game's id or with its join code ([ui-design.md](ui-design.md), Game browser and lobby).

- `Visibility` is `public` or `invite` (default). A public game in its lobby is listed; an invite game is reached only by its seat links and join code. `Registry::set_visibility(game, by, visibility)` is for the host (seat 0) before the start.
- A seat is `Issued` (its token was made with the game: the seat link), `Open` (no token, no account, an empty name) or `Claimed` (a join took it and was given a token). A `CHECK` ties an open seat to a null `token_digest` and no account.
- `Registry::claim` gives the first open seat under a trimmed name of 1 to 32 characters and returns its token, the only time it is in clear; an account already seated gets its seat back with `token: None`; guests may take several. `Entrance::Listing` (a game id) reaches public games only, `Entrance::Code` any; `ClaimError::Started` is judged before `Full`. The seat plays the visitor's deck, else the one it was made with.
- `Registry::release(game, seat, by)` reopens a claimed seat when its holder leaves or the host removes them: the token dies, the account is freed, the connection closes, `Ready` and stops reset and the table is told. Other seats get 409 `not_claimed`; a seat number over 11 or written with a sign or leading zero is 404.
- `Registry::set_teams(game, by, teams)` is for the host before the start (`PUT /api/games/{id}/teams`, body and answer `TeamsBody`). The actor checks the teams against the table (400 naming `teams`), takes back every Ready, keeps the teams in the setup and the lobby's recipe, and sends the roster; a started game answers 409 `started`. The creation request carries the first teams in `house_rules.teams`.
- `Registry::close(game, by)` ends a lobby, host only: the actor refuses any later `Ready` (`close_lobby`; `TableError::Started` if running, so close and last `Ready` have one winner), the record becomes `expired` (not loaded or listed, code freed; a failed write calls `reopen_lobby`), and the entry leaves the registry. `Removal` says `ClosedByHost` first, so sockets close with 4002; then `DELETE /api/games/{id}` answers 204.
- Claims, releases, visibility changes, closes and `bind_seat` of one game run one at a time (`Entry::turn`). A claim writes the row (guarded by `state = 'open'` and the lobby status), then the actor (refused once running), then the slot; a release empties the seat in the actor first. A failed write changes nothing.
- The join code is eight symbols of `ABCDEFGHJKLMNPQRSTUVWXYZ23456789` (40 bits, OS CSPRNG), in clear only in `Created::code`; the registry and `games.code_digest` keep its SHA-256 (unique among `lobby` and `running` games). `Registry::find_by_code` ignores case, spaces and hyphens and compares every digest without stopping early. Misses count per client network, a hit returns its count, and over the limit even a right code is `Err(Blocked)`.
- `Registry::lobbies()` returns the public games whose actor has not started, newest first, at most `MAX_LISTED_LOBBIES` (50), as `ListedGame`. It holds no token, code, digest or link, and a full lobby stays in it.

### Spectators

- Any signed-in user watches any running game, public or invite, through `GET /ws/{game}/watch` (the session cookie; no host's permission and no link). No session is 401 (503 on a server without accounts), a malformed address and an unknown game are 404 (`Registry::authenticate_spectator`). A game that has not started closes the socket with 1001.
- A spectator's socket joins like a seat's; `Welcome.player` is the spectator viewer, then the snapshot of the spectator view: public information only, never a hand or a library. It acts on nothing: a message is answered `Error{BadMessage}`, and `Resync` closes it with 1013 so the client reconnects for a fresh snapshot.
- The seats hear `Spectators {count}` whenever the number of spectators changes (`Seats::spectator_news`, swept on every turn of the actor's loop, a spectator's hang-up included) and on joining while some watch.
- `GET /api/lobbies` lists, to a signed-in visitor only, the running games of either visibility after the public lobbies, each row `running: true` (`Registry::running`), so the game browser offers to watch them; a guest sees the public lobbies alone (the answer varies by `Cookie`).

### Tutorial table

- `POST /api/tutorial` (no body, no account needed) answers 201 `{game, seat, token}`: a running Commander game of two seats made by `tutorial::setup` and `Registry::register_tutorial`. It has no recipe, so it is never recorded, never listed, absent from My games and gone after a restart; it is invite only, every seat is held and no clock runs (`Pace` without lengths). Seat 0 is the player, named after the account or "You"; seat 1 is "Practice bot".
- The setup skips the opening (`Start::At` the player's first main phase) with fixed hands and libraries, top card first: the player holds Forest, Grizzly Bears and Hero in Training over two Forests (nothing white is castable before the Plains), draws Plains, Giant Growth and Forest, and has Giada, Font of Hope in the command zone; the bot holds Swamps and Dregscape Zombie over a Swamp and starts at `BOT_LIFE` (6). Every card is a printing of a Verified corpus card from `tutorial/scryfall.json`, with its oracle and printing ids, so it shows its art and Oracle text. Before answering, the request gives the player the lasting stops of `PLAYER_STOPS` (own declare attackers and postcombat main, the web's defaults), so the web sends none and the decisions never depend on saved preferences.
- `tutorial::spawn_bot` joins seat 1, sends `Ready` for both seats and answers each decision with `bot_answer`, which reads the decision alone: the first land, else the first castable spell, else a pass; every creature attacks the player; no blocks; anything else the table's fallback (`pace::fallback`). The lesson therefore meets the same decisions every time: `tests/it/tutorial.rs` plays a script of every decision the player is sent, asserts each scripted action is legal and affordable when asked, and writes the trace the web lesson is tested on (`web/src/features/learn/tutorialTrace.json`, refreshed with `MTG_UPDATE_BINDINGS=1`).

### Bot seats

- A seat of `POST /api/games` with `bot: true` is played by the server (`bots.rs`): a held seat other than seat 0, with a deck (400 `invalid_request` otherwise), named "Bot" ("Bot 1", "Bot 2", ... when there are several). Its `level` (`easy`, `normal` or `hard`; Normal when left out, 400 on a seat without `bot`) sets how well it plays. `GameRecipe.bots` lists the seats as `{seat, level}`, left out while empty, so a resumed game hands them to bots again; a bare seat number, as older recipes store, is an Easy bot. The create answer carries the seat's token like any held seat's; the web keeps none of them as an invite.
- `spawn_bot_seat` joins the seat, sends `Ready` (again whenever a `Lobby` roster shows its seat not ready, as after a change of teams) and answers each decision of its seat after `ServerConfig.bot_delay` (`BOT_DELAY`, 800 ms) through `GameHandle`, as a client would, so every answer is checked, logged and paced like a player's. It keeps reading while it waits, and joins again a second after its connection is dropped.
- `BotBrain` answers with the `SeatBrain` of its level (`mtg-testkit`, `brain/`), seeded from the game's seed and the seat. Easy plays a land whenever it can and otherwise a random legal answer (`RandomBot`). Normal and Hard are a `HeuristicBot`: before each answer the bot fetches a copy of the engine (`GameHandle::table`) and thinks on a blocking thread. After `TURN_ANSWERS` (120) answers in a turn it only passes until the turn changes. A refused answer is followed by the table's fallback (`pace::fallback`); a refused fallback, or a decision with none, concedes the seat. A bot therefore never stalls a game, with or without clocks.
- A `HeuristicBot` reads the table only through `Seen`: the shared zones, its own hand and every count, never another hand or a library. It plays for its team: teammates are its own side in every evaluation, so it never attacks or harms them, defends them where the rules let it block, and attacks the opponent its creatures can kill this turn, else the leader, as its teammates do.
- It decides by simulation on clones of the engine (`Search`): an answer is applied and the game played on to a horizon (the top of the stack resolved, the end of combat, or the end of the next turn's combat), every seat answering quickly (passes, attacks and blocks from the battlefield, its own questions by heuristics, other seats' questions from the question alone), and the position scored by `evaluate`: each team's life (worth more the lower it is), cards in hand, permanents (creatures by power, toughness and combat keywords), mana sources (the first six worth most), and the damage each side's creatures threaten next turn; the strongest opposing team counts half again.
- Priority: a land in its own main phase (the one that lets it cast the most of its hand), then, in its main phase, in response to an opposing spell, in combat or at an opponent's end step, the play whose outcome beats passing (outside combat it must change something that outlasts the turn, so pumps wait for combat; a play that adds a mana source counts extra while it has fewer than five). Attacks: the quick attack (what no defender can block, kill and survive; everything when lethal). Blocks: greedy on the engine's combat forecast, so it chumps only to survive. A creature effects keep from attacking and blocking (Pacifism) counts for little. Targets: the likeliest few (the most valuable opposing permanent or spell, its own best, the opponent most worth hitting) simulated through resolution. Mulligans keep two to five lands in seven; discards and bottoms take the least useful card; other choices are searched among a few candidates.
- Normal (`Settings::NORMAL`) runs up to 40 simulations a decision (300 ms cap). Hard (`Settings::HARD`) runs up to 160 (1 s cap) and searches combat: attack candidates (no attack, every creature, the quick attack, one creature more or less than the best, and each opponent at a table of several) are simulated through the defenders' blocks and the next turn's combat, in which the seat also plays its lands and permanent spells; blocks with two creatures together are tried and the best few block sets simulated; its own targets are searched inside simulations, and a main-phase play is judged with the best play that can follow it. Without the clock (`Settings::untimed`) a brain's answers depend on the seed alone. A searching brain (Very Hard) plugs in as another `SeatBrain`.

### Replays

- `GET /api/games/{id}/replay` (`Registry::replay`) answers a recorded game whose record is `finished` as `Replay {seat, names, result, steps}`; it needs a session like watching (401; 503 without accounts) after a malformed id (404); an unknown or unfinished game, and a server without records, are one 404. The game is rebuilt from its recipe and log as a resume does, so it replays under the cards it was played with.
- The account that held a seat (`seats.user_id`, from the session cookie) gets the steps as that seat saw them, its hidden cards included; any other account gets the spectator's (`StateView::for_spectator`), so a replay shows no hand, library or face-down card that seat could not see.
- `replay::cut` cuts the log into steps: a step ends where a player is given priority after an event that is not bookkeeping (priority, an empty pool, the start of a turn or step), so every step shows a change. `replay::steps` projects each with `project_events` from the state before it; the first is at `from_seq` 0, and the steps fold into the viewer's view of the final state exactly as the live batches did.

### Game browser endpoints

`lobbies.rs` puts the registry's calls on HTTP.

- The seat routes (`PUT .../seats/{n}/deck`, `leave`, `release`, `PATCH`, `DELETE`) need a credential: `Authorization: Bearer <token>` (no database needed) wins over the session cookie, other schemes are ignored; with neither, 503 `accounts_unavailable` or 401 `unauthenticated`. Listing and joining are open; a session binds the seat it joins. A credential opening no seat, an unknown game and a malformed id are one 404.
- `release`, `PATCH` and `DELETE` are for the host (seat 0) alone (403 `forbidden`); a player gives a seat back with `leave`. After the start these and a join that may reach the game answer 409 `started`; a second `DELETE` is 404. These calls are not throttled.
- A join checks the body and the name before the game or the code, so a missing name says nothing about either. A signed-in visitor is named after the account unless a `name` is sent; a guest needs 1 to 32 characters without control or invisible marks (400 `invalid_name`).
- An unknown or malformed id, an invite game reached by id and a wrong or malformed code are one byte-identical 404. A right code reaches any game, so a full or started one gets 409 `full` or `started`. Only a successful join returns the game id.
- `POST /api/join` counts wrong codes per address (5 a minute; then 429, even for a right code), after the body and name checks. `GET /api/lobbies` counts every request, 304s too (`AttemptLimits::lobby_requests_per_ip`, 30 a minute); the precon endpoints count apart (`precon_requests_per_ip`, 60).
- The lobby list's `ETag` is strong: the SHA-256 of the length-prefixed row fields cut to 128 bits, unchanged as games age. `If-None-Match` is read as a list, `W/` ignored, `*` matching any. 503 `lobbies_unavailable` means a server shutting down.
- A seat taken away closes with 4001 (the actor tells it from a replaced or dropped socket by the seat being open, `GameStatus::open`); a game leaving the registry closes every socket, 4002 when its host closed it, else 1001.

### Choosing a deck

The holder of a seat chooses its deck while the game is in its lobby: the host when the game is made, a joiner when joining or after (`seat_deck.rs`, `deck_choice.rs`, `lobbies.rs`).

- A choice is a snapshot (`ChosenDeck`): the engine's cards, commanders and companion, their `DeckRef` and `DeckFingerprint`, and the table's view (`SeatDeck`: name, format, `manual_cards`); later deck edits change nothing. A card marked by hand plays as its printed side (`mtg_cards::assemble_by_hand`; a companion keeps its rules), named in `DeckRef::Oracle.by_hand` so a resume builds it alike; its pinned read is dropped.
- The companion is the one card of the companion zone with a companion ability; a second is refused by the deck check. It is outside the game, so the table hears of it only at the opening's reveal; `manual_cards` counts it. It leaves with the seat or the deck.
- The Attraction deck is the `attractions` zone, each copy with the lights of its line's printing, else the newest paper printing (`printings.attraction_lights`); a card with none stays out. `DeckRef::Oracle.attractions` keeps ids and lights, both in the fingerprint (`DeckFingerprint::of_seat_all`). The `stickers` zone reaches no game.
- A deck id is a demo id, `precon:<code>` (no account needed) or a stored deck's UUID, found under the request's session (a seat token does not replace it); another account's deck, a missing one or no session is 404 `unknown_deck` (400 at creation). Only the creator's seat starts with a stored deck.
- The check is `deck_report` in the deck's own format plus the game format's rules (`format_issues`): Normal wants 60 cards, at most four copies and no command zone; Commander applies `Commander.validate_seat` under the house rules, and a `modern` deck with a command zone keeps `no_commander_zone`. A demo deck in a Commander game warns `commander_count`; a companion is judged by `validate_companion` (`companion_condition`, `color_identity`).
- Every issue is a warning. A table that relaxed a rule lifts the issues it replaces (`IssueCode::lifted_by`), and a deck flagged only for those is judged by `validate_seat` under the table's rules. `ChosenDeck::for_table` counts the table's issues with the deck's own into `SeatDeck.warnings`, seen by every seat, and moves commanders into the library at a Normal table. The issues name cards, so only the owner reads them (`POST /api/decks/check` with `game`).
- The setup skips the engine's deck check (`GameSetup::validate_decks` off). The one refusal is 422 `deck_blocked` (`empty` issue) for a deck with no card the database knows; unknown cards are left out. `manual_cards` counts unverified copies in the main deck and commanders.
- `PUT /api/games/{id}/seats/{n}/deck` is for the seat's holder (another holder 403, a stranger the 404 of `leave`). The actor decides in command order: a ready seat gets 409 `seat_ready` until `NotReady`, a running game 409 `started`; the roster follows each change.
- A seat made without a deck cannot be ready until its holder chooses. A seat given back returns to its original deck or none, and an open seat is listed without a deck.
- When the game starts with decks other than the recipe's, the actor stores the new recipe and `games.manual_play` (`GameRecords::set_start`, only while a `lobby`) before the first event; if that fails the start is refused with `Internal` and the next `Ready` retries. A started game rebuilds from the recipe alone ([Deck check](#deck-check)).
- `GameStatus::manual_play` is whether some held seat plays a deck with `manual_cards` above zero; in the lobby it follows the roster, and `games.manual_play` (`boolean NOT NULL DEFAULT false`) keeps the start's value for resumes. A lobby never reads the column.

## Tournaments

`tournaments.rs` runs an organiser's rounds of Commander games between signed-in players; `tournaments/pairing.rs` pairs and ranks, `tournaments/store.rs` keeps the tables (`tournaments`, `tournament_entrants`, `tournament_players`, `tournament_matches`).

- Every route needs a session: 503 `accounts_unavailable` without a database, then 401 `unauthenticated`. A tournament is seen by its organiser and its players; for anybody else, and for a malformed id, it is 404. The organiser's routes answer a player 403 `forbidden`. Organiser steps and drops run one at a time (`Tournaments::turn`), so a round is paired and its games made once.
- `POST /api/tournaments` takes `{name, format, structure, rounds?}`: a name of 1 to 60 characters, `commander_1v1` or `commander_2v2`, `swiss` or `elimination`, and 1 to 15 Swiss rounds (left out, the start picks the base-2 logarithm of the field, rounded up). It answers 201 `{id, invite}`. The invite is a join-code-shaped code kept in clear: it only signs an account up, and the organiser shows it again.
- `GET /api/tournaments/invite/{code}` shows what an invite signs up for; `POST .../join` signs the account up while the tournament is in `signup`: alone in 1v1, in 2v2 as a new team (`team_name`, else the username) or into the open place of `team`. An account plays for one entrant (409 `already_joined`); a full team or a field of 64 is 409 `full`; a started tournament 409 `started`. `POST /api/tournaments/{id}/leave` takes the player out before the start, and an entrant without players goes with it.
- `POST .../start` needs two complete entrants or more (409 `not_enough_entrants`) and shuffles them with the tournament's stored seed. Swiss pairs each entrant, in standing order, with the highest-ranked one it has not met (an exhaustive search; a rematch only when none avoids it) and gives an odd field's bye to the lowest-ranked entrant without one. Elimination pairs neighbours in bracket order, the next round from the winners in table order; a bye goes to the last entrant without one.
- Each match is a normal Commander game made with `Registry::create_decked`: invite only, no decks (each seat chooses in the lobby), seats alternating between the entrants (A1, B1, A2, B2) and bound to their accounts (`GameOptions::accounts`), so a player opens it with the session alone. A game that cannot be made is logged and the match waits; a game dropped without a result (expired, closed, failed or gone) is forgotten when the tournament is read; `POST .../games` makes the missing ones. A match game has no player host: seat 0 gets 403 for the host routes (close, visibility, teams, removing a player).
- A match's result is read from its game record when the tournament is read: a finished game's winner on an even seat wins for the first entrant, on an odd seat for the second; a draw is a draw. `PUT .../matches/{round}/{table}` sets `{outcome}` (`a`, `b`, `draw`) and marks it `corrected`, which a game's result never overwrites; any round of a running Swiss tournament, only the current round of an elimination one.
- `POST .../next` needs every match of the round decided (409 `undecided`; an elimination draw too), then pairs the next round or, after the planned Swiss rounds or with one entrant left in the bracket, finishes. `POST .../drop` (the player's own entrant) and `POST .../entrants/{n}/drop` (the organiser) stop an entrant's pairings and give an undecided match of the current round to its opponent.
- Standings: 3 match points for a win or a bye, 1 for a draw; ties are broken by the opponents' match-win fraction, the game-win fraction, the opponents' game-win fraction and sign-up order. Fractions are at least 1/3 and a bye is no opponent. A match is one game, so the game-win fraction counts the match record (a bye is one game won).
- 2v2 tournaments sign teams up but answer `start` with 409 `format_unavailable` and `playable: false` until the server makes team games (`TEAM_GAMES`).
- The organiser's account takes its tournaments with it; a player's account leaves its entrant (foreign keys with `ON DELETE CASCADE`). A match's game has no foreign key and outlives a deleted tournament as a normal game.

## Cards and decks

`Server::open_with(config, pool, cards)` takes the `CardSource` (database, rules, `Caps`) shared by games, card endpoints and deck checks. Rules are the embedded files plus the `card_rules` rows of `mtg-cli compile` ([data-sources.md](data-sources.md)): a file wins over a row, an unreadable row is logged and skipped, a changed row rebuilds the library. A request judges under one snapshot. Without a database: 503 `cards_unavailable`, `decks_unavailable`.

### Cards

Public and read-only (`card_api.rs`). `GET /api/cards` searches the deck cards (never tokens or emblems), `GET /api/cards/{oracle_id}` returns one card with its legality, printings, rulings and `reasons`, the clauses the game cannot run. A malformed and an unknown id are one 404; the id is case-insensitive.

- With `q` the order is by relevance (name equal, name starting with, name containing, rules text only), then by name; without it by name. Ties break by oracle id, so pages never overlap or skip. `page` counts from 1 up to 10 000, `per_page` defaults to 40 and is lowered to 100.
- Parameters: `q` (name contains it, accents and case ignored, or rules text in web-search syntax), `colors` with `colors_match` (`includes`, `exactly`, `within`, `overlaps`; letters of WUBRG or `C`), `identity` (the card's identity lies within the letters), `type` (words that must each appear in the type line), `cmc` (`3`, `2-4`, `5+`), `format` (a Scryfall format key; legal or restricted), `page`, `per_page`.
- Blank values are no filter; an unknown parameter or a malformed value is 400 `invalid_request` naming it.
- A hit carries the art of the card's default printing (the newest paper one) and `support`, the state `Library::support` gives with the server's rules and `Caps`.

### Pictures

`images.rs` and `images/` serve a card identity's picture from a disk cache filled from the Scryfall CDN on demand; public, read-only, 503 `cards_unavailable` without a database. Pictures are served as Scryfall serves them, never cropped, re-encoded, resized or marked (Scryfall's image terms); `art_crop` is Scryfall's own crop.

- `GET /api/images/oracle/{oracle_id}/{face}/{size}` is the newest paper printing with a picture (else digital); `/printing/{printing_id}/{face}/{size}` one printing; `/token/{face}/{size}?name=&type=&colors=&power=&toughness=` a token by name: one with every fact, else the same type line and colors, else the most shared facts (power and toughness first), ties to the most printings, then the smallest oracle id, so a URL always gives the same picture and `ETag`.
- `face` counts from 0. A printing with a picture per face answers face `n`'s own; a printing with one picture answers it for each of its faces. Any other face is a missing picture. `size` is `small`, `normal`, `large`, `art_crop` or `png`; another size or a non-numeric face is 400 `invalid_request`.
- A picture answers 200 with `Cache-Control: public, max-age=604800` and a strong `ETag`, a digest of the cache key, so it changes with the stored link. `If-None-Match` answers 304 without reading the cache or asking the CDN. Every other answer keeps `no-store`.
- An unknown identity, a malformed id, a face or size without a picture, a CDN without it and an unknown token name are one typed 404, `no_image` (the web draws a text face). A CDN failure or a non-picture answer is 502 `image_unavailable`.
- The link comes from the database and is still checked: `https`, a host that is `scryfall.io` or under it, no port or credentials, plain path segments ending in `.jpg` or `.png`; a failing link is a missing picture and nothing is requested.
- **Cache.** One file per picture in `MTG_IMAGE_CACHE`, named by the link's path and query, written to a temporary name and renamed. At first use older files count as least recently used and interrupted writes are deleted; after each write the least recently used files go until the total fits `MTG_IMAGE_CACHE_MIB`.
- **Upstream.** One fetch per link at a time, shared by the requests that wait for it. Requests send `User-Agent: mtg-cli/<version>`, follow no redirect, time out after 30 s and are spaced 100 ms server-wide. Timeouts, 408, 425, 429 and 5xx are retried three times (2 s, 4 s, or `Retry-After` up to 20 s); 404 and 410 are a missing picture, other statuses a failure. Only a JPEG or PNG of at most 8 MiB is kept.
- **Warm-up.** A game's start spawns a task that fetches the `normal` picture of every face its seats play (commanders and companions included) in the chosen printing, else the newest paper one, once each, two at a time through the same cache and spacing; the game never waits and failures are only logged.

### Decks

`decks.rs`, `deck_cards.rs`, `deck_import.rs`. Every route needs an account (503 `decks_unavailable`, then 401 `unauthenticated`); another account's deck is 404. Bodies are JSON of at most 64 KiB (640 KiB for an import). `decks` and `deck_cards` belong to `mtg-db` (`DeckRepo`): `owner` has no foreign key (`users` is the server's), and a line's printing is nulled when the printing goes.

- A save is refused with 400 `invalid_deck` (name outside 1 to 100 characters or with control characters, notes over 10 000, a format outside `SUPPORTED_FORMATS`, over 250 lines or 250 cards, a count outside 1 to 999, a bad id or a printing not of the card) or 400 `unknown_cards`. An account stores at most 100 decks (409 `deck_limit`), counted under an owner advisory lock.
- `PUT /api/decks/{id}` replaces name, format, notes and every line in one transaction and runs the check again.
- `GET /api/decks` checks every deck at request time, in one load of their cards on the blocking pool; each card's support, draft state and differing clauses are decided once per library (`Library::read_differs` keeps the read sheet's verdict).
- A `by_hand` line (`deck_cards.by_hand`) makes its card `manual` in the check whatever its rules (one reason, `OWNER_BY_HAND`, no draft to read, its own `coverage_hash` via `deck_report_by_hand`); any marked line marks the card, and a card the engine cannot host stays `unplayable`.
- Saving stores `{deck_hash, coverage_hash, checked_at, summary}` from `mtg_cards::deck::deck_report` in `decks.last_check`. The state is never stored: every read runs the check of the moment ([deck-check.md](deck-check.md)) and `verified` follows it, so a release shows its effect without a save. Reads write nothing.
- `POST /api/decks/check` takes `{deck}` (own stored deck or `precon:<code>`) or `{format, cards}` (an unsaved list, under the rules of a save), and optionally `game` or `table` (`{format, house_rules?}`), not both; it saves nothing. With a table, legality becomes the verdict a seat choosing the deck there gets (`judge_at_table`). An unknown `game` is 404 `not_found`; both, or a format other than `normal` or `commander`, 400 `invalid_request`; a rule out of range 400 with `field`.
- Import (`POST /api/decks/import`, nothing saved) reads a pasted list in any `mtg-import` format (`source` detected when absent; `format: commander` takes Moxfield's first line, and a pairable second, as commanders), at most 100 000 characters and 1 000 lines, and answers `cards`, `unresolved` (with suggestions), `problems` and `ignored`, resolving names through the card index ([data-sources.md](data-sources.md), Name resolution).
- Export (`GET /api/decks/{id}/export?format=text|arena|mtgo`) writes split cards as the format does and other multi-faced cards by their front face, with printings where the format has them. A deck the format cannot hold (MTGO text with a commander and nothing else) answers 422 `export_failed`.

### Precons

`precon_api.rs`, `precons.rs`. Public and read-only: 503 `cards_unavailable` without a database; every request counts per address (`AttemptLimits::precon_requests_per_ip`; 429 `rate_limited` with `Retry-After`). `GET /api/precons` lists every precon, newest first; `GET /api/precons/{code}` returns one with issues and cards by board. Codes are case-insensitive; an unknown or malformed one is 404 `not_found`.

- A code is the set code and the slug of the name (`c18-adaptive-enchantment`: lower-case ASCII words joined by hyphens, cut at 80 characters). A set holds several precons, so the set alone names none; a code another deck already has ends in the deck's id.
- `Judge::judge` judges a precon for list, view and seat alike: `deck_report` as `commander` with a commander, else `freeform`; an entry matching no stored card is an `unknown_card` warning, `unplayable` and left out. `state` is the verdict of the moment (`verified`, `read`, `manual_play`). `issues` are the report's legality, else the engine's check in the deck's format. `playable` is false only when the database knows none of its cards.
- All precon judgments live in `CardSource::precon_index`, rebuilt once per change of `PreconRepo::version`, `CardRepo::index_version`, `CardRulesRepo::version` or `DraftRepo::version` in its own task, judging on the blocking pool. The first build is awaited; later requests wait at most `FRESH_WAIT` (500 ms), else get the previous index. The single-precon view judges again per request.
- A seat plays a precon by `deck: "precon:<code>"` (create, join or `PUT .../deck`) like a stored deck: main board as library, commander board as commanders. Any table takes it with its warnings (`no_commander_zone` at a Normal table, its commanders in the library; `commander_count` at a Commander table). Refusals: 422 `deck_blocked` when not `playable`, 404 `unknown_deck` (400 at creation). One-click play is a create with it as seat 0's `deck`.
- `POST /api/decks/check` with a precon needs no session but counts against the address's precon budget; with `game` or `table` it equals a seat's verdict (`judge_cards_at_table`). `format` or `cards` beside it is 400 `invalid_request`. Any other body without a session is 401, a malformed one too.

### Favourite precons

`favourites.rs`: a favourite points to a precon and follows its cards and check. Needs an account (401 `unauthenticated`; 503 `cards_unavailable`). `PUT` and `DELETE /api/favourites/precons/{code}` are idempotent (204); unknown or malformed codes are 404 `not_found`; writes count per account (`AttemptLimits::favourite_writes_per_user`, 429 `rate_limited`). `PreconListItem.pictures` and `DeckListItem.pictures` are a deck's commanders, else its first card.

### Account preferences

`prefs.rs` and `PrefsRepo` (`user_prefs`, deleted with the account) keep the default stops, "End my turn by hand", the always-yield list (`auto_yields`, at most `MAX_AUTO_YIELDS`), the auto choices and the hand sort (`hand_sort`, null for the default) across browsers. Unsaved stops default to the own declare attackers and postcombat main (`default_stops`); a saved empty list stays empty. A sort naming a group twice is refused; missing groups are appended in starting order. `PUT /api/me/prefs` takes at most `MAX_DEFAULT_STOPS` lasting stops at priority steps, one per side and step, else 400 `invalid_request`. 401 `unauthenticated`; 503 `cards_unavailable`; bodies at most 8 KiB.

### Fix marks

`fix_marks.rs` and `mtg-db`'s `FixMarkRepo` (`card_fix_marks`). A player marks a card the game does not run whole, or any card that misbehaves, as important to fix; the owner reads the marks ranked. Every route needs a signed-in account: 503 `cards_unavailable` without a database, then 401 `unauthenticated`. Bodies are at most 4 KiB.

- One row per account and card (`PRIMARY KEY (user_id, oracle_id)`); a second `PUT` replaces the note and keeps the time. An account holds at most `MAX_FIX_MARKS_PER_USER` (500): a new card beyond it is 409 `fix_mark_limit` (a note on a marked card is always taken), checked under an advisory lock of the account (`FixMarkRepo::mark_within`); a `wrong` read past the cap keeps its verdict without a mark. Marks and unmarks count per account (`AttemptLimits::fix_mark_writes_per_user`, 200 per `window`, 429 `rate_limited`). `user_id` has no foreign key (`Accounts::delete_user` deletes the marks); a card deleted from `cards` takes them. An unknown card or a non-uuid id is 404 `not_found`.
- A note is trimmed; an empty one is none. It holds at most 280 characters (`MAX_FIX_NOTE_CHARS`; the table checks the same limit) and no control character but a line break, else 400 `invalid_request`.
- No endpoint shows another account's mark. `FixMarkRepo::ranked(limit)` counts accounts per card, most marked first, ties by the newest mark and then name; `mtg-cli fix-marks` adds the support state and prints it ([data-sources.md](data-sources.md)), and `check-rules --gaps --marks` weighs gaps by the marks of the cards they free.

### Reads

A draft is a stored reading of a card's Oracle text that runs when it is trusted ([cards-dsl.md](cards-dsl.md)). `mtg-db`'s `DraftRepo` (`card_drafts`) and `ReadRepo` (`card_reads`) hold drafts and verdicts; `reads.rs` serves them to a reader and `mtg-cli reads` prints them.

- `card_drafts (oracle_id, read_hash, rules jsonb, retired, ...)`, primary key `(oracle_id, read_hash)`, is written only by `mtg-cli compile` from the embedded drafts; `DraftRepo::insert` makes a draft current and retires the card's other, and `DraftRepo::get` also reads retired (pinned) drafts. `source`, `oracle_matches`, `proof` and `differs` come from `drafts/meta/`.
- `card_reads (id, user_id, oracle_id, read_hash, verdict, note, created_at)` is append-only (the database refuses an update; only `ReadRepo::delete_all_of` removes rows): an account's newest row on a hash is its verdict, `user_id` is null for the server's own verdict, which is always `wrong`, and a `wrong` always has a note (the fix-mark note rules).
- A trusted Forge draft (source `forge`, `oracle_matches`, not `differs`) is approved until a `wrong` counts and runs for everyone as `forge`; a `differs` draft is pending until a `correct` counts.
- `GET /api/reads?deck=<id>` (own deck) or `?precon=<code>` lists what the account has left to read there (`ReadQueue`: pending drafts, then approved ones it has not judged); with neither, every card whose current draft awaits a first read (cached in `PendingReads` until rules, drafts, verdicts or cards change), and with `?mine=true` those in its decks. Other parameter mixes or empty values are 400. Most played first (`card_ranks`). `GET /api/reads/{oracle_id}` gives the current draft (`ReadCard`); no deciding draft, another account's deck or an unknown code is 404.
- `PUT /api/reads/{oracle_id}` (`ReadBody`) appends the account's verdict on the shown hash (409 `stale_read` if not current); a `wrong` needs a note (fix-mark rules) and marks the card to fix. Read routes need an account (503 `cards_unavailable`, then 401) and count against `AttemptLimits::read_requests_per_user` (429 `rate_limited`). `CardDetail.draft` carries a deciding draft's hash and state.
- Bulk reads (`bulk_reads.rs`): `GET /api/reads/bulk` lists the pending `differs` drafts the account has not judged whose every differing clause matches under `mtg_cards::wording` ([cards-dsl.md](cards-dsl.md), Bulk reads), kept per `read_hash`. `PUT /api/reads/bulk` (1 to `MAX_BULK_APPROVALS` cards) appends one `correct` per card still a candidate on the named hash and answers the `approved` and `stale` ids. Same session, budget and 503 rules.
- `CardSource` rebuilds the library when `DraftRepo::version` (a digest of drafts and verdicts) changes, so a verdict reaches card endpoints, deck checks and the precon index at once and a game at its start. A rebuilt `Library` inherits the decisions of every card whose draft did not change (`Library::inherit_decisions`). Deck checks and judging run on the blocking pool.
- `PUT /api/fix-marks/{oracle_id}` on a card that plays on a draft (`read`, `forge`, `unverified` or proved `partial`) also appends the account's `wrong` verdict (the note, else "marked to fix"), so the card plays by hand from the next game. An `AbilityLeftToTable` event ([engine.md](engine.md), Fail-soft resolution) appends the server's `wrong` on the pinned hash with the error as note.
- **Pinning.** The started recipe stores `reads`, the `read_hash` of each card played on a draft, and resume and replay build those rows, so a verdict never changes a running game, only its label (Unverified for a whole draft not approved now: `CardDefinition.unverified`, outside the fingerprint). A resume fails when a pinned row is gone or a card's text or compiled rules changed.
- **Conjured cards.** `CardSource::conjured_in` resolves each name a game's decks conjure through the card index and assembles it like a deck card with its reads pinned; `GameRecipe.conjured` keeps their fingerprints by name and a resume fails with `ConjuredChanged` when one differs. A name that does not resolve to one card conjures nothing.

## Pace of play

At each priority decision the actor asks the seat, passes for it at once or passes for it after a short wait. Every pass takes the path of the seat's own answer (`Running::answer`), so it is checked, logged, saved and projected like a click.

### Windows

`windows::verdict` returns `Ask`, `Pass` or `Dwell` from the step, the active player, the stack, the seat's stops, mode and yield, and `can_act`; auto-pass off is the `Held` mode. A seat passed for is never sent the decision; a non-priority decision is asked unless the engine gives its one answer (below). The first matching rule wins:

| # | Condition | Verdict |
|---|---|---|
| 1 | Full Control (`NextWindow`, `Held`) | Ask at every window |
| 2 | A running yield (a stop of the seat ends it, except in the step it was asked at) | Pass at once |
| 3 | A stop of the seat for this side and step, or a by-hand window | Ask, whether it can act or not |
| 4 | A default window and the seat can act | Ask |
| 5 | A default window and the seat cannot act | `Dwell`; pass at once in its own main phase with an empty stack |
| 6 | Smart stops on, no yield, and a permanent of the seat has an ability its timing lets it use (`smart_stops::can_answer_now`, costs left out) | Ask when such an ability can be paid now, `Dwell` otherwise |
| 7 | Anything else | Pass at once, whatever the seat holds |

The default windows are both main phases on the seat's own turn, declare attackers and blockers on any turn, and any step with another seat's object on the stack. `Own` stops apply on the seat's turns, `Opponent` stops on others'. Rule 6 (`windows::smart`, `AutoChoices.smart_stops`, off by default) reads the battlefield, never a cost or the hand, so a spell in hand never gets the seat asked and a discard cost only turns Ask into `Dwell`.

A question with one legal answer whose options every seat sees (`mtg_engine::choose::sole_answer`) is answered for an `Off` seat in the same run of passes, and a payment the seat cannot make (`YesNoOptions::only_no`) is declined; Full Control asks both. A question whose one answer could come from a hidden zone (`YesNoOptions::unseen`: a discard, search, reveal or exile from hand) is always asked.

### Stops, Full Control and yields

- Stops and modes live in the actor per seat (`Stops`), never stored: they survive reconnects, and a resumed game starts every seat in the default mode without stops. Setting one takes no engine step, so the pending decision keeps its id.
- A one-turn stop goes once the game has left its step: a step entered and left in one go counts, one never entered does not. `NextWindow` returns to `Off` when the seat passes, itself or at its clock or dwell.
- A by-hand window: once a spell or ability resolves whose card has clauses owed at resolution (`ManualClause::owed_at_resolution`), the game leaves an ability to the table (`AbilityLeftToTable`) or a by-hand line comes due (`ByHandDue`), its seat holds a stop at every window until it passes, read from public facts only; `ObjectView.manual_owed` flags those objects by the same rule ([web.md](web.md), By hand).
- Turning Full Control on ends a running yield and `Yield` is refused while it is on; turning it off at a priority decision the windows then pass for passes at once.
- `AutoChoices.skip_triggers` passes a seat at a priority decision with a triggered ability on top of the stack (`auto_yields::passes_for`, beside the per-ability yields), from the stack alone. Setting the choices at such a decision passes at once (`take_stops_passing`), as a new yield does.
- `AutoSkip` and `CancelAutoSkip` set and take back a seat's rules for its own triggers (`auto_skips.rs`), held in `Stops` and never stored. `Running::auto_answer` applies them after the engine's sole answer and before `AutoChoices`, outside Full Control, from the next question on.
- `Yield` and `CancelYield` set and end a per-seat yield (`Yields`), never stored. Its end rules ([protocol.md](protocol.md), Yields) live in `Active::ends_at` and read the table alone, never `can_act`. If a started yield ends at once the decision is issued again with a new id, so a client that dropped its prompt always gets one.

### Passes and saves

- An answer, its passes and the save are one unit (`publish_with`): copies of yields and stops, `Running::skip_idle` (the passes, appended to the step), one projection, one append, one batch per seat. The copies replace the seats' own only once saved, so a failed save restores engine, yields, stops and modes.
- A run of passes stops at the first decision the windows ask or that waits for a dwell, at any other decision, at the game's end and after `MAX_IDLE_PASSES` (500), where the decision is sent as it is and its seat's yield ends with a notice. `start_up` settles a resumed game before any client connects.

### Take-backs

- `take_backs.rs`: `TakeBack` from a seat asks the engine (`take_back_offer`); a refusal is `TakeBackRefused {reason}` to that seat, a second request while one is open `Error{BadMessage}`. Otherwise every other seat still in the game is asked (`TakeBackAsked`, to every viewer) and has `TAKE_BACK_WAIT` (15 s) to answer `TakeBackVote`; a decline or silence ends it (`TakeBackEnded` `Declined`, `TimedOut`). Bots agree at once (`bots.rs`).
- The game goes on meanwhile; a step that changes the seat's offer ends the request as `Withdrawn`. Once every seat agreed, `Running::take_back` is a step like an answer: its passes follow, then `TakeBackEnded` `Approved`.
- After every step each connected seat is sent `TakeBackOffer` when what it may take back changed, and on join or resync with the open request.
- Tournament games are made with `take_backs: false` (`tournament_rules`), so the engine refuses them as `Off`.
- A step that cuts the log (`Engine::cut_since`: a take-back, or a cast backed out of with its mana) is saved with `EventStore::rewind` (delete from the cut and append, one transaction under the game's lock) and announced as a `Snapshot` marked `rewound` to every connected seat and spectator, with the pending decision and `Deciding`. A failed save restores the engine and its cut events.

### Manual actions

- `ClientMessage::Manual {action}` is accepted at any time from a seat in the game (`running/manual.rs`); the actor handles one message at a time, so it lands between engine decisions. A seat that lost or a finished game gets `Error{BadMessage}`, a refused action `Error{IllegalAnswer}` with the engine's reason.
- The step is `Engine::manual`, published like a concession (one projection, append and batch for every seat, then the windows' passes). While a replacement question holds a batch the action waits in `Running.deferred` and applies in order once a later command or clock expiry leaves the engine settled.
- A decision the change leaves equal keeps its id and clock and is not resent; one whose options changed is resent under a new id. A decision of the same seat and kind keeps its clock (`carried_clock`), so a manual action never starts, resets or extends a response clock and the acting seat learns nothing of another's decision.

### Response clocks

- A decision gets its clock when issued (`Running::settle`, `Pace::clock_for`) from its kind and seat alone, never the options, hand, mode or stops, so a `Priority` decision is clocked whether the seat can act or not. The match names every kind, so a new kind must choose its clock and answer.
- Four lengths, `0` for none: response (`MTG_RESPONSE_SECS`), blocks (`MTG_BLOCK_SECS`), think (`MTG_THINK_SECS`) for what takes thought, active (`MTG_ACTIVE_SECS`) for every decision of the active player's turn; a non-active seat is clocked by class (`Class::of`). The opening (mulligans, bottoming, its scry) has no clock (`Pace::OPENING_CLOCK`) and waits for each seat until it answers or concedes.
- The countdown shows after `MTG_CLOCK_GRACE_SECS`; the clock runs from issue and `Decision.show_after_ms` ([protocol.md](protocol.md), Decisions) is absent when the countdown shows at once, also for a clock no longer than the grace. A reconnect is told what is left of the delay.
- **Timer bank** (`bank.rs`): three own turns in a row without a reached deadline earn an extension, at most three (`BANK_TURNS`, `BANK_MAX`); a reached deadline restarts the count. At a sent clock's deadline the seat spends one (`Running::extend`): the deadline moves by `MTG_EXTENSION_SECS` (`0` disables the bank), the decision is resent with its id and the new `deadline_ms`, and every seat gets `Bank`; only an empty bank lets `fall_back` answer. In memory; public in `PlayerView.bank`.
- **Turn time** (`turn_time.rs`): each seat's wall time on its own turns, chess-clock style; a turn ends at the next turn, the game's end or its player leaving, the opening is no turn and time with nobody connected does not count. The actor stamps `PlayerView.turn_time_ms` and `turn_time_running` at send time ([protocol.md](protocol.md), Decisions). In memory; a resume starts from zero.
- The clock ignores the mode, runs while the seat is away, stops with any answer and restarts on resume; a reconnect is told what is left and the deadline does not move.
- At the deadline `Running::fall_back` answers through `Running::answer` (a late answer is `StaleDecision`) and logs at info `clock ran out; the table answered for the seat` (or `clock ran out; an extension was spent`) with game, seat and kind, never a card. A waiting command goes first; a failed save retries a second later; a decision with no answer to give stops its clock and waits.
- The fallback decides least and shows nothing, from kind and options alone (`pace.rs` tables every kind): a pass; the first cards, option or mode offered; the lowest number; `Cancel` where a cast is free to take back; only the required attackers and blockers; lethal damage in order; no target (`Cancel` for a spell, `Remove` for a trigger, which a seat cannot send: `IllegalAnswer`); no optional cost; an even division; "No".

### Dwell

- A seat that cannot act at a default window (declare attackers or blockers, another seat's object on the stack, its own main phase with a non-empty stack) and holds no stop, Full Control or yield is neither asked nor passed at once, since an instant pass shows it holds nothing.
- The decision is held back (`Running::settle`): never sent, a join or `Resync` gets the snapshot alone, an answer for it is stale. At its `due` time `expire` passes through `fall_back` in a step, append and batch of its own, so other seats see a seat that could act and passed.
- The wait is uniform in 250 to 700 ms (`MTG_DWELL_MS`), drawn per idle seat from an OS-seeded generator independent of game seed and state. It is wall clock only: log, event order and state hash equal those of a game without dwell. `Dwell::OFF` (also `Pace::MANUAL`) passes at once.
- A stop at the window, Full Control or auto-pass off sends the decision at once; a yield passes at once; a concession removing the seat ends the wait. Chat, `Resync`, stale answers, stops elsewhere and other seats' commands leave it.
- It hides whether a seat could act in a combat window or with a spell on the stack. It does not hide its own main phase with an empty stack, how long a seat that can act takes, a pass at a clock deadline, or a choice of stops, Full Control or yield.

## Previews

The actor answers `Preview` itself (`preview.rs`) from `Engine::preview_forecast`, null rather than an `Error` when there is none ([protocol.md](protocol.md), Preview). A per-seat bucket (`PREVIEW_BURST` 20, then `PREVIEWS_PER_SECOND` 10) lasts the game; over it the forecast is null without touching the engine. Answers have no bucket, so a flood cannot starve them.

## Configuration

Blank values count as unset; any other malformed value refuses to start (`ConfigError`). `ServerConfig` holds more values (queues, socket limits, retention, hash cost) that only tests set.

| Variable | Default | Meaning |
|---|---|---|
| `MTG_BIND` | `127.0.0.1:8080` | Listen address, `ip:port`. |
| `MTG_ALLOWED_ORIGINS` | none | Comma list of `http(s)://host[:port]` origins besides the server's own host (sockets, state-changing requests, CORS). |
| `MTG_MAX_GAMES` | 100 | Most games alive at once, at least 1. |
| `MTG_AUTO_PASS` | on | `1`, `on`, `true` or `0`, `off`, `false`. Off starts every seat in `Held`. |
| `MTG_RESPONSE_SECS` | 10 | Response clock; `0` for none, at most 86 400. |
| `MTG_BLOCK_SECS` | 45 | Clock for declaring blockers; same range. |
| `MTG_THINK_SECS` | 60 | Clock for a seat that is not the active player when paying, dividing combat damage, declaring attackers and discarding; same range. |
| `MTG_ACTIVE_SECS` | 0 | Clock on every decision of the active player's turn after the opening; `0` for none; same range. |
| `MTG_EXTENSION_SECS` | 20 | Seconds an extension of the timer bank adds; `0` turns the bank off; same range. |
| `MTG_CLOCK_GRACE_SECS` | 3 | Seconds a clock runs before its countdown shows; `0` shows it at once; same range. |
| `MTG_DWELL_MS` | `250-700` | Dwell: a range, one number for a fixed wait, `0` for none; at most 5000. |
| `MTG_REGISTRATION` | `open` | `closed` refuses self-registration; existing accounts keep working and the admin still makes accounts. |
| `MTG_ADMIN_USERNAMES` | unset | The admins' usernames, comma-separated; blank entries are skipped. Unset: nobody is admin ([Admin](#admin)). |
| `MTG_ADMIN_USERNAME` | unset | One more admin username. |
| `MTG_COOKIE_SECURE` | 0 | `1` marks the session cookie `Secure`; set it behind TLS. |
| `MTG_TRUSTED_PROXIES` | 0 | Reverse proxies in front of the server, 0 to 16. |
| `MTG_SEED` | random | A whole number (`u64`) every game shuffles with, so a game replays exactly; each game logs its seed (`game created`). For tests and replays, not production. |
| `DATABASE_URL` | unset | Postgres URL, never logged. Set, the binary connects, migrates and calls `Server::open`; unset or blank, it warns and keeps games in memory without accounts. `.cargo/config.toml` defaults it to the local database. |
| `MTG_IMAGE_CACHE` | `$XDG_CACHE_HOME/mtg-server/images`, else `~/.cache/mtg-server/images` | Directory of the picture cache. |
| `MTG_IMAGE_CACHE_MIB` | 2048 | Size limit of the picture cache in MiB, 1 to 1048576. |
| `RUST_LOG` | `info` | Log filter. |

## Testing

- Actor tests drive games through `GameHandle` with bot and manual clients (`tests/it/common`) and compare log, state hash and result with a bare engine (`mtg-testkit`); `tests/it/windows.rs` compares the actor with `mtg-testkit`'s plain `Windows` rule for 2 to 4 seats.
- Clocks, the dwell and the leak tests run on tokio's paused time. `many_seat_pace.rs` runs a response clock, the timer bank and hidden hands at 13, 32 and 64 seats; `clocked_manual.rs` holds a manual action to its clock (`carried_clock`) and behind a replacement question (`flush_deferred`). The leak tests (`pace_leak.rs`, `pay_leak.rs`) play two games differing in one fact and require every other seat to get identical messages and batches. `pace.rs` tables the clock and fallback of every kind, seat, setting and options shape.
- Transport tests (`tests/it/net`) use a real listener on port 0, raw TCP for hostile HTTP and tokio-tungstenite for sockets; `FlakyStore` fails writes on demand.
- Each HTTP area has a fake-registry test and a Postgres one (`pg_*.rs`), with scans for tokens, codes and card names that must not appear. Postgres tests use the throwaway database of `scripts/check.sh` ([testing.md](testing.md)) and `#[sqlx::test(migrations = false)]` with `migrate`. Pictures use a fake CDN; no test asks Scryfall. The ignored `pg_commander_real.rs` needs a compiled database in `MTG_REAL_DATABASE_URL`.
