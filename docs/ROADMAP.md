# Roadmap

Single source of truth for progress. Any fresh session continues with: "read `docs/ROADMAP.md` and do the next unchecked item".
Rules: one item = one focused piece of work (about 15 minutes for an agent). Tick the box in the same commit that finishes the item. Design reference: `docs/PLAN.md`.

## Done
- [x] M0 Workspace, lint rules, CI, `CLAUDE.md`, local Postgres compose
- [x] M0 Frontend spike: React stack locked in (`docs/spike-react.md`)
- [x] M1a `mtg-import`: Scryfall (array + JSONL), MTGJSON decks and index, deck-list parsers, name resolution; validated on real data (`docs/data-sources.md`)
- [x] M1b `mtg-db`: schema, upserts, card search, precon storage
- [x] M2a Engine core + `mtg-testkit`: zones, turns, priority, stack, combat, layers skeleton, bots

After a restart, run `git worktree list` and read each worktree's status before touching anything in flight.

## M1 Card data (rest)
- [ ] `mtg-db` follow-ups: reap stale `running` import runs, skip NUL-containing records instead of failing the batch
- [ ] CLI binary `mtg-cli`: download Scryfall bulk + MTGJSON deck index/decks, ingest into Postgres (resumable, polite rate limit)
- [ ] Verify Moxfield, Archidekt, MTGO and Arena export parsers against real exports. Verified on real exports of two Commander decks (`real_exports.rs`) and on a corpus of 29 public decks, 183 export files compared with each site's own card lists (`corpus_parse.rs`, `corpus_detect.rs`, `corpus_export.rs`, `docs/data-sources.md`): MTGO text is its own `DeckFormat`, Moxfield's single commander is promoted by `promote_implied_commander`, Moxfield `STICKERS:` is reported in `ParsedDeck::ignored`. Still open: a real MTGO `.dek`, a real Arena client export, companions and custom cards (no public deck found), and resolving the names parsing leaves as written (flavor and digital names, `A / B`, `A/B`, `A /// B`, front faces)
- [ ] Importer 2b, name resolution on the corpus: look up by printing (set + collector number) before name; resolve `A / B`, `A/B`, `A /// B`, front-face-only, flavor and digital (Arena/MTGO) names; real-data test that resolves all 183 corpus exports against Scryfall
- [ ] Card search HTTP endpoints in `mtg-server` (name, filters, color identity, pagination)

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
- [x] Game flow, from the first hands-on test: auto-pass, response clocks, a pass button that names what passing does, yields
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
  - [ ] (d3) Idle windows as short hidden clocks (P3): a window the seat cannot use waits 0.3 to 0.9 s (`MTG_DWELL_MS`), then passes as its own step and batch
  - [x] (d4) Full Control in the actor (P4)
  - [x] (d5) Web session for stops and Full Control (P5)
  - [x] (d6) Stop ladder in the table (P6)
  - [x] (d7) Keys like Arena (P7)
  - [ ] (d8) Leak test (P8): two seeded games equal but for one hidden card send the other seat the same messages at the same times
  - [ ] (d9) Timer bank (P9): extensions earned and spent at expiry, a delay before the countdown shows, an optional clock for the active seat

