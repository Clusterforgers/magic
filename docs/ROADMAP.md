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
- [ ] `mtg-db` follow-ups: reap stale `running` import runs, skip NUL-containing records instead of failing the batch, give `mtg-server` its own migration table (`mtg-db` owns `_sqlx_migrations`)
- [ ] CLI binary `mtg-cli`: download Scryfall bulk + MTGJSON deck index/decks, ingest into Postgres (resumable, polite rate limit)
- [ ] Verify Moxfield, Archidekt, MTGO and Arena export parsers against real exports. Verified on real exports of two Commander decks (`real_exports.rs`) and on a corpus of 29 public decks, 183 export files compared with each site's own card lists (`corpus_parse.rs`, `corpus_detect.rs`, `corpus_export.rs`, `docs/data-sources.md`): MTGO text is its own `DeckFormat`, Moxfield's single commander is promoted by `promote_implied_commander`, Moxfield `STICKERS:` is reported in `ParsedDeck::ignored`. Still open: a real MTGO `.dek`, a real Arena client export, companions and custom cards (no public deck found), and resolving the names parsing leaves as written (flavor and digital names, `A / B`, `A/B`, `A /// B`, front faces)
- [ ] Importer 2b, name resolution on the corpus: look up by printing (set + collector number) before name; resolve `A / B`, `A/B`, `A /// B`, front-face-only, flavor and digital (Arena/MTGO) names; real-data test that resolves all 183 corpus exports against Scryfall
- [ ] Card search HTTP endpoints in `mtg-server` (name, filters, color identity, pagination)

## M2 Engine and first playable
- [x] Merge M2a; review against `CLAUDE.md`; engine design notes in `docs/engine.md`
- [x] `mtg-protocol`: Decision/Event/State-view wire types, generated TypeScript bindings, drift test
- [x] `mtg-server` actor (no network): `GameActor` owns an `Engine`, per-seat channels, projected events and decisions, stale-answer and reconnect handling, `EventStore` trait with an in-memory store; bot clients play full games in tests
- [x] `mtg-server` transport: axum binary, create-game REST with per-seat tokens, one WebSocket per seat, built-in demo decks, graceful shutdown; tests with real sockets (in-memory store)
- [x] Protocol sync: every `Events` batch carries the whole table state plus a before/after object diff, so the client never folds engine events; reference `StateView::apply`, per-seat property tests, transcript fixture for the web
- [x] Web core: typed REST + WebSocket client (reconnect, close codes, gap in `from_seq` sends `Resync`), zustand store applying snapshots and batches, tested against the transcript fixture and a fake socket
- [ ] Web game UI: part 1a done (routes, create-game page, connection/lobby/game-over screens, spike removed); part 1b done (read-only N-seat table in `GameTable`: grid and focus layouts, seat panels, battlefields, hand, stack, log, chat, card preview, `/dev/table`); part 2 left: Arena-style prompts for all eight decision kinds, legal-option highlights, "nothing to do" banners, commander damage and tax once the views carry them
- [ ] Playwright e2e: full 2-player vanilla game in the browser
- [ ] `mtg-server` persistence: Postgres `EventStore` with its own migration table, game and seat records (store a serializable setup recipe), resume running games on boot; `sqlx::test` tests

## M3 Rules depth
- [ ] `mtg-cards` DSL: Cost, Selector, Condition, Effect, Duration, TargetSpec primitives with data files
- [ ] Keywords, one task each with a table test: flying/reach, first strike/double strike, deathtouch, trample, lifelink, vigilance, haste, menace, defender, indestructible, hexproof/ward
- [ ] Triggered abilities (incl. APNAP ordering, delayed triggers)
- [ ] Replacement and prevention effects
- [ ] Layers: full CR 613 ordering with dependencies, counters, auras, equipment
- [ ] Tokens, copy effects, X costs, alternative and additional costs, modal spells

## M4 Multiplayer product
- [ ] Auth (username + argon2, session cookie), invite-link lobbies
- [ ] Deck builder UI with card search, import, validation, export
- [ ] Precon browser with one-click play
- [ ] N-player layout, game log, stack panel, stop settings
- [ ] Spectators and reconnect polish

## M5 Commander
- [ ] `Commander` format rules: 100-card singleton, color identity, 40 life, command zone, tax, 21 commander damage
- [ ] Fix: when the active player leaves mid-turn the turn continues without an active player (CR 800.4; `docs/engine.md`), with a scenario test
- [ ] Partner, companion, Commander-specific tests and fuzz games with 4-6 players

## M6 Coverage
- [ ] Manual-resolve fallback mode for unimplemented cards
- [ ] Oracle-text compiler for common templates with golden tests
- [ ] Coverage dashboard ranked by frequency; target: 100% of a chosen set of Commander precons

## M7 Polish and deploy
- [ ] Dockerfile, Kustomize manifests for k3s, Postgres, ingress + TLS on the user's domain
- [ ] Animations, sounds, replays, solo play against bots
- [ ] Performance profile on real hardware with real card art
