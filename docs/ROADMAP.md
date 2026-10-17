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
- [ ] Verify Moxfield, Archidekt, MTGO and Arena export parsers against real exports. Verified on real Moxfield, Archidekt, Arena and MTGO text exports of two Commander decks (`real_exports.rs`, `docs/data-sources.md`): MTGO text is its own `DeckFormat` and Moxfield's commander is promoted by `promote_implied_commander`. Still open: a real MTGO `.dek`, MTGO text without sideboard cards, partner/background commanders, double-faced and split names, accented names, flavor names (Moxfield's `The Ball` is Fellwar Stone), Archidekt sideboard/maybeboard markers, a real Arena client export, 60-card decks with sideboards
- [ ] Card search HTTP endpoints in `mtg-server` (name, filters, color identity, pagination)

## M2 Engine and first playable
- [x] Merge M2a; review against `CLAUDE.md`; engine design notes in `docs/engine.md`
- [x] `mtg-protocol`: Decision/Event/State-view wire types, generated TypeScript bindings, drift test
- [x] `mtg-server` actor (no network): `GameActor` owns an `Engine`, per-seat channels, projected events and decisions, stale-answer and reconnect handling, `EventStore` trait with an in-memory store; bot clients play full games in tests
- [x] `mtg-server` transport: axum binary, create-game REST with per-seat tokens, one WebSocket per seat, built-in demo decks, graceful shutdown; tests with real sockets (in-memory store)
- [x] Protocol sync: every `Events` batch carries the whole table state plus a before/after object diff, so the client never folds engine events; reference `StateView::apply`, per-seat property tests, transcript fixture for the web
- [x] Web core: typed REST + WebSocket client (reconnect, close codes, gap in `from_seq` sends `Resync`), zustand store applying snapshots and batches, tested against the transcript fixture and a fake socket
- [ ] Web game UI: create-game page, N-seat board, hand, stack, log; Arena-style prompts for all eight decision kinds, legal-option highlights, "nothing to do" banners; replaces the spike simulator
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
- [ ] Partner, companion, Commander-specific tests and fuzz games with 4-6 players

## M6 Coverage
- [ ] Manual-resolve fallback mode for unimplemented cards
- [ ] Oracle-text compiler for common templates with golden tests
- [ ] Coverage dashboard ranked by frequency; target: 100% of a chosen set of Commander precons

## M7 Polish and deploy
- [ ] Dockerfile, Kustomize manifests for k3s, Postgres, ingress + TLS on the user's domain
- [ ] Animations, sounds, replays, solo play against bots
- [ ] Performance profile on real hardware with real card art
