# Web client

The browser client: game creation, lobby, table, decision prompts and accounts, on the wire contract of [protocol.md](protocol.md) and the transport of [server.md](server.md).

## Stack and layout

React, Vite, Tailwind, shadcn/ui on Base UI, TanStack Router and zustand. Commands are in `CLAUDE.md`.

- Pages live in `src/features` (`auth`, `create`, `decks`, `game`, `games`, `table`); `src/routes` only picks them. The Vite plugin rewrites `routeTree.gen.ts` on `pnpm dev` and `pnpm build`; commit it with the route change.
- `src/net` (REST, socket) and `src/play` (session, store) hold the client. `src/components` has the shell and the pieces pages share; `src/components/ui` is shadcn (`pnpm exec shadcn add <name>`) and is edited to the tokens. `src/protocol` has the ts-rs bindings and the transcripts Rust writes.
- The dev and preview servers proxy `/api` and `/ws` to `MTG_SERVER_URL` (default `http://127.0.0.1:8080`) and keep the browser's `Host`, which the server's Origin check compares with the page's `Origin`.

## Design system and shell

The tokens, scales and pieces of [ui-design.md](ui-design.md). `src/index.css` holds them as CSS variables that `@theme inline` maps to Tailwind classes; the `dark` class on `<html>` swaps the colours.

| Role | Classes | Use |
|---|---|---|
| `background`, `surface`, `table`, `raised` | `bg-background`, `bg-surface`, `bg-table`, `bg-raised` | page; panels and rows; sunken boards; menus and dialogs |
| `foreground`, `muted-foreground` | `text-foreground`, `text-muted-foreground` | text; secondary text |
| `border`, `border-strong`, `ring` | `border-border`, `border-border-strong`, `outline-ring` | separators; edges of fields and controls (`border-input` is the same colour); focus |
| `primary`, `on-primary` | `bg-primary text-on-primary` | main action, current page, selection |
| `turn-you`, `on-turn-you`, `turn-you-edge`, `turn-them` | `bg-turn-you`, `border-turn-you-edge`, `text-turn-them` | your-turn pill; edge of the active seat; an opponent's turn |
| `success`, `warning`, `destructive` | `text-success`, `bg-warning/12` | ready, urgent, lethal and Concede |
| `mana-<colour>`, `on-mana-<colour>` | `bg-mana-blue text-on-mana-blue` | pip fill and ink, the same in both themes |
| `card`, `popover`, `secondary`, `muted`, `accent`, `input`, `scrim` | shadcn's names | aliases of the roles above or a quiet step of them; `scrim` dims the page behind a dialog |

| Scale | Values |
|---|---|
| Type | `text-label` 11/16 (card labels only), `text-xs` 12/16, `text-sm` 14/20, `text-base` 16/24, `text-lg` 18/28, `text-2xl` 24/32, `text-life` 36/40 with tabular digits, `text-display` 32 to 48 by width; weights 400, 500, 600 |
| Radius | `rounded-md` card 6, `rounded-lg` control 8, `rounded-xl` panel 12, `rounded-full` pill |
| Elevation | `shadow-e1`, `shadow-e2` menus, `shadow-e3` dialogs; the `surface` utility is a panel (fill, hairline, radius) |
| Motion | `duration-fast`, `duration-base`, `duration-slow` 100, 200, 300 ms; `ease-out` in, `ease-in` out; `popup-motion` fades a popup and scales it only under `no-preference` |
| Width | `max-w-narrow` 28rem for a form, `max-w-page` 48rem, `max-w-wide` 72rem; `PageShell` sets one with the gutters |
| Breakpoints | `sm` 640, `lg` 1024, `xl` 1280, `2xl` 1600 |

- `tokens.test.ts` computes the contrast of every text, edge and ring pair from the stylesheet in both themes, with colours outside sRGB cut off and with their chroma lowered: 4.5:1 for text, 3:1 for edges, rings and other non-text parts. A state tint (`bg-success/12`, a hovered `bg-primary/90`) sits on the page, panels and menus, not on the table. The test also holds the scales above. `tokenUse.test.ts` keeps hard-coded and palette colours, sizes written in px or rem, text sizes outside the scale and translucent focus rings out of `components/` and the pages beside the table.
- Focus is a 2 px outline in `ring` with a 2 px offset, set once for `:focus-visible` in `index.css`; a component does not draw its own.
- `Button` heights are `xs` 24, `sm` 32, `default` 36 and `lg` 44 px, and 44 under `pointer: coarse`. `loading` shows a spinner and ignores presses while the button stays focusable (`aria-disabled`).
- `Popover` (a panel at its button; `keepMounted` keeps its content in the page, hidden, while it is shut) and `Sheet` (a dialog that hangs from a top bar of a given height, a full-width sheet on a phone and 24 rem of it from `sm`) join the menu and dialog primitives in `components/ui`; both are built on Base UI like the rest.
- `Pill` (neutral, success, warning, turn-you, turn-them; a control when rendered as a button or link, then `selected` rings it), `EmptyState`, `Kbd`, `Tooltip` for icon-only controls and `Skeleton` are the shared pieces. `Loading` is a polite status at once and the skeleton after 300 ms (`useAfterDelay`). An error is an `Alert` with an icon and a title, and a Retry where retrying works.

