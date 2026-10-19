# Web client

The browser client: game creation, lobby, table, decision prompts and accounts, on the wire contract of [protocol.md](protocol.md) and the transport of [server.md](server.md).

## Stack and layout

React, Vite, Tailwind, shadcn/ui on Base UI, TanStack Router and zustand. Commands are in `CLAUDE.md`.

- Pages live in `src/features` (`auth`, `create`, `game`, `games`, `table`); `src/routes` only picks them. The Vite plugin rewrites `routeTree.gen.ts` on `pnpm dev` and `pnpm build`; commit it with the route change.
- `src/net` (REST, socket) and `src/play` (session, store) hold the client. `src/components/ui` is shadcn (`pnpm exec shadcn add <name>`). `src/protocol` has the ts-rs bindings and the transcripts Rust writes.
- The dev and preview servers proxy `/api` and `/ws` to `MTG_SERVER_URL` (default `http://127.0.0.1:8080`) and keep the browser's `Host`, which the server's Origin check compares with the page's `Origin`.

## Data flow

- `net/api.ts` checks REST bodies at runtime; a failure is an `ApiError` with a code of `crates/mtg-server/src/api.rs` or a local `unknown`, `network` or `bad_response`. `net/parse.ts`: `parseFrame` is the only door from the wire (JSON, known `type`, required fields, 1 MiB cap); garbage is a `FrameError`, never a throw.
- `net/socket.ts`: `GameSocket` sends `Join` first and once per connection. `net/pacer.ts` holds sends to a burst of 30, then 15 a second; the server allows 40 and 20 and closes a socket over it with 1008.
- `play/view.ts`: pure ports of `StateView::apply`; unchanged objects, players and zones keep their identity.
- `play/session.ts`: `createSession({game, token, name})` owns the socket, applies the client rule of [protocol.md](protocol.md) and writes the zustand store (`play/store.ts`) that screens read with `useStore(session.store, selector)`.

The session's rules:

- A Snapshot sets the base and `seq`. A batch whose `from_seq` is not `seq` (or any batch but the one at 0 while there is no view) sends one Resync; nothing is applied and decisions are ignored until the next Snapshot.
- A `Decision` replaces `pending`. `answer()` sends once per decision id; a Snapshot or a lost connection forgets answered ids, so the server's repeat can be answered.
- `GameOver` sets `result` and makes a later close final. `StaleDecision` and `IllegalAnswer` give a notice and a Resync; `BadMessage` and `Internal` only a notice.
- A wrong token is a refused upgrade, which a browser reports as 1006, so the client retries forever (`attempt` counts).

| Close or message | Socket status |
|---|---|
| 1001, 1006, 1013 and any code not listed below | `reconnecting`: retry after 250 ms doubling to 15 s, less up to 50 % jitter; a Welcome resets it |
| 4000 | `replaced`, no retry; `connect()` takes the seat back |
| 1000, 1002, 1007, 1008, 1009; `Error` UnsupportedVersion or UnknownGame | `stopped`, reason in `notice` |
| any close after `GameOver` | `stopped`, no notice |

## Create and game pages

- `/` (`create/CreateGamePage`): decks from `api.listDecks()`, 2 to 12 seats with a name and a deck, format Normal (Commander shown disabled). `create/form.ts` repeats the server's checks; `create/apiErrors.ts` words every `ApiError` code.
- `/game/$gameId#seat=<n>&token=<token>` (`game/GameRoute`) reads the fragment from the router's raw `location.href`, because its `hash` is percent-decoded.
- A seat token lives only in the fragment, which a browser never sends. `components/SeatLinkRow` prints it only inside a `data-share-link` anchor, and tests scan the page for tokens outside those (`test-utils/leaks.ts`). `lib/invites.ts` keeps the host's links in `sessionStorage` (every access in try/catch) so the lobby can hand them out after the create page is gone.
- `game/GamePage` makes one session per (game, seat, token) and connects one microtask after mount, so StrictMode's mount, unmount, mount opens one socket. `game/gameContext` gives `useGame()` (`game`, `session`, `lookup`) and `usePlay(selector)`; `lookup` is `GET /api/games/{id}`, polled every 3 s in the lobby.

