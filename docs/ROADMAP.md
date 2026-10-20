# Roadmap

Progress record by milestone: what is built and what comes next, in build order.

One item is one piece of about 15 minutes; tick it in the commit that finishes it; designs are in `docs/`.

## Done
- [x] M0 Workspace, lint rules, CI, `CLAUDE.md`, local Postgres compose
- [x] M0 Frontend spike: React stack locked in (`docs/spike-react.md`)
- [x] M1a `mtg-import`: Scryfall (array + JSONL), MTGJSON decks and index, deck-list parsers, name resolution; validated on real data (`docs/data-sources.md`)
- [x] M1b `mtg-db`: schema, upserts, card search, precon storage
- [x] M2a Engine core + `mtg-testkit`: zones, turns, priority, stack, combat, layers skeleton, bots

## M1 Card data
- [x] `mtg-db` follow-ups: reap stale `running` import runs, skip NUL-containing records instead of failing the batch
- [x] CLI binary `mtg-cli`: download Scryfall bulk + MTGJSON deck index/decks, ingest into Postgres (resumable, polite rate limit; `docs/data-sources.md`)
- [ ] Verify Moxfield, Archidekt, MTGO and Arena export parsers against real exports: the 29-deck corpus passes; open: a real MTGO `.dek`, an Arena client export, companions, custom cards (`docs/data-sources.md`)
- [ ] Importer 2b, name resolution on the corpus: by printing (set + collector number) before name, plus flavor, digital and multi-face names (`docs/data-sources.md`); a real-data test resolves all 183 exports
- [x] Card search HTTP endpoints in `mtg-server` (name, filters, color identity, pagination)

## M2 Engine and first playable
- [x] Merge M2a, reviewed against `CLAUDE.md`; design in `docs/engine.md`
- [x] `mtg-protocol`: Decision/Event/State-view wire types, generated TypeScript bindings, drift test
- [x] `mtg-server` actor (no network): per-seat channels, projected events and decisions, stale-answer and reconnect handling, `EventStore` trait; bots play full games
- [x] `mtg-server` transport: axum binary, create-game REST with seat tokens, one WebSocket per seat, demo decks, graceful shutdown
- [x] Protocol sync: each `Events` batch carries the table state and an object diff, so the client never folds engine events
- [x] Web core: typed REST and WebSocket client with reconnect and resync, zustand store
- [x] Web game UI: routes, create-game page, lobby and game-over screens, N-seat table, prompts for all eight decision kinds
- [x] Protocol and engine gaps the prompts exposed: mulligan counts, payment cancel and complete, hybrid and Phyrexian mana costs paid with mana or life
- [x] Playwright e2e: full 2-player vanilla game in the browser (`pnpm e2e`)
- [x] `mtg-server` persistence (a): Postgres `EventStore` and migration table
- [x] `mtg-server` persistence (b): game and seat records, resume running games on boot
- [x] Game flow (a) to (c): auto-pass, response clocks, a pass button that names what passing does, yields
- [x] Combat readability: engine `combat_forecast`, attack and block lines with outcome marks in the table
- [x] Block preview: the engine's outcome of a pending attack or block, shown while it is chosen
- [x] Table feedback: animations for attacks, blocks, damage, deaths, entries and life changes
- [x] Concede: a button with a confirm step
- [x] Lobby shows who has pressed Ready
- [x] Users, (a) server: accounts, session cookie, auth endpoints, seats bound to users, "my games", rejoin by session
- [x] Users, (b) web: login and register pages, account header, "My games" with rejoin
- [x] Resume checks the deck contents: a deck fingerprint per seat fails a game whose cards changed
- [ ] Game flow (d), copy MTG Arena's priority, stops and yields so the pace never tells the table what a seat holds (`docs/arena-flow.md`, P1 to P10). In order:
  - [x] (d1) Stops and Full Control on the wire (P1, P4)
  - [x] (d2) Stops in the actor with Arena's default window table (P2)
  - [x] (d3) Idle windows as short hidden clocks (P3): a window the seat cannot use waits 0.25 to 0.7 s (`MTG_DWELL_MS`), then passes as its own step and batch
  - [x] (d4) Full Control in the actor (P4)
  - [x] (d5) Web session for stops and Full Control (P5)
  - [x] (d6) Stop ladder in the table (P6)
  - [x] (d7) Keys like Arena (P7)
  - [x] (d8) Leak test (P8): two seeded games equal but for one hidden card send the other seat the same messages at the same times
  - [ ] (d9) Timer bank (P9): extensions earned and spent at expiry, a delay before the countdown shows, an optional clock for the active seat
  - [x] (d10) The leak test covers a spell and a trigger on the stack as well as combat windows