### Theme

`hooks/useTheme.ts`: `createThemeStore` keeps the choice (`light`, `dark` or `system`) in `localStorage` under `theme` (the system's choice removes the key), puts the `dark` class on `<html>`, and follows `prefers-color-scheme` and other tabs' `storage` events while a component is subscribed. It works when storage or `matchMedia` is missing or throws, keeping the choice for the page. A script in the head of `index.html` sets the class before the first paint; `themeScript.test.ts` holds it equal to the store. `ThemeMenu` in the header sets the choice.

### Shell

`components/AppShell` frames every route: a skip button (it focuses `main`; a `#main` link would replace the seat token in a game address), the header, `AuthNotice` and `OfflineNotice`, then `main`.

| Part | Content |
|---|---|
| Brand | "Magic", home |
| Main navigation | Play, Decks and My games (signed in only), Create; `aria-current` marks the page |
| Theme | `ThemeMenu`: Light, Dark, System |
| Account | `AccountNav`: Sign in and Register for a guest, a menu named after the username with Sign out for an account; nothing while unknown or without accounts |

Play opens `/` and Create opens `/new`. Under 640 px the navigation drops to a second row and Register leaves the header (the sign-in page links to it).

A page that takes the whole window calls `useFullBleed` (`components/appChrome`, counted, so two pages swapping do not flicker): the shell then draws no header, its root is the window high with `overflow: hidden` (`data-bleed`), and only the offline notice floats over the page. The game table is the one such page.

A page that only fits the window under the header calls `useFitWindow`: the root is the window high (`data-fit`) and `main` is a flex column that takes the rest, so the page scrolls one region inside itself and the page never scrolls. Play is the one such page.

## Data flow

- `net/api.ts` checks REST bodies at runtime; a failure is an `ApiError` with a code of `crates/mtg-server/src/api.rs` or a local `unknown`, `network` or `bad_response`. `net/parse.ts`: `parseFrame` is the only door from the wire (JSON, known `type`, required fields, 1 MiB cap); garbage is a `FrameError`, never a throw.
- The game browser calls are on the same client (`net/api.ts`): `listLobbies`, `joinGame`, `joinByCode`, `leaveGame`, `releaseSeat` and `setVisibility` ([server.md](server.md), Game browser endpoints). They reject with a `LobbyError {code, status, retryAfter}` whose code is the server's `LobbyErrorCode` or a local `unknown`, `network` or `bad_response`; `describeLobbyError` (`net/lobbyErrors.ts`) has words for every code, and `net/parseLobbies.ts` reads each body down to its leaves.
  - `listLobbies(previous)` sends the `ETag` of the listing it is given and, on a 304, resolves with that very object, so `result === previous` means nothing changed. A listing carries `fetchedAt`; a row's age now is `age_secs` plus the time since then.
  - A seat token goes in `Authorization: Bearer` and the session cookie rides along, so a call with no token acts for the signed-in user's own seat. `createGame` takes `visibility` and `held` and reads the join `code`.
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
| 4001 | `stopped`, no retry, notice "You were removed from this game."; the seat was given back or the host removed the player |
| 1000, 1002, 1007, 1008, 1009; `Error` UnsupportedVersion or UnknownGame | `stopped`, reason in `notice` |
| any close after `GameOver` | `stopped`, no notice |

## Create and game pages

- `/` (`browser/PlayPage`): the open games of `GET /api/lobbies`, a Create game link to `/new` and a "Join with a code" field. `useOpenGames` polls 5 s after each answer with the held `ETag` (a 304 keeps the same rows), pauses while the tab is hidden and keeps the rows, dimmed, beside an error with Retry. The rows are a table with a caption from the M breakpoint up and stacked rows under it, in a region that scrolls inside the page (`useFitWindow`); polling never moves focus, and `FocusKeeper` sends it from a removed row to the next Join. A join calls `onJoin(target)`; the route passes none, so every Join is disabled under the note "Joining is the next piece".

- `/new` (`create/CreateGamePage`): decks from `api.listDecks()`, 2 to 12 seats with a name and a deck, format Normal (Commander shown disabled). `create/form.ts` repeats the server's checks; `create/apiErrors.ts` words every `ApiError` code.
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

`features/table/GameTable` takes no props: it reads with `usePlay(selector)` and starts from `TableOptionsContext` (`mode`, `focus`, `effects`, `effectsSlow`, `cues`, `viewport`; only the dev route and tests set it). The table is exactly the window high and never scrolls the page.

### Regions and breakpoints

`TableGrid` is one grid inside a root that clips (`h-dvh overflow-hidden`). Each region is a cell with `min-height: 0` (`data-area`); a zone that cannot fit scrolls inside itself with an edge fade (`fade-scroll`, `useEdgeFade`), never the page. `useFullBleed` (`components/appChrome`) makes `AppShell` draw no header while the table is mounted: the offline notice, and the connection and server notices of `GamePage` (`data-slot="table-notices"`), float under the top bar.

| Region | Content | Size |
|---|---|---|
| bar | home, the turn badge, the side sheet's button (S, XS), the table menu | 44 px; 32 px on a phone on its side |
| opponents | the opponents' pods and boards | `minmax(0, 1fr)` |
| shared | the stack as chips, the prompt or the waiting line, priority, the combat key | `clamp(56px, 9dvh, 88px)` |
| own | your pod and battlefield | `minmax(0, 1.15fr)` |
| hand | your hand in one line, as tall as the dock allows | `clamp(112px, 17dvh, 200px)`; phone `clamp(96px, 14dvh, 120px)` |
| controls | the passing line, the stops (one row, or one Stops button), the pass buttons | bottom right, as wide as its content up to 480 px (760 with the stops row, 160 on a phone) |
| side | Log, Chat, Stack | by breakpoint |

| Breakpoint | Width | Side area |
|---|---|---|
| XS | under 640 | a sheet from the top bar; a phone layout |
| S | 640 to 1023 | a sheet from the top bar |
| M | 1024 to 1279 | a 44 px rail; a tab opens a 320 px panel over the table |
| L | 1280 to 1599 | a column of 22 % of the width, 280 to 320 px |
| XL | 1600 up | the same column |

- `layout.ts` is pure and holds the sizes: `regionSizes(viewport)` (rows, side kind, the most a prompt sheet grows), `gridRows`, `gridColumns` and `mainColumns` (the CSS templates, held equal to the numbers by `layoutRegions.test.ts`), `chooseMode`, `arrangeSeats` and `planTable`, which joins them for a window and a seat count. `useViewport` follows the window.
- Mode: grid for 2 to 4 seats unless a board column would be under 280 px (`gridColumnWidth`), focus otherwise; the menu's Grid and Focus override it. Focus draws a two-line tile per opponent and one board large: the active player, else the priority holder, else the next seat still in, following the turn until a tile is chosen ("Follow the turn" in the menu undoes it). From 8 seats cards are compact until a zone is measured.
- Seats (`SeatPlan.shape`): `duel`, one opponent, is a wide board beside a 168 px pod column; `columns`, 3 or 4 seats, are two or three columns of a one-line pod (`SeatPanel` `variant="line"`) over a board; `focus` is the tile strip over one board with a pod column. Your own board is a pod column beside the board. On a phone (XS) a pod is one line, yours at the foot of your board, the focused opponent's has none (its tile says it), and the strip leads with the active seat.
- Pods draw the seat panel as before (name, life, mana, counts, zones, command zone); only the viewer's hand has left it for the hand dock. The views carry no art, counters, attachments or commander damage and tax, so none is drawn; the panel's slot reads "not tracked yet" and `data-slot="card-art"` waits for an image id. A card with keywords lists their names as plain text (`keywordLine`): under the type line of a full-size face (`data-slot="card-keywords"`), in the preview, and in the spoken description; `groupPermanents` keeps permanents with different keywords apart.
- Shared strip (`SharedStrip`): the turn banner lies over it and the ticker is the last row of its sheet ([Turn cues](#turn-cues)); the prompt keeps its title for a screen reader only. A prompt that needs more than the strip grows into a sheet of at most 40 dvh, up over the opponents' board and starting beside the pod column, so your life, your board (the cards a prompt asks you to click), the hand dock and the controls stay in view and clickable; past that it scrolls inside itself.
- Controls dock (`ControlsDock`): top to bottom, the server's passing line, the stops and the pass buttons, so the primary button is the one in the bottom right corner. The pass buttons of the Priority prompt are drawn here, not in the prompt's line (`InControlsDock` portals them into `[data-slot="pass-slot"]`; without a dock they draw in place). From a 1280 px window (`planTable().stopsRow`) the stop ladder and Full Control are one row with the Mine or Theirs switch; below it one "Stops" button opens the same content as a popover above it (`StopsDock`). Never both, never in the table menu.
- A phone's dock (`useCompactDock`, set by `ControlsSlotProvider`) is one wrapping line of the Stops button and the menu of passing, then the primary button alone; "End turn" is the first item of that menu. At most five controls show at any size, a group that works as one (the ladder with its switch and Keep, Full Control with its menu) counting once, and every button is 32 px high or more, 44 px under a coarse pointer (`ControlsDockSizes.test.tsx`).
- Hand dock (`HandDock`): one line of cards as tall as the dock allows (`handCardWidth`), at the normal gap while they fit and overlapping by just enough (`handFan`, at least 62 % of a card shows) before the line scrolls.
- Side area (`SideTabs`, `sideState`): tabs Log, Chat and Stack with counts: the log lines worth reading (not passes or mana) and the chat lines of other seats since the tab was last on show, and the spells on the stack. Nothing counts while a tab is on show, and lines present when the table opens are read.
- Top bar and menu (`TopBar`, `TableMenu`): the turn badge reads "Your turn · 5 · Precombat main" (gold) or "Bo's turn · 5 · Declare blockers" (outlined), a heading, not a live region. The menu is a panel with Layout (Grid, Focus, Follow the turn), Theme and Concede, which asks `ConcedeDialog` hosted by the bar; the stops are in the controls dock at every size.
- Opening hands: while `view.stage` is `Mulligan`, `OpeningStatus` lists every seat's public progress from `PlayerView.mulligan` (`describeOpening`) at any size, because from five seats only one opponent has a pod. The hand dock shows the opening hand while the Mulligan prompt is up.
- Your turn: while `isYourTurn` (stage Turns, no result) the shared strip and the viewer's pod take the `your-turn-cue` utility (ring and glow) and the badge turns gold. `features/game/useTurnTitle` prefixes the tab title with "Your turn · " and, while the tab is hidden and a decision waits, alternates it with "● Your move" (held on it under reduced motion).
- Game over: the results list scrolls inside itself past 24 rem or 45 dvh, so the headline and the way to a new game stay in the window.
- Battlefield rows (`Battlefield`, `CardRow`, `CardStack`): each row draws the stacks of `groupPermanents`; a closed stack is the card on top with two edges behind it and a ×N chip. A click, Enter or Space opens it in place and moves the focus to its first member; Escape or the chip closes it. A stack keeps its key while any member stays, so it stays open while members tap, die or join. While the active prompt marks a member legal or selected, the stack is held open: its members take the marks and the clicks, the player cannot close it, and it closes with the prompt. A closed stack shows the dimming of the rest. `TableOptions.stacks` is on except under vitest, where a test of stacks turns it on.
- A tapped card lies on its side inside the box of an upright one: a face as wide as the box and five sevenths as high, its text upright (`data-turned`), so the fit counts one box for it. Card text is 11 px or more at every size (`MIN_TEXT_REM`, `cardSize.test.ts`).
- Opponent boards shrink before yours: your region is 1.15 times as tall as the opponents' and every board fits its own zone, so the same permanents never get larger cards on an opponent's board than on yours. 40 permanents a seat fit both boards of a duel from 1024x768 up without a zone scrolling (`rowStacks.test.ts`).
- Rendering: a `Card` reads only its own object and combat role and is `memo`; a `Battlefield` reads its seat's stacks as ids and keys (`battlefieldStacks` through `useStable`), and a row is `memo` over the ids and openness of its stacks. A batch that taps one permanent redraws that card, and its row only when a permanent changes stack (`perf.test.tsx`: 12 seats with 40 permanents; 4 seats stacked inside a time budget).
- Preview: hover and focus show a card large, a click that only looks pins it until Escape or Close, and a click that acts (the prompt marks the card: it plays, taps or is chosen) pins nothing, since a pinned preview covers part of the table and the prompt with it.
- Cards carry `data-object-id` and pods `data-seat`; regions carry `data-region` (`bar`, `shared`, `hand`, `controls`) and cells `data-area`; the prompt mounts in `[data-slot="decision-line"]`.

## Fit and grouping

`features/table/fit.ts` and `group.ts` are pure (no DOM, no store) and decide how big a zone's cards are and which permanents share a box.

- `fitRows(box, groups, limits)`: `box` is the zone's width and height in px; a group is a row with its `stacks` and its card shape (`aspect` 7/5; `scale` of the zone's card width, 1, or `LAND_SCALE` 0.8 for lands). It tries card widths from 112 px down to the 44 px floor in steps of 2 and answers with the first at which every row wraps inside the width and all lines fit the height. A width that fits keeps fitting at a smaller one, so a bigger box never gives a smaller card.
- Whole pixels: the box is floored; a row's box is the card width times `scale`, rounded to the nearest px (halves up), and its height that width times `aspect`, rounded the same way. `layout` carries each row's box, `perLine` and `lines`; components draw exactly those sizes. Every stack takes one box of its row's size, tapped or not. `FIT_LIMITS` puts 6 px between boxes and 4 between lines, wrapped or of different rows; a box narrower than one card still takes one card a line.
- When the floor does not fit the height, `scrolls` is true at the floor and `hidden` counts the stacks on lines cut off at the bottom of the box, the "+N" of the zone's edge fade. `rows` and `lines` count the rows with stacks and all wrapped lines.
- `groupPermanents(permanents, { groupOthers })` merges permanents with the same face into a `Stack` (`key`, `count`, `members`): name, owner, controller, token or card, types, colors, power, toughness, abilities, tapped, summoning sick, damage, counters and attachments (the last two once the views carry them). Tapped and untapped never share a stack. Lands and tokens always merge; other cards only with `groupOthers`, which the caller sets when a zone's fit is under `GROUP_OTHERS_BELOW` (64 px).
- Stacks come in the order of their first member. A merged stack's key is its face and a card that stays alone has its id, so a key survives members joining and leaving, and one member joining or leaving changes no other stack's key, count or place. `expandStack(stack)` returns the members, for opening a stack in place and for prompts that need each one; regrouping them gives the same stacks.
- Use: `battlefieldStacks(state, seat)` (`rowStacks.ts`) groups a seat's rows once per view into ids and keys, twice: lands and tokens merged, and every identical card merged too. `layoutSeat` fits the zone's content box (`useContentBox`, a `ResizeObserver`) with the first and, when its cards would fall under `GROUP_OTHERS_BELOW`, with the second: one box per stack, one per member of an open stack, lands at 0.8. It returns each row's entries (`ids`, `open`, `held`) and the `Fit`; the rows draw exactly those boxes, a tapped one included. A permanent in combat (`role`) never merges, so its marks and lines stay its own. `Battlefield` writes `--card-w` and `--card-h` on each row; a face is whole from 76 px and short below it. Without a measured size (jsdom, a hidden zone) cards keep the box of their size. `CardRow` takes the box (`box`) and, for the hand, the gap (`gap`, negative to overlap). `fitPerf.test.ts` holds grouping and fitting 40 permanents a seat for 6 and 12 seats to a few milliseconds.

## Prompts

`features/table/prompt/` answers the decision this seat faces.

- `PromptHost` (in `SharedStrip`, in place of the read-only "Waiting for ..." line while `ownDecision` is set) renders `PROMPTS[kind]` with the kind's `options` in a scope keyed by decision id, so a new decision remounts the prompt with an empty selection.
- `registry.ts` has an entry per `DecisionKind`; a new kind stops compiling until it has one (`kinds/<Kind>.tsx`, a `PromptComponent<'Kind'>` with props `{ options }`). `kinds.test.tsx` fails while one of the ten built kinds (the eight the table has always asked, `ChooseTargets` and `OrderTriggers`) is the `UnsupportedPrompt` stand-in. The other nine questions of spells and abilities are stand-ins in their own files, which their tasks replace.
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
| `ChooseTargets` | the legal objects, as toggles | a button per legal target, players and undrawn cards too; Confirm targets; Undo; Cancel the spell (`can_cancel`); the chosen targets listed with Remove | `Targets`, `Cancel` |
| `OrderTriggers` | none | the triggers in resolution order with Up and Down; Default order; Confirm order | the indices in the order they go on the stack |

Every prompt but Mulligan, AssignDamage and OrderTriggers dims the viewer's other cards.

- Mulligan: the sentence reads `bottom_if_keep`, `bottom_if_mulligan` and `free_mulligans`, so the client derives no count.
- Nothing to do: the newest `NothingToDo` event of this turn shows "You have no creatures that can attack" (or "block") until the viewer has answered something, since the engine usually asks for priority right after.
- PayMana: mana that would be wasted is not offered (`payment.ts` `fits`), so a payment is complete exactly when the engine's check passes; `payment.ts` resolves the cost as `ManaCost::resolutions` does (each hybrid and Phyrexian symbol in one of its ways) and `payment.test.ts` checks it against an independent rule. `toPayment` sends `{ pool, taps, life }`, 2 life per Phyrexian symbol paid with life; a life switch works while the life paid stays within `max_life` and the chosen mana still has a place (`lifeChoices`).
- DeclareBlockers: a blocker blocks one attacker, as the engine demands, and `can_block` alone decides who may be blocked. The prompt does not read `BlockOptions.min_blockers` yet, so one blocker on a menace attacker is sent and the engine refuses it.
- AssignDamage: without trample the engine takes any whole-number split that adds up to `power`, so that is all the prompt enforces. The first split (`damageSplit.ts`) gives lethal damage in the offered order and the rest to the last. The lethal figures are the engine's (`DamageOptions.lethal`, damage marked and deathtouch counted); for options without them the hint works them out from the toughness and the attacker's `keywords` (deathtouch makes 1 lethal) and names the keyword where it matters.
- With trample (`DamageOptions.excess`) the split may fall short: the rest goes to the player or planeswalker attacked, named in the prompt and in a line under the rows (`data-slot="damage-excess"`), and Confirm waits until every blocker has its lethal damage. The first split gives each blocker lethal damage and leaves the rest to that line; the answer still lists the blockers' shares only.
- ChooseTargets: the sentence names the source (the spell, or "Mulldrifter's ability" when `triggered`) and gives the engine's `wording` of the specification; `targetText.ts` holds the words. A click on a chosen target takes it back, one on another target changes a choice of one, and a full choice of several closes the rest. The list has every legal target because a player is not a card, a group or a zone may not draw one, and a card says where it is in brackets. A triggered ability has no Cancel (it cannot be backed out of); its way out is the response clock's `Remove` ([server.md](server.md), Response clocks).
- OrderTriggers: the list is in resolution order (the top resolves first) and the answer is the same list reversed, the order they go on the stack (`orderText.ts`). The engine's order (the last to trigger on top) is in place at first. A moved row keeps focus on its button; at an end the button stays focusable and does nothing, so a repeated press never sends the trigger back; the sentence says where the trigger went.

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

## Turn cues

`features/table/turn/` shows whose turn it is and what the game passed, from the batch that arrived. A cue only reads the play store: none waits for, delays or paces an answer, and each reads what every seat sees ([ui-design.md](ui-design.md), Turn and focus).

- `feed.ts`: `deriveFeed(before, after, events, names, options)` (pure) gives `{ banner, lines, skipped, announcement }` for one batch, and `feedBetween(prev, next, names)` gives it for two play states through `batchBetween`, once per state. A step is skipped when it began and ended inside the batch and a round of priority passed in it, so untap, cleanup and the steps the rules skip never count. The lines are the skip note ("Skipped Upkeep, Draw"; three names, then "and N more"), "You had no plays" for the viewer's own main phase passed at once with no yield, and the newest line the log does not draw quietly (a turn's start is the banner's). The viewer's own draw and moves into a hand or library read as every other seat reads them. `feedLeak.test.tsx` replays a game and a copy with one hidden card changed, and a game at the pace of an automatic pass and of a seat deciding, and requires equal feeds and cues.
- `timing.ts`: the times of the cues. The banner fades in for 250 ms, is held 600 and fades out 150 (100, 1,500 and 100 under reduced motion); a ticker line stays 3 s, at most three lines to a batch; a skipped step is dashed 1 s. `bannerTimes(reduced, slow)` stretches them by the dev route's `?slow=`.
- `useTurnFeed.ts`: `useTurnFeed(onFeed)` calls back for every batch while the table draws its timed cues (`TableOptions.cues`, on everywhere but under vitest); `useCueOptions()` gives that switch and the slow-down.
- `TurnBanner.tsx`: a band over the shared strip, "Your turn" (turn-you) or "Bo's turn" (turn-them) and the turn number, for a batch that changes the turn or the active seat. `aria-hidden` and without a pointer; a polite live region (`data-slot="turn-announcement"`) says the same, with the steps skipped.
- `TurnTicker.tsx`: one line in the bottom row of the strip's sheet (`h-4`, kept when empty, so the strip does not move). A newer batch replaces what is left of an older one at once. It is `aria-hidden`, since the log and the live region carry the words.
- `StepLadder.tsx`, `ladder.ts`: the step marks in the top bar from 768 px (`pipStates`): the current step lit in turn-you or turn-them, passed steps filled, steps to come hollow, a skipped step dashed for a second. `aria-hidden`; the badge says the same in words.
- `turn.css`: the banner's fades and the ticker line's rise. Only opacity changes outside the no-preference block; `turnCss.test.ts` holds that and the fade times equal to `timing.ts`. `cueTokens.test.ts` holds the contrast of the cue tokens `on-turn-them`, `step-passed` and `step-skipped` in both themes.
- Tests drive a table with `renderCueTable` (`test-utils/cueTable.tsx`: cues on, a manual clock, `deliver(events, to)` applies a batch) and build batches from `test-utils/turnEvents.ts`.

## Pace controls

The Priority prompt's controls (`prompt/PassControls.tsx`, drawn in the controls dock of the table, 44 px high), over the wire rules in [protocol.md](protocol.md) (Pace of play) and the design in [arena-flow.md](arena-flow.md).

### Primary button and yields

`data-slot="primary-action"` answers `Pass` (disabled when `can_pass` is false). `primaryLabel` (`passWords.ts`) is display only; the server decides what happens. "Pass priority" is never a visible label; it is the `aria-description`.

| Situation | Label |
|---|---|
| a spell on the stack | "Resolve <top spell>" |
| the viewer's turn, before the last window | "Next: <next step>" ("Next: Combat") |
| the viewer's turn, the End step | "End turn" |
| another seat's turn | "Continue" |

- "End turn" (`data-slot="secondary-action"`; not drawn when the primary button already ends the turn, and in a phone's dock it is the first item of the menu) asks for `yieldUntil('EndOfTurn')`. The menu "More ways to pass" has "Resolve everything on the stack" (`Resolve`, only over a stack) and "Pass until my next turn" (`MyTurn`).
- A yield is asked once per prompt: the buttons stay off until the server answers with `Yielding` (the prompt goes) or a fresh decision (the yield ended at once). The server refuses a yield under Full Control, so there `yieldUntil` sends nothing and the prompt draws neither "End turn" nor the menu.
- `YieldIndicator` ("Passing until the end of turn", with "Stop passing" for `cancelYield()`) shows while the store's `yielding` is set; only `Welcome` and `Yielding` set it. A `Priority` decision on screen is cleared when a yield starts or, for a yield that ended inside one step and was never told, with the next batch.

### Stops and Full Control

`features/table/stops/` and `play/session.ts`. The server decides the windows from the stops and the mode ([protocol.md](protocol.md), Stops and Full Control); the page only asks for them and shows what the server holds.

- `stops` (`{side, step, keep}`; `keep` true is lasting, false goes once its step has passed) and `fullControl` (`Off`, `NextWindow`, `Held`) are replaced whole by `Welcome`, `Stops` and `FullControl` and survive a dropped connection until the next `Welcome`.
- `StopLadder` (in `StopsBar`, beside `FullControlToggle`) draws one row with a button per step in turn order (ten; first-strike damage joins while it is the current step or holds a stop) and `StopSideSwitch`, "Mine" (your turns) or "Theirs" (opponents'), which picks the side on show. The row follows the turn and a side picked by hand holds until the turn changes hands; the side out of sight shows its count on its segment. Both rows stay in the page, one `hidden`. Untap and cleanup get none: the server refuses them, so `setStop` sends nothing for them (`play/stops.ts`). A lit button (`aria-pressed`) is a stop the server holds; `data-keep` is `turn` or `lasting`.
- `StopsDock` puts `StopsBar` in the controls dock as one row from 1280 px, else behind the "Stops" button (named "Stops, 2 set, Full Control held"). The popover (`layout="panel"`: the steps in a grid of five, Keep labelled) stays in the page while shut, so the Keep toggle, the side picked and the Control keys keep working.
- A press sets a one-turn stop and clears a held one; shift and press, or the Keep toggle (for touch), sets a lasting stop and makes a one-turn stop lasting (`pressRequest`). The toggle asks for Held, or Off when a mode is on; its menu asks for the mode it names.

### Response clock

- A `Decision` with `deadline_ms` ([protocol.md](protocol.md), Decisions) becomes `pending.deadlineMs`. The session counts it down in `PlayState.clock` on its own `timers` (a test's manual clock moves it) and, at zero, clears `pending` itself: the server has answered for the seat, and a prompt left up would invite a stale answer.
- `prompt/ResponseClock` shows "Passing for you in 7 s" (`clockText.ts` has the words for what the server answers at zero: a pass, no blocks, no targets, the triggers in the order they triggered) as a `role="timer"` that does not announce each second; the last three seconds are urgent (`data-urgent`). Its bar is the `response-clock-bar` utility in `prompt/responseClock.css`.

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

- `/decks` (`features/decks/DecksPage`): signed out goes to `/login?next=/decks`. `net/decks.ts` `createDecksApi({fetch, baseUrl})` is the typed client of the deck endpoints; a failure is a `DeckError {code, status}` with the server's `DeckErrorCode` or a local `unknown`, `network` or `bad_response`, and 503 `decks_unavailable` shows a notice in place of the list. A row (`DeckRow`) shows name, format, card count and the check state as a pill with an icon and a word (Verified, Manual play, Blocked, Unchecked) over one line that counts what needs manual steps or blocks the deck. Export links to `GET /api/decks/{id}/export` in three formats; Delete asks first. New deck saves a name and a format; Import reads a pasted list or a file, lists what it ignored or could not find, and saves the cards under a name. From `sm` up the page is as tall as the window under the header and only the list scrolls.

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
- `src/test-utils` has the fakes (`FakeNetwork`, `FakeTimers`, `startSession()`), the Rust-generated `transcripts` (replayed through the real store by `transcript.test.tsx`) and the harnesses: `renderGame()` (fake sockets, manual clock, in-memory REST), `renderTable` and `renderLiveTable` (they lay the table out for `TEST_VIEWPORT`, 1600x1000, which has the side column and the whole controls dock; `options.viewport` sets another, and jsdom has no layout, so a test that needs measured sizes fakes `ResizeObserver` and `clientWidth`, as `zoneFit.test.tsx` does), and `renderApp(url, {accounts, rest})` with `startAccounts(script)`, an in-memory account server.
- `test-utils/passControls.ts` finds the primary button by its slot, since its words change. A new prompt copies `Priority.test.tsx` (table-driven) and `PriorityLive.test.tsx` (real session, one Answer per decision).
- Screenshots: `node scripts/ui-shot.mjs <url> <out-prefix>` (options in `CLAUDE.md`) over a dev route. The page loads at the first width, so shoot one width per run: state read at mount does not follow a later width. The table follows `resize`, so one run may shoot several widths.
- A theme in a screenshot: the headless browser follows its system, so store the choice and reload once, which also runs the head script: `--wait "(sessionStorage.getItem('s') ? <ready> : (sessionStorage.setItem('s', '1'), localStorage.setItem('theme', 'dark'), location.reload(), false))"`.
### End-to-end

`pnpm e2e` (`playwright.config.ts`) starts `cargo run -p mtg-server` (an empty `DATABASE_URL` gives the in-memory store and the built-in demo decks; `MTG_DWELL_MS=0` passes for idle seats at once, since the default wait would take the full game from about 8 s to between 29 and 35 s) on port 8301 and `pnpm build` plus `vite preview` on port 5301, both new for every run.

- `E2E_SERVER_PORT` and `E2E_WEB_PORT` override the ports (defaults 8301 and 5301, `e2e/ports.ts`) to run several suites at once; the Vite dev server that serves `/dev/table` to `fit.spec.ts` listens on `E2E_DEV_WEB_PORT` (the web port plus 1000). `E2E_REUSE=1` keeps servers already listening there; `E2E_TRAIL=1` prints every answered decision; `--repeat-each=N` repeats the game.
- On NixOS Playwright's own browser cannot run. Point `CHROMIUM_PATH` at the Nix chromium with a glob, because the store path changes with every update: `CHROMIUM_PATH=$(ls -d /nix/store/*-chromium-[0-9]*/bin/chromium | head -n 1) pnpm --dir web e2e`.
- `full-game.spec.ts` (`driver.ts`) plays both seats through the real UI. A seat waits for a `[data-decision]` with a new `data-decision-id` (the id tells two Priority prompts apart), reads its legal cards (`data-highlight="legal"`) and answers with real clicks. There are no sleeps: waits are `waitForFunction` polls. The engine seed is random per game.
- `fit.spec.ts` sweeps the six sizes of [ui-design.md](ui-design.md) (1920x1080, 1440x900, 1366x768, 1280x720, 1024x768, 390x844), down and back up in one page: in a real two-seat game at the opening hands and at the first priority window, on `/dev/table` at 2, 4, 6 and 12 seats with 40 permanents a seat, and on the game-over screen. At each size the page does not scroll, every region is inside the window, and the hand, the primary pass button (or Keep), your life and the way to a new game are in view and not covered (`document.elementFromPoint`).
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
- Every animation sits inside `@media (prefers-reduced-motion: no-preference)` (`fx.css`, `combat.css`, `responseClock.css`, `turn.css`, the `your-turn-cue` utility of `index.css`); under `reduce` an effect is a still highlight, the clock bar is not drawn, the turn banner only fades and the tab title does not flash. JS reads `useReducedMotion` or `prefersReducedMotion`. A CSS test per file (`fxCss.test.ts`, `combatCss.test.ts`, `responseClockCss.test.ts`, `turnCss.test.ts`, `yourTurnCss.test.ts`) fails when an animation leaves the media query.
- Colours are tokens with a light and a dark value (`tokens.test.ts` checks their contrast); Tailwind class names are written whole so Tailwind can see them.
