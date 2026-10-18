# Web notes (M2f core, M2g UI)

The client layer in `web/src/net` and `web/src/play`, and the first screens in `web/src/features` (see UI at the end). Wire contract: `protocol.md`; transport: `server.md`.

## Modules
- `net/api.ts`: `api.createGame/getGame/listDecks` over `fetch`; bodies are checked at runtime and a failure is an `ApiError {code, status, message}`. Codes: those of `crates/mtg-server/src/api.rs` (`invalid_request`, `unsupported_format`, `unknown_deck`, `forbidden_origin`, `not_found`, `method_not_allowed`, `payload_too_large`, `unsupported_media_type`, `internal`, `server_full`, `unavailable`) plus local `unknown`, `network`, `bad_response`.
- `net/parse.ts`: `parseFrame(data)` is the only door from the wire (JSON, known `type`, required fields, 1 MiB cap). Garbage is a `FrameError`, never a throw.
- `net/socket.ts`: `GameSocket` sends `Join` first and once per connection, retries, and reports `idle | connecting | open | reconnecting | replaced | stopped`. `createSocket`, `timers`, `random`, `origin` are injectable. `net/pacer.ts` holds sends to 30 at once, then 15 a second (the server allows 40 and 20 and closes a socket over it with 1008), so a bot or auto-pass screen is slowed, not dropped.
- `play/view.ts`: pure `applyBatch`, `fromFirstBatch`, `fromSnapshot` (ports of `StateView::apply`); unchanged objects, players and zones keep their identity.
- `play/session.ts`, `play/store.ts`: `createSession({game, token, name})` gives `{store, connect, disconnect, ready, answer, concede, chat, resync, dismissNotice}`. Make one per game page (not per render). In React: `useStore(session.store, selector)`, `connect()` in an effect, `disconnect()` in its cleanup.

## State flow
Socket event -> session (the client rule) -> `store.setState`. A Snapshot sets the base and `seq`. A batch with any other `from_seq` (or any batch but the one at 0 while there is no view) sends one Resync; nothing is applied and decisions are ignored until the next Snapshot. `Decision` replaces `pending`; `answer()` sends once per id and clears it. A Snapshot or a lost connection forgets answered ids, so the server's repeat of a decision can be answered again. `GameOver` sets `result` and makes a later close final.

## Reconnect
| Close or message | Result |
| 1001, 1006, 1013 and any code not listed below | retry after 250 ms doubling to 15 s, less up to 50 % jitter; a Welcome resets it |
| 4000 | `replaced`, no retry; `connect()` takes the seat back |
| 1000, 1002, 1007, 1008, 1009; `Error` UnsupportedVersion or UnknownGame | `stopped`, reason in `notice` |
| any close after `GameOver` | `stopped`, no notice |

StaleDecision and IllegalAnswer give a notice and a Resync; BadMessage and Internal only a notice. A wrong token is a refused upgrade, which a browser reports as 1006, so the client retries forever (`attempt` counts).

## Tests
`scripts/check.sh web`, or `pnpm test` in `web/`. Fakes in `web/src/test-utils`: `FakeNetwork`, `FakeTimers`, `startSession()`, the Rust-generated `transcripts`. `vite` proxies `/api` and `/ws` to 127.0.0.1:8080 and keeps `Host`, which the server's Origin check compares with the page's `Origin`.

## UI (M2g part 1a)
Routes are file-based in `web/src/routes` and only pick a page; `routeTree.gen.ts` is rewritten by the Vite plugin on `pnpm dev` or `pnpm build`, so commit it with the route change. Pages are in `web/src/features/{create,game,table}`, shared parts in `components/` (`ui/` is shadcn: `pnpm exec shadcn add <name>`), pure helpers in `lib/`. The M0 spike (simulator, store, `e2e/spike.spec.ts`) is gone; `spike-react.md` keeps its numbers, and `pnpm e2e` has no specs until the table task adds some.

- `/` is `create/CreateGamePage`: decks from `api.listDecks()`, 2 to 12 seats (name and deck each, a preview of the deck), format fixed to Normal with Commander shown disabled. `create/form.ts` repeats the server's checks, `create/apiErrors.ts` puts every `ApiError` code in plain words. On success it lists one link per seat with copy buttons, copy all and "Open my seat" (seat 1).
- `/game/$gameId#seat=<n>&token=<token>` is `game/GameRoute`, which reads the fragment from the router's raw `location.href` (its `hash` is percent-decoded) and renders `game/GamePage`.
- Seat links (`lib/seatLink.ts`): the token lives only in the fragment, which a browser never sends, and is printed only inside a `data-share-link` anchor (`components/SeatLinkRow`); tests scan the page for tokens outside those (`test-utils/leaks.ts`). `lib/invites.ts` keeps the host's links in `sessionStorage` (every access in try/catch, the app works without it) so the lobby can hand them out after the create page is gone, and it supplies the `Join` name.

| Store | Screen (`game/GamePage`, `connectionOf` in `game/connection.ts`) |
|---|---|
| no readable seat and token in the address | `SeatLinkMissing`; no session, no socket |
| `phase` over | `GameOver`: winner, draw or concession (from `view.players[].lost`), each seat's finish, link to `/`; no connection notice afterwards |
| `status` replaced or stopped | `ConnectionNotice` alone: the reason from `notice` and one button to connect again |
| idle or connecting, never welcomed | "Connecting to the game…" |
| `reconnecting`, `attempt` below 5 | "Connection lost. Reconnecting… (attempt N)" |
| `reconnecting`, `attempt` 5 or more | "Cannot reach the game", "Try again now" (`disconnect()` then `connect()`, counting from 1); the socket keeps retrying meanwhile and a Welcome moves the page on |
| welcomed, `phase` lobby / playing | `Lobby` / `GameTable` |