## M3 Rules depth
Design and task order: `docs/cards-dsl.md` (Tasks). (a) to (c) in order, then (d) to (k) in parallel.
- [x] (a) `mtg-cards` DSL: data model for every ability kind, loader, validation, support records and a 40-card corpus
- [x] (b) Card assembly: a deck of oracle ids becomes `CardDefinition`s, printed side from the import and rules from the DSL
- [x] (c) Effect pipeline core: ability objects on the stack, snapshots, the resolution journal, interpreter stubs, the replacement seam and `ChooseTargets` with the CR 608.2b recheck
- [ ] (d) Decision kinds still open, one task each across engine, protocol and a web prompt: ChooseNumber, DivideAmount, ChooseCosts, ChooseReplacement (the table in `docs/cards-dsl.md`, Decisions the effects need)
- [x] Engine side of YesNo, ChooseOption and ChooseMode: the answer checks and the effects that ask them (`May`, `Choose`, modal spells)
- [ ] A response-clock test through the actor for YesNo, ChooseOption and ChooseMode (`docs/server.md`, Response clocks)
- [ ] Response clocks for the choice decisions still without one (numbers, divisions, costs, replacements, mana payments), each with a fallback answer, so one seat cannot hold the table; targets, trigger orders, yes or no, options, modes, cards and orders of cards have theirs (`docs/server.md`, Response clocks)
- [x] (e) Keywords, one task each: flying, reach, first/double strike, deathtouch, trample, lifelink, vigilance, haste, menace, defender, indestructible, hexproof, ward; the damage prompt gets trample targets and the views carry keywords, counters and attachments (`docs/web.md`)
- [ ] (e2) Keyword ward, which needs the counter effect and a payment decision; attachments drawn tucked beside their host; counters and attachments in the `/dev/table` fixture with screenshots (`docs/web.md`)
- [ ] (f) Keyword actions (CR 701), one engine primitive each, with a web prompt where the player chooses (scope in `docs/cards-dsl.md`, Tasks)
  - [x] Discard and Sacrifice with the ChooseCards engine side; Archon of Cruelty is Verified
  - [x] LookAtTop, Scry, Reveal, Move and Shuffle with the OrderCards engine side; Opt and Brainstorm are Verified
  - [x] Destroy, Exile and Counter (a spell) on targets and "all" selectors; Doom Blade, Swords to Plowshares, Counterspell and Wrath of God are Verified
  - [x] Pay ("unless that player pays", "you may pay") and mana costs on activated abilities; Izzet Charm and Rhystic Study are Verified
  - [ ] Looked-at and revealed cards reach the viewer's views so the prompts can show their names
  - [ ] Web prompts for scry and the order of cards put back (ChooseCards and OrderCards already draw candidates from the views)
  - [ ] A `Pay` the player cannot afford is still asked, with No as the only answer, so the pace does not show whether they could pay (`docs/arena-flow.md`)
  - [ ] The support walker names `Cost.Mana.X` for an `{X}` ability cost, so such a card is not Verified before X costs run
