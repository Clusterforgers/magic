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
- [ ] (d) Decision kinds the effects need, one task each across engine, protocol and a web prompt (the table in `docs/cards-dsl.md`, Decisions the effects need)
- [ ] Response clocks for the choice decisions of a non-active seat (cards, modes, numbers, options, yes or no, mana payments), each with a fallback answer, so one seat cannot hold the table; targets and trigger orders have theirs (`docs/server.md`, Response clocks)
- [x] (e) Keywords, one task each: flying, reach, first/double strike, deathtouch, trample, lifelink, vigilance, haste, menace, defender, indestructible, hexproof, ward; the damage prompt gets trample targets and the views carry keywords, counters and attachments (`docs/web.md`)
- [ ] (e2) Keywords hexproof and ward, one task each; counters and attachments on the object views; the blockers prompt reads `BlockOptions.min_blockers` so menace is explained before Confirm (`docs/web.md`)
- [ ] (f) Keyword actions (CR 701), one engine primitive each, with a web prompt where the player chooses (scope in `docs/cards-dsl.md`, Tasks)
- [ ] (g) Tokens, copy effects, X costs, alternative and additional costs, modal spells
- [ ] (h) Keyword abilities beyond combat (CR 702), one task each for the other `Keyword` variants (flash, equip, cycling, kicker, flashback, cascade, storm, ...); cost-based ones follow (g)
- [x] (i) Triggered abilities: APNAP ordering, delayed triggers, intervening if, with the target and trigger-order prompts and their response clocks (`docs/engine.md`)
- [ ] (i2) Arena flow P10: smart stops, per-ability yields, an order-by-timestamp preference, damage auto-assign and the engine's answer to a one-option decision (`docs/arena-flow.md`)
- [ ] (i3) State triggers (CR 603.8) and abilities that work from a zone other than the battlefield (`from`) (`docs/engine.md`)
- [ ] (j) Replacement and prevention effects (CR 614 to 616)
- [ ] (k) Layers: full CR 613 ordering with dependencies, timestamps, counters, auras, equipment

## M4 Multiplayer product
- [x] Invite-link lobbies on top of users
- [x] (B1) Deck list page: the account's decks with name, format, size and check state; new, import (text, Arena, MTGO), export and delete
- [ ] (B2) Deck builder page: card search with filters, add and remove by zone, counts, save with the check on save (`docs/deck-check.md`)
- [x] Deck check and the Verified marker: legality and manual-step cards reported and highlighted; a saved deck with a clean check carries a marker until it or the engine changes (`docs/deck-check.md`)
- [ ] (B3) Deck check in the builder: cards that are not verified highlighted with icon, word and reason, an issues-only filter, the Verified marker, and each deck's state in the lobby and the precon browser (`docs/deck-check.md`)
- [ ] Precon browser with one-click play
- [ ] Choose a deck in the lobby: a stored deck or a precon per seat; the table sees each seat's deck state as a count of manual cards, never their names (`docs/deck-check.md`)
- [x] N-player layout, game log, stack panel, stop settings
- [ ] UI rebuild: the table fits the screen, a turn change is unmistakable and games are joined from a list (`docs/ui-design.md`, Tasks). In order:
  - [x] (U1) Design tokens for both themes with a contrast test, a theme switch and an app shell with navigation
  - [x] (U2) Fit engine: card sizes from the space available and grouping of identical permanents, pure and tested
  - [x] (U3) Table grid: top bar, opponents, shared strip, own board, hand dock and side tabs in one viewport from 1280x720
  - [x] (U4) A pod panel and a hand dock fan that hold 40 permanents a seat
  - [ ] (U5) Controls dock: a 44 px pass button, a one-row stop ladder and a menu for the rest
  - [ ] (U6) Turn feedback: active-seat cues in the pods, the Focus cross-fade and a distinct theme for each phase
  - [x] (U7) Server: game visibility, open seats, claim and join code
  - [x] (U8) Server: public game listing, join, leave and release, with privacy tests
  - [ ] (U9) Game browser page: live list, join dialog, empty, loading and error states
  - [ ] (U10) Create-game visibility, invite link and code, lobby controls for host and guests
  - [ ] (U11) Page polish: game over, My games, sign-in and one page width scale
  - [ ] (U12) Layout gate at six sizes in both themes; the audit leaves `docs/ui-design.md`
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