After a Welcome the lobby or table stays under the reconnect notice. A `notice` shows above any screen as a dismissible alert, except on `GameOver` and where a connection notice already prints it.

`GamePage` makes one session per (game, seat, token), connects one microtask after mount and disconnects on unmount, so StrictMode's mount-unmount-mount opens one socket. `game/gameContext` gives every screen `useGame()` (`game`, `session`, `lookup`) and `usePlay(selector)`; `lookup` is REST (`GET /api/games/{id}`): seat names and who is connected, fetched once and polled every 3 s in the lobby.

The table is `features/table/GameTable.tsx`, rendered with no props while `phase` is `playing` (see Table below). `GamePage` puts notices and the reconnect banner in a narrow column above it, so the table uses the full width.

Lobby gaps (no Rust changed): no message says who has pressed Ready, so the lobby shows only this seat's own Ready (kept in the page, lost on reload, where a repeat Ready is harmless), and the protocol cannot take Ready back. "Here" comes from REST `connected`, hence the polling.

Tests: `test-utils/gameHarness.tsx` `renderGame()` mounts the game page on fake sockets, a manual clock and an in-memory REST server (`fakeServer.ts`); `router.tsx` gives pages a real router on a memory history; `@testing-library/user-event` drives the keyboard; `src/routing.test.tsx` runs the real route tree. Screenshots: `node scripts/ui-shot.mjs <url> <out-prefix> [--widths 1400,390] [--wait <js condition>]` drives headless Firefox over WebDriver BiDi, waits for the page to render (Firefox's own `--screenshot` fires at `load` and gives a blank SPA) and reports horizontal overflow.

## Table (M2g part 1b)
`features/table/` is the read-only table; decision prompts are the next task. `GameTable` takes no props: it reads with `usePlay(selector)`, chats through `useGame().session.chat`, and starts from `TableOptionsContext` (`mode`, `focus`; only the dev route sets it).

- Layout, `layout.ts` (pure; tested for 2 to 12 seats and every viewer): `arrangeSeats` lists the opponents in turn order after the viewer, who is drawn last, at the bottom. Grid mode (default up to 4 seats): every opponent's panel and battlefield in `min(3, opponents)` columns (fewer on narrow screens). Focus mode (default from 5): a tile per opponent and one opponent large; the default focus is the active player, else the priority holder, else the next seat still in, and follows the turn until a tile is chosen (click, Enter or Space; "Follow the turn" undoes it). The toolbar switches either mode in any game. From 8 seats cards are compact; lands always are.
- Parts: `SeatPanel` (name, life, mana pool, mulligan line, eliminated reason, library and hand counts, hand strip or card backs, graveyard top card and list dialog, exile of that owner, command zone, a commander damage and tax slot that says "not tracked yet"), `Battlefield` (rows of lands, other permanents, creatures, creatures toward the middle), `Card` and `CardFace` (a text face; a `data-slot="card-art"` box waits for an image id in `ObjectView`; tapped is a quarter turn plus the word "Tapped", and sick, damage, attacking, blocked and blocking are words too), `CentreStrip` (turn banner, priority, read-only decision line), `StackPanel` (top first, with controller), `Drawer` (log sentences from `eventText.ts`, chat; a side column from 1024 px, bottom tabs below), `CardPreview` (hover or focus shows the large card with its abilities, docked away from it; a click pins it, Escape or Close dismisses).
- Rendering by construction: a `Card` reads only its own object and combat role and is `memo`; a row reads its ids through `useShallow(battlefieldRow)`, and `selectors.ts` splits the shared battlefield by controller once per view. A batch that taps one permanent redraws that card and no row; `perf.test.tsx` counts the renders at 12 seats with 40 permanents each, and fails without `memo` or `useShallow`. Each row of cards is one tab stop (roving focus: arrows, Home, End).
- Seams for the prompts: `Card` takes optional `highlight` (`legal | dimmed | selected`) and `onSelect(id)`; cards carry `data-object-id` and seat panels `data-seat`; `[data-slot="decision-line"]` in `CentreStrip` is where the prompt goes. Not in the views yet, so not drawn: card art, counters, attachments, commander damage and tax.
- Dev route: with `pnpm dev`, `/dev/table?seats=2|4|12&mode=grid|focus&focus=<seat>` (2 to 12 seats) shows the real table over `features/table/dev/fixture.ts`: about 40 permanents per seat, an attack with blockers, 3 stack items, graveyards, a commander each, 50 log events. The route exists only with `import.meta.env.DEV`; `devBuild.test.ts` builds the app and fails if the fixture's marker or card names reach the production bundle (a DEV build must contain them, so the check can fail). By hand: `pnpm build && ! grep -r dev-table-fixture dist`.
- Screenshots: `node scripts/ui-shot.mjs "http://127.0.0.1:5173/dev/table?seats=4" out --widths 1400 --full --wait "document.querySelector('[data-seat]')"`. The page loads at the first width, so shoot one width per run; the script prints OVERFLOWS when the page scrolls sideways.
- Tests: `test-utils/tableHarness.tsx` (`renderTable` over a fixture game, `renderLiveTable` over a real session) and `test-utils/cards.ts`; `transcript.test.tsx` replays the Rust transcripts through the real store into the table.