- [ ] (g) Tokens, copy effects, X costs, alternative and additional costs, modal spells
  - [x] Tokens: CreateToken, AddMana of any colour and a sacrificed source as a cost; Treasure is a mana ability, Dockside Extortionist is Verified and Hero of Bladehold's Soldiers enter tapped and attacking
  - [x] Clue (`{2}`, sacrifice) and Food (`{2}`, `{T}`, sacrifice) are paid through the payment decision of an ability, the sacrifice among the events that announce it
  - [ ] Costs of activated abilities beyond `{T}`, mana and sacrificing the source: sacrificing a chosen permanent (the choice is made when the ability is announced), life, discard
  - [ ] Treasure and other sacrifice sources offered while paying a spell, and mana abilities of several mana (Sol Ring, Gaea's Cradle)
  - [ ] Tokens that copy an object, and a card's support counting the abilities of the predefined tokens it makes
- [ ] (h) Keyword abilities beyond combat (CR 702), one task each for the other `Keyword` variants (flash, equip, cycling, kicker, flashback, cascade, storm, ...); cost-based ones follow (g)
- [x] (i) Triggered abilities: APNAP ordering, delayed triggers, intervening if, with the target and trigger-order prompts and their response clocks (`docs/engine.md`)
- [ ] (i2) Arena flow P10: smart stops, per-ability yields, an order-by-timestamp preference, damage auto-assign and the engine's answer to a one-option decision (`docs/arena-flow.md`)
- [ ] (i3) State triggers (CR 603.8) and abilities that work from a zone other than the battlefield (`from`) (`docs/engine.md`)
- [ ] (j) Replacement and prevention effects (CR 614 to 616)
- [ ] (k) Layers: full CR 613 ordering with dependencies, timestamps, counters, auras, equipment
  - [x] `Effect::Continuous` with `ModifyPT` and `AddKeyword`: effects with a source, timestamp and duration in the state, derived through layers 6 and 7c and `keywords_of`, ended by their duration; Giant Growth is Verified
  - [ ] Left of (k): the other modifications as effects (`SetPT`, `SwitchPT`, `AddType`, `AddSubtype`, `SetColors`, `RemoveKeyword`, `SetController`, `CantAttack`, `CantBlock`), static abilities with dependencies (CR 613.8), characteristic-defining abilities, counters, auras and equipment, and keywords in snapshots

## M4 Multiplayer product
- [x] Invite-link lobbies on top of users
- [x] (B1) Deck list page: the account's decks with name, format, size and check state; new, import (text, Arena, MTGO), export and delete
- [x] (B2) Deck builder page: card search with filters, add and remove by zone, counts, save with the check on save (`docs/deck-check.md`)
- [x] Deck check and the Verified marker: legality and manual-step cards reported and highlighted; a saved deck with a clean check carries a marker until it or the engine changes (`docs/deck-check.md`)
- [ ] (B3) Deck check, remaining: each precon's state in the precon browser (`docs/deck-check.md`)
- [ ] Precon browser with one-click play
- [ ] Choose a deck in the lobby, remaining: precons as a third kind of deck (`docs/server.md`, `docs/deck-check.md`)
- [x] N-player layout, game log, stack panel, stop settings
- [ ] UI rebuild: the table fits the screen, a turn change is unmistakable and games are joined from a list (`docs/ui-design.md`, Tasks). In order:
  - [x] (U1) Design tokens for both themes with a contrast test, a theme switch and an app shell with navigation
  - [x] (U2) Fit engine: card sizes from the space available and grouping of identical permanents, pure and tested
  - [x] (U3) Table grid: top bar, opponents, shared strip, own board, hand dock and side tabs in one viewport from 1280x720
  - [x] (U4) A pod panel and a hand dock fan that hold 40 permanents a seat
  - [x] (U5) Controls dock: a 44 px pass button, a one-row stop ladder and a menu for the rest
  - [ ] (U5b) One step ladder: the ladder that shows the turn also sets the stops (click a step), as in Arena, and the second row of step buttons goes
  - [x] (U6) Turn feedback: badge, banner, ticker, step ladder and phase themes, an edge on the active seat and a dot on the priority holder, and a Focus board that follows the turn with a cross-fade
  - [x] (U7) Server: game visibility, open seats, claim and join code
  - [x] (U8) Server: public game listing, join, leave and release, with privacy tests
  - [x] (U9) Game browser join flow: a Join takes a seat in one click for an account and after a name for a guest, opens the game and says "Someone took that seat"
  - [x] (U9) Join with a code: `POST /api/join` names the game in `Joined`, and the Play page's code field joins through the same flow
  - [x] (U10) Create-game visibility and the lobby: the invite link and code, the host's visibility toggle, Close and removing a player, Leave, and the page an invite link opens
  - [x] (U11) Page polish: game over, My games, sign-in and one page width scale
  - [x] (U12) Layout gate at six sizes in both themes; the audit leaves `docs/ui-design.md`
  - [ ] Layout gate for the pages that need an account (Decks, My games, the builder with a real deck), in `accounts.spec.ts`
- [ ] Spectators and reconnect polish: a spectator seat, a face-down card rule in `zone_is_visible`, a "waiting on" message for tables of three or more
- [ ] Account settings: change the password, delete the account (its seats are freed); a game that failed to resume shows in My games with its reason

## M5 Commander
- [ ] `Commander` format rules: 100-card singleton, color identity, 40 life, command zone, tax, 21 commander damage; the views carry commander damage and tax so the pod can show them
- [ ] Fix: when the active player leaves mid-turn the turn continues without an active player (CR 800.4; `docs/engine.md`), with a scenario test
- [ ] Partner, companion, Commander-specific tests and Commander-format fuzz games with 4-6 players

## M6 Coverage
- [ ] Manual-resolve fallback mode for unimplemented cards (`docs/cards-dsl.md`, Compiler and Manual fallback)
- [ ] Oracle-text compiler for common templates with golden tests (`docs/cards-dsl.md`, Compiler and Manual fallback)
- [ ] Coverage dashboard over `Library::support`: states, reasons and clause spans per card, ranked by saved-deck demand; target: 100% of a chosen set of Commander precons (`docs/deck-check.md`)

## M7 Polish and deploy
- [ ] Animations beyond the table feedback (zone moves to hand, exile and library, a spell on the stack; `docs/web.md`), sounds, replays, solo play against bots
- [ ] Dockerfile, Kustomize manifests for k3s, Postgres, ingress + TLS on the user's domain (`docs/PLAN.md`, Deployment)
- [ ] Performance profile on real hardware with real card art