## M3 Rules depth
Design and task order: `docs/cards-dsl.md` (Tasks). (a) to (c) in order, then (d) to (k) in parallel.
- [ ] (a) `mtg-cards` DSL: Cost, Selector, Condition, Effect, Duration, TargetSpec, Amount and every ability kind as data files with loader, validation, support records and a 40-card corpus
- [x] (b) Card assembly: a deck of oracle ids becomes `CardDefinition`s, printed side from the import and rules from the DSL; `CardDefinition` moves to `mtg-cards`
- [ ] (c) Effect pipeline core: ability objects on the stack, snapshots, the resolution journal, interpreter stubs, the replacement seam and `ChooseTargets` with the CR 608.2b recheck
- [ ] (d) Decision kinds the effects need, one task each across `mtg-engine`, `mtg-protocol` and a web prompt, with tests: `ChooseMode`, `ChooseNumber` (X, "any number"), `DivideAmount`, `ChooseCards` (search, reveal, "choose a creature"; any seat), `OrderCards` (scry, surveil, put back, bottom in any order), `YesNo` for "may", `ChooseOption`, `ChooseCosts`, `OrderTriggers`, `ChooseReplacement`
- [ ] (e) Keywords, one task each with a table test: flying/reach, first strike/double strike, deathtouch, trample, lifelink, vigilance, haste, menace, defender, indestructible, hexproof/ward; `DamageOptions` gets player and trample targets and the views carry keywords for the damage prompt's lethal hint (`docs/web.md`, AssignDamage)
- [ ] (f) Keyword actions (CR 701), one engine primitive each, table-tested, with a web prompt where the player chooses: draw/discard/mill/shuffle, scry, surveil, fateseal, search, reveal, sacrifice/destroy/exile/counter, create tokens and populate, fight, explore, proliferate, investigate and the predefined tokens (Clue, Treasure, Food), amass, connive, learn, transform/meld, attach/equip, regenerate, goad, monstrosity, adapt, bolster, support, venture; Arena-only digital actions (conjure, seek, specialize) stay out
- [ ] (g) Tokens, copy effects, X costs (`ManaSymbol::X`, `ChooseNumber`, PayMana prompt, `payment.ts`), alternative and additional costs, modal spells
- [ ] (h) Keyword abilities beyond combat (CR 702), one task each: flash, equip/reconfigure, enchant, cycling, kicker/multikicker, flashback, madness, prowess, convoke, delve, cascade, storm, morph/disguise, suspend, evoke, overload, and the Commander-relevant partner and companion
- [ ] (i) Triggered abilities: APNAP ordering, delayed triggers, intervening if, per-step stops and holding priority, and the smart stops, per-ability yields, auto-ordered triggers and damage auto-assign of Arena flow P10 (`docs/arena-flow.md`)
- [ ] (j) Replacement and prevention effects (CR 614 to 616)
- [ ] (k) Layers: full CR 613 ordering with dependencies, timestamps, counters, auras, equipment

## M4 Multiplayer product
- [ ] Invite-link lobbies on top of users (accounts land in M2)
- [ ] Deck builder UI with card search, import, validation, export
- [ ] Deck check and the Verified marker (the player's request, 2026-10-18; design in `docs/deck-check.md`): choose or build a deck, run a check that reports legality and every card the game cannot play without manual steps, highlight those cards (icon, word, edge, reasons, unsupported clauses underlined), show a marker on a saved deck whose check is clean, derived from the deck's content hash and the cards' support records so any edit or engine change removes it until the next check; save runs the check automatically, the sideboard counts toward the marker, a deck with manual cards may be played with a count-only note to the table (names show when a card becomes public); live issue counter while building; precon badges
- [ ] Precon browser with one-click play
- [ ] N-player layout, game log, stack panel, stop settings
- [ ] Spectators and reconnect polish: a spectator seat, a face-down card rule in `zone_is_visible`, a "waiting on" message for tables of three or more

## M5 Commander
- [ ] `Commander` format rules: 100-card singleton, color identity, 40 life, command zone, tax, 21 commander damage; `PlayerView`/`ObjectView` carry commander damage and tax so the table's "not tracked yet" slot (`SeatPanel`) fills
- [ ] Fix: when the active player leaves mid-turn the turn continues without an active player (CR 800.4; `docs/engine.md`), with a scenario test
- [ ] Partner, companion, Commander-specific tests and fuzz games with 4-6 players

## M6 Coverage
- [ ] Manual-resolve fallback mode for unimplemented cards
- [ ] Oracle-text compiler for common templates with golden tests Reference first: the phase.rs oracle parser (Rust, MIT or Apache-2.0, `crates/engine/src/parser`) and XMage (MIT); Forge and Magarena are GPL-3.0, so study only and copy nothing. Check each compiled card by rendering the data back to English and comparing it with the oracle text
- [ ] Coverage dashboard over `Library::support` (computed, never stored): states, reasons and clause spans per card, ranked by how many saved decks use it; target: 100% of a chosen set of Commander precons (`docs/deck-check.md`)

## M7 Polish and deploy
- [ ] Dockerfile, Kustomize manifests for k3s, Postgres, ingress + TLS on the user's domain
- [ ] Animations, sounds, replays, solo play against bots
- [ ] Performance profile on real hardware with real card art