| Store | Screen (`connectionOf`, `game/connection.ts`) |
|---|---|
| no seat link and not signed in | `SeatLinkMissing`; no socket |
| `phase` over | `GameOver`; no connection notice |
| `status` replaced or stopped | `ConnectionNotice` alone: the reason and a button to connect again |
| idle or connecting, never welcomed | "Connecting to the game…" |
| `reconnecting`, `attempt` below 5 | "Connection lost. Reconnecting… (attempt N)" |
| `reconnecting`, `attempt` 5 or more | "Cannot reach the game" and "Try again now"; the socket keeps retrying and a Welcome moves the page on |
| welcomed | `Lobby` while `phase` is lobby, `GameTable` while playing |

After a Welcome the lobby or table stays under the reconnect notice. A `notice` shows above any other screen as a dismissible alert.

## Lobby

- The `Lobby` message ([protocol.md](protocol.md)) is kept whole as `lobby` (`{seats, needed}`) and dropped when the connection goes or the game starts, so it is only what the open socket last heard.
- `game/lobbyModel.ts` (pure) makes the rows. The roster wins; before it and while reconnecting the REST lookup stands in, with "Seat N" for an unnamed seat. This page's own seat is connected exactly when its socket is open.
- A polite live region (`aria-live`, no role, because the connection banner is the page's `status`) announces what changed between two looks (`announcement`): at most three changes, none for this seat's own connection.
- The button follows the server's roster ("Ready", then "Not ready"). A reconnect sends no Ready of its own, and the second click of a double click is ignored (`event.detail`), so a fast connection cannot take the Ready back by accident.

## The table

`features/table/GameTable` takes no props: it reads with `usePlay(selector)` and starts from `TableOptionsContext` (`mode`, `focus`, `effects`, `effectsSlow`; only the dev route sets it).

- Layout (`layout.ts`, pure): opponents run in turn order after the viewer, who is drawn last, at the bottom. Grid mode (default up to 4 seats) draws every opponent in `min(3, opponents)` columns. Focus mode (default from 5) draws a tile per opponent and one opponent large: the active player, else the priority holder, else the next seat still in, following the turn until a tile is chosen ("Follow the turn" undoes it). The toolbar switches modes at any size. From 8 seats cards are compact; lands always are.
- The views carry no art, counters, attachments, keywords or commander damage and tax, so none is drawn; the panel's slot reads "not tracked yet" and `data-slot="card-art"` waits for an image id.
- Opening hands: while `view.stage` is `Mulligan`, `OpeningStatus` lists every seat's public progress from `PlayerView.mulligan` (`describeOpening`) at any size, because from five seats only one opponent has a panel.
- Your turn: while `isYourTurn` (stage Turns, no result) the banner and the viewer's panel take the `your-turn-cue` utility (ring and glow) and the banner a "Your turn" badge. `features/game/useTurnTitle` prefixes the tab title with "Your turn · " and, while the tab is hidden and a decision waits, alternates it with "● Your move" (held on it under reduced motion).
- Rendering: a `Card` reads only its own object and combat role and is `memo`; rows read ids through `useShallow(battlefieldRow)`; `selectors.ts` splits the shared battlefield by controller once per view. A batch that taps one permanent redraws that card and no row (`perf.test.tsx`, 12 seats with 40 permanents).
- Cards carry `data-object-id` and panels `data-seat`; the prompt mounts in `[data-slot="decision-line"]`.

## Prompts

`features/table/prompt/` answers the decision this seat faces.

- `PromptHost` (in `CentreStrip`, in place of the read-only "Waiting for ..." line while `ownDecision` is set) renders `PROMPTS[kind]` with the kind's `options` in a scope keyed by decision id, so a new decision remounts the prompt with an empty selection.
- `registry.ts` has an entry per `DecisionKind`; a new kind stops compiling until it has one (`kinds/<Kind>.tsx`, a `PromptComponent<'Kind'>` with props `{ options }`). `kinds.test.tsx` fails while an entry is the `UnsupportedPrompt` stand-in.
- `usePrompt()` gives `{ id, name, decision, answer, selection }`. `answer(a)` wraps `session.answer`: once per decision, false when already sent, refused by the socket (a retry is allowed) or when the store holds another decision by now. `PromptLayout` (prop `text`, controls as children) draws the shared shape: the decision's name, one polite live sentence, a row of controls.
- Card marks (`prompt/marks.ts`): `useCardMarks(marks, onSelect, { toggles })` takes a `Map<ObjectId, 'legal' | 'dimmed' | 'selected'>` from `markCards(own, legal, selected)`. A click or Enter/Space on a legal or selected card calls `onSelect(id)`; a `Card`'s own `highlight` and `onSelect` props win. Each `Card` subscribes to its own id, so publishing redraws only the changed cards.

| Kind | Legal cards | Controls | Answer |
|---|---|---|---|
| `Priority` | `playable_lands`, `castable`, permanents with `mana_abilities` | a click plays, casts or taps (`ManaChoice` asks which mana when a permanent has several abilities); [pace controls](#pace-controls) | `PlayLand`, `CastSpell`, `ActivateMana`, `Pass` |
| `Mulligan` | none | Keep; Mulligan (disabled unless `can_mulligan`) | `Keep`, `Mulligan` |
| `BottomCards`, `Discard` | the offered cards, as toggles | Confirm waits for exactly N | ids in order chosen |
| `PayMana` | untapped permanents with a mana ability | Confirm payment; Pay automatically, or Pay the rest for me once something is chosen and the cost is not covered (both need `auto_available`); Cancel casting (`can_cancel`) | `Manual`, `Auto`, `Complete`, `Cancel` |
| `DeclareAttackers` | the offered creatures, as toggles | Attack at; Attack with all; Confirm attack; No attack | `[creature, target]` pairs |
| `DeclareBlockers` | creatures with something to block | click a blocker, then the one attacker it blocks (a list, so an attacker on an undrawn seat can be picked); Confirm blocks; No blocks | `[blocker, attacker]` pairs |
| `AssignDamage` | blockers | a number box per blocker; Confirm | `[blocker, amount]` pairs |

Every prompt but Mulligan and AssignDamage dims the viewer's other cards.

- Mulligan: the sentence reads `bottom_if_keep`, `bottom_if_mulligan` and `free_mulligans`, so the client derives no count.
- Nothing to do: the newest `NothingToDo` event of this turn shows "You have no creatures that can attack" (or "block") until the viewer has answered something, since the engine usually asks for priority right after.
- PayMana: mana that would be wasted is not offered (`payment.ts` `fits`), so a payment is complete exactly when the engine's check passes; `payment.ts` resolves the cost as `ManaCost::resolutions` does (each hybrid and Phyrexian symbol in one of its ways) and `payment.test.ts` checks it against an independent rule. `toPayment` sends `{ pool, taps, life }`, 2 life per Phyrexian symbol paid with life; a life switch works while the life paid stays within `max_life` and the chosen mana still has a place (`lifeChoices`).
- DeclareBlockers: a blocker blocks one attacker, as the engine demands, and `can_block` alone decides who may be blocked.
- AssignDamage: the engine takes any whole-number split that adds up to `power`, so that is all the prompt enforces. The first split (`damageSplit.ts`) gives lethal damage in the offered order and the rest to the last; the views carry no keywords, so deathtouch is not in the lethal hint.

## Combat overlay and previews

`features/table/combat/` draws what combat will do from the engine's `combat_forecast` in the table view ([engine.md](engine.md), [protocol.md](protocol.md)); `net/parse.ts` checks it down to its leaves. The client never works an outcome out: it shows the engine's numbers and hides them when they cannot hold.

- The seam: `useBlockPreview()` (`forecast.ts`) is the forecast to draw; the overlay, the marks, the key, the sentences and the prompts' outcome all ask it. It is the table's forecast while no choice is pending and the engine's preview of the choice being made otherwise (`previewFor`, a pure rule), because the table's forecast describes combat as it stands. A forecast is never drawn for a choice it does not describe: marks go the moment the choice changes and return with the reply.
- `CombatOverlay` is an SVG over the table column (`aria-hidden`, `pointer-events: none`, mounted only while a line exists) in two layers: the lines under the centre strip and a faint copy above it, so a line behind the prompt can still be followed. `plan.ts` (pure) turns the combat view, forecast and unsent choices into lines: attacker to seat or planeswalker (muted), blocker to attacker (strong, coloured by the pair's outcome), unsent choices dashed.
- `rects.ts` measures the DOM when anchors appear and, once per frame, when something may have moved, never per render; jsdom has no layout, so tests inject a `RectProvider`.
- Marks: `data-doomed` and a "Dies" badge, a "-N" badge for damage, and on an attacked seat its life before and after ("20 → 16") with "Lethal" (`LifeForecast`). The key (`CombatLegend`) lists only the kinds of line on screen.
- DeclareBlockers publishes its pairs and DeclareAttackers the attack so far (`usePublishPending`, `pending.ts`). Both end with `prompt/PreviewOutcome.tsx` (`data-state` `checking`, `ready` or `unavailable`): the engine's sentences, "Checking outcome…" or "Outcome shown after you confirm."; while checking, the last sentences stay in the page (`aria-hidden`, `invisible`) so the section keeps its height.
- Preview ([protocol.md](protocol.md), Preview): `session.preview(decision, answer)` sends one `Preview` for the pending decision and remembers the answer's key (`previewKey`, the answer as JSON, so order of choice counts). Replies come in order, so the client keeps only the newest question's as `store.preview = { decision, key, forecast } | null`, dropped with the decision, a snapshot, game over and the connection. An `Error{BadMessage}` while a preview is unanswered means an older server does not know previews: no notice, and none are asked again until the next connection.
- `usePreviewRequest(decision, answer)` asks `DEBOUNCE_MS` (80 ms) after the choice last changed, so a run of clicks asks once, and returns `idle`, `checking`, `ready` or `unavailable` (no forecast, a session that would not send, or `GIVE_UP_MS`, 1.5 s, without a reply). Prompts pass the answer Confirm would send (`blocksAnswer`, `attacksAnswer`, also used by the seam), so the question's key is the choice's key.

## Table feedback

`features/table/fx/` draws short animations for what a batch did. They decorate the true state, which the table already shows when they start: nothing waits for them, no answer is delayed, and ending one only removes an attribute.

- `watchBatches(store, fx)` (`batch.ts`) subscribes to the play store. `batchBetween(prev, next)` takes a change for a batch only when `seq` moved on by exactly the events appended to the log, so snapshots, decisions and chat never animate. `deriveEffects(before, after, events)` (`derive.ts`, pure) returns `Effect[]` in a fixed order (life by seat, then `attacked`, `blocked`, `damaged`, `entered`, `died`, each by object id) and reads the object diff off the two moments, since the fold keeps the identity of every unchanged object.

| Effect | Comes from | ms |
|---|---|---|
| `attacked`, `blocked` | `AttackersDeclared`, `BlockersDeclared` and the combat diff | 500 |
| `damaged` | a permanent: `DamageDealt`, else newly marked damage; a seat: the life diff, since no event says life | 550 |
| `healed` | a seat: the life diff | 550 |
| `entered` | a permanent that was not on the battlefield | 350 |
| `died` | battlefield to graveyard, or named by `CreatureDied` (a token); at most 24 a batch | 500 |

- A `died` effect carries the card's last look, so a ghost stands where it was; a player who loses takes their permanents out of the game, so they vanish without ghosts.
- `createFx(options)` (`store.ts`) keys effects by card, seat and battlefield row; `FxState` is replaced, never changed, so an untouched card keeps its snapshot. One timer per kind and batch removes effects after `FX_MS`; a batch plays at most 120. `mode` (`off`, `reduced`, `full`) is read at every batch; `reduced`, while the system asks for less motion, plays only still highlights.
- `TableOptions.effects` is on by default except under vitest (`import.meta.env.MODE === 'test'`), so other tests see no timers and no extra markup; a test turns it on with `renderTable({ options: { effects: true } })` and `vi.useFakeTimers()`.
- `Card` sets `data-fx` and `data-fx-side` (`own` or `other`), `LifeFx` wraps the life figure, and `CardRow` puts each `GhostCard` back before the card it stood before, as an `aria-hidden` `li` with no `data-object-id` and nothing focusable.
- Durations are the custom properties `--fx-<kind>-ms` in `fx/fx.css`, which `fxCss.test.ts` holds equal to `FX_MS`; `--fx-slow` stretches them.

## Pace controls

The Priority prompt's controls (`prompt/PassControls.tsx`), over the wire rules in [protocol.md](protocol.md) (Pace of play) and the design in [arena-flow.md](arena-flow.md).

### Primary button and yields

`data-slot="primary-action"` answers `Pass` (disabled when `can_pass` is false). `primaryLabel` (`passWords.ts`) is display only; the server decides what happens. "Pass priority" is never a visible label; it is the `aria-description`.

| Situation | Label |
|---|---|
| a spell on the stack | "Resolve <top spell>" |
| the viewer's turn, before the last window | "Next: <next step>" ("Next: Combat") |
| the viewer's turn, the End step | "End turn" |
| another seat's turn | "Continue" |

- "End turn" (`data-slot="secondary-action"`; not drawn when the primary button already ends the turn) asks for `yieldUntil('EndOfTurn')`. The menu "More ways to pass" has "Resolve everything on the stack" (`Resolve`, only over a stack) and "Pass until my next turn" (`MyTurn`).
- A yield is asked once per prompt: the buttons stay off until the server answers with `Yielding` (the prompt goes) or a fresh decision (the yield ended at once). The server refuses a yield under Full Control, so there `yieldUntil` sends nothing and the prompt draws neither "End turn" nor the menu.
- `YieldIndicator` ("Passing until the end of turn", with "Stop passing" for `cancelYield()`) shows while the store's `yielding` is set; only `Welcome` and `Yielding` set it. A `Priority` decision on screen is cleared when a yield starts or, for a yield that ended inside one step and was never told, with the next batch.

### Stops and Full Control

`features/table/stops/` and `play/session.ts`. The server decides the windows from the stops and the mode ([protocol.md](protocol.md), Stops and Full Control); the page only asks for them and shows what the server holds.

- `stops` (`{side, step, keep}`; `keep` true is lasting, false goes once its step has passed) and `fullControl` (`Off`, `NextWindow`, `Held`) are replaced whole by `Welcome`, `Stops` and `FullControl` and survive a dropped connection until the next `Welcome`.
- `StopLadder` (in `StopsBar`, beside `FullControlToggle`) has a row for your turns and one for opponents' with a button per step in turn order (ten; first-strike damage joins while it is the current step or holds a stop). Untap and cleanup get none: the server refuses them, so `setStop` sends nothing for them (`play/stops.ts`). A lit button (`aria-pressed`) is a stop the server holds; `data-keep` is `turn` or `lasting`. Below 640 px the rows fold behind "Stops · 2 set".
- A press sets a one-turn stop and clears a held one; shift and press, or the Keep toggle (for touch), sets a lasting stop and makes a one-turn stop lasting (`pressRequest`). The toggle asks for Held, or Off when a mode is on; its menu asks for the mode it names.

### Response clock

- A `Decision` with `deadline_ms` ([protocol.md](protocol.md), Decisions) becomes `pending.deadlineMs`. The session counts it down in `PlayState.clock` on its own `timers` (a test's manual clock moves it) and, at zero, clears `pending` itself: the server has answered for the seat, and a prompt left up would invite a stale answer.
- `prompt/ResponseClock` shows "Passing for you in 7 s" as a `role="timer"` that does not announce each second; the last three seconds are urgent (`data-urgent`). Its bar is the `response-clock-bar` utility in `prompt/responseClock.css`.

### Keys

| Key | Action |
|---|---|
| Space | presses the primary button |
| Enter | presses "End turn"; where none is drawn (Full Control, the End step) it presses the primary button (`enterYields`) |
| Shift+Enter or another modifier | nothing |
| Ctrl alone | Full Control `NextWindow`; again, Off; nothing from `Held` |
| Ctrl and Shift together | Full Control `Held`; again, Off |

Space and Enter act only when nothing has the keyboard (`isFocusFree`: no field, button, link, menu or dialog has focus, no modifier is held, the key is not repeating); a focused card plays itself instead. `useFullControlKeys` follows the same rule and `ModifierChords` fires on release, so Ctrl+Shift+I and Ctrl-click do not count.

## Accounts and games pages

Sign-in uses the server's accounts ([server.md](server.md); bodies in [protocol.md](protocol.md), REST bodies). A guest needs no account: a seat link opens a seat. A server without `DATABASE_URL` answers 503 `accounts_unavailable`, and the app then offers no sign-in anywhere.

- `net/accounts.ts` `createAccountsApi({fetch, baseUrl})` sends `credentials: 'include'`, as `api.createGame` does, which binds seat 0 to the creator. A failure is an `AccountError {code, status, retryAfter}` with the server's `AuthErrorCode` or a local `unknown`, `network` or `bad_response`; `AUTH_ERROR_CODES` is a `Record` over the generated union, so a new code stops the build until it has words.
- `features/auth/store.ts` `createAuthStore({api, initial})` is the app's `appAuth`, started once by `AuthProvider`; a page without a provider (most tests) gets a stand-in whose status is `unavailable`. `status` is `loading` until `GET /api/auth/me` answers, then `signed-in` (200), `signed-out` (401, and a network, 5xx or unreadable answer, since nothing is then known to be signed in) or `unavailable` (503).
  - A failed `logout` keeps the user and sets `notice`, since a cookie that is still alive must not look signed out. An answer that arrives after who is signed in changed, or after a newer question, is dropped (`round` counter).
- `/login` and `/register` (`AuthScreen`, `AuthForm`) check a username of 3 to 32 letters, digits, `_` and `-` and a password of 10 to 128 characters (`rules.ts`, copies of the Rust constants) before sending. The password is in no React state: the field is uncontrolled, read at submit, sent, and emptied once the answer is in. Every `AuthErrorCode` has a sentence and a field (`errors.ts`; a wrong sign-in names neither half).
- `next`: `sanitizeNext` (`features/auth/next.ts`) accepts only a path on this site: one leading `/`, no backslash or control character (a tab inside `/<tab>/host` is deleted by URL parsing and gives `//host`), no `//` after dot segments, not `/login` or `/register`, and no fragment, because a seat token must not travel in a query string. `parseAuthSearch` always names the key (`next: undefined` when refused): the router merges what a route validates over the raw address, so leaving the key out would let the unchecked value through.
- `/games` (`features/games/MyGamesPage`): signed out goes to `/login?next=/games`. A row (`GameRow`) offers "Rejoin" for an open game, a link to `/game/<id>` with no fragment, which connects through the session. A finished game has no link: the server drops it from memory ten minutes after it ends, so the row is the record.
- The create page waits until it is known who is signed in; a signed-in creator gets seat 1 named after the account. The lobby shows "signed in as" on seats with an account (`useSeatAccounts`).
- Game page: with a seat link, `GamePage` joins by token whoever is signed in (the server binds an unbound seat to the signed-in visitor). Without one and signed in, `GamePage account=<username>` opens `/ws/{game}` (no `GameSocketOptions.token`) and the cookie of the upgrade picks the seat; `Join` carries the username (the server ignores `name`). `ConnectionNotice` words this session connection (`via="session"`).

## Dev routes

Both exist only with `import.meta.env.DEV`; `devBuild.test.ts` fails if a fixture marker or card name reaches the production bundle (a DEV build must contain them, so the test can fail). By hand: `pnpm build && ! grep -r dev-table-fixture dist`.

| Route | Parameters |
|---|---|
| `/dev/table` | `seats` 2 to 12 (default 4); `mode` `grid` or `focus`; `focus` a seat; `turn=you` makes it the viewer's turn; `prompt` a name below; `fx` `attack`, `block`, `damage`, `death`, `life` or `enter` plays that move 600 ms after load, and buttons at the top replay any move; `slow` above 1 up to 20 stretches every effect; `preview` 0 to 10000 ms delays the fixture's preview answer (120 by default); `stops` such as `own:main1,opp:end:keep` (`own` or `opp`, a step name, `keep` for lasting; `dev/stopSpec.ts`); `fc` `next` or `held` |
| `/dev/lobby` | `seats` 2 to 12 (default 4); `ready` and `away` seat lists (`0,2`); `me` this page's seat; `names=long` gives 32-character names. Ready and Not ready work as if a server answered. |

`/dev/table` shows the real table over `features/table/dev/fixture.ts` (about 40 permanents per seat, an attack, a stack, a log). `fixtureSession` (`dev/devGame.ts`) answers as the server would.

The `prompt` names are the keys of `DEV_PROMPTS` in `dev/prompts.ts` (`DEV_TABLES` sets the moment of the game each needs): `priority`, `priority-main`, `priority-end`, `priority-wait`, `priority-clock`, `block-clock`, `yield`, `yield-resolve`, `yield-turn`, `forecast` (`turn=you` flips the sides), `attack`, `block`, `mulligan`, `mulligan-free`, `bottom`, `discard`, `damage`, `damage3`, `damage1`, `pay`, `pay-short`, `pay-locked`, `pay-hybrid` and `pay-low-life`.

## Testing

- vitest on jsdom: `scripts/check.sh web`, or `pnpm test` in `web/`.
- `src/test-utils` has the fakes (`FakeNetwork`, `FakeTimers`, `startSession()`), the Rust-generated `transcripts` (replayed through the real store by `transcript.test.tsx`) and the harnesses: `renderGame()` (fake sockets, manual clock, in-memory REST), `renderTable` and `renderLiveTable`, and `renderApp(url, {accounts, rest})` with `startAccounts(script)`, an in-memory account server.
- `test-utils/passControls.ts` finds the primary button by its slot, since its words change. A new prompt copies `Priority.test.tsx` (table-driven) and `PriorityLive.test.tsx` (real session, one Answer per decision).
- Screenshots: `node scripts/ui-shot.mjs <url> <out-prefix>` (options in `CLAUDE.md`) over a dev route. The page loads at the first width, so shoot one width per run: state read at mount, such as the drawer, does not follow a later width.
### End-to-end

`pnpm e2e` (`playwright.config.ts`) starts `cargo run -p mtg-server` (an empty `DATABASE_URL` gives the in-memory store and the built-in demo decks) on port 8301 and `pnpm build` plus `vite preview` on port 5301, both new for every run.

- `E2E_SERVER_PORT` and `E2E_WEB_PORT` override the ports (defaults 8301 and 5301, `e2e/ports.ts`) to run several suites at once. `E2E_REUSE=1` keeps servers already listening there; `E2E_TRAIL=1` prints every answered decision; `--repeat-each=N` repeats the game.
- On NixOS Playwright's own browser cannot run. Point `CHROMIUM_PATH` at the Nix chromium with a glob, because the store path changes with every update: `CHROMIUM_PATH=$(ls -d /nix/store/*-chromium-[0-9]*/bin/chromium | head -n 1) pnpm --dir web e2e`.
- `full-game.spec.ts` (`driver.ts`) plays both seats through the real UI. A seat waits for a `[data-decision]` with a new `data-decision-id` (the id tells two Priority prompts apart), reads its legal cards (`data-highlight="legal"`) and answers with real clicks. There are no sleeps: waits are `waitForFunction` polls. The engine seed is random per game.
- `accounts.spec.ts` needs Postgres. With `E2E_DATABASE_URL` set to an empty scratch database (the server migrates it on boot) the config also starts a second server on port 8302 and a second preview on 5302 (`E2E_ACCOUNTS_SERVER_PORT`, `E2E_ACCOUNTS_WEB_PORT`). Without it `pnpm e2e` stays hermetic and the spec is skipped. Usernames are random per run, so one database serves many runs.

```sh
docker compose exec db psql -U magic -c 'create database magic_scratch'
E2E_DATABASE_URL=postgres://magic:magic@localhost:5432/magic_scratch pnpm --dir web e2e e2e/accounts.spec.ts
docker compose exec db psql -U magic -c 'drop database magic_scratch'
```

To try the account screens by hand, run the server on that database (`DATABASE_URL=… MTG_BIND=127.0.0.1:8318 cargo run -p mtg-server`) and the dev server in front of it (`MTG_SERVER_URL=http://127.0.0.1:8318 pnpm --dir web dev --port 5318`). `ui-shot.mjs` cannot set a cookie, so a signed-in page needs Playwright.

## Conventions

- Server-held state is shown, never predicted: ready, yields, stops and Full Control change on screen only when the server says so.
- Colour never carries meaning alone, and live regions are polite. A row of cards, a ladder row or a list is one tab stop (arrows, Home, End); a control that must keep focus when it cannot act is `aria-disabled`, not `disabled`.
- Every animation sits inside `@media (prefers-reduced-motion: no-preference)` (`fx.css`, `combat.css`, `responseClock.css`, the `your-turn-cue` utility of `index.css`); under `reduce` an effect is a still highlight, the clock bar is not drawn and the tab title does not flash. JS reads `useReducedMotion` or `prefersReducedMotion`. A CSS test per file (`fxCss.test.ts`, `combatCss.test.ts`, `responseClockCss.test.ts`, `yourTurnCss.test.ts`) fails when an animation leaves the media query.
- Colours are tokens with a light and a dark value; Tailwind class names are written whole so Tailwind can see them.
