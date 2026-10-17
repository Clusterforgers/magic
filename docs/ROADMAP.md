# Roadmap

Single source of truth for progress. Any fresh session continues with: "read `docs/ROADMAP.md` and do the next unchecked item".
Rules: one item = one focused piece of work (about 15 minutes for an agent). Tick the box in the same commit that finishes the item. Design reference: `docs/PLAN.md`.

## Done
- [x] M0 Workspace, lint rules, CI, `CLAUDE.md`, local Postgres compose
- [x] M0 Frontend spike: React stack locked in (`docs/spike-react.md`)
- [x] M1a `mtg-import`: Scryfall (array + JSONL), MTGJSON decks and index, deck-list parsers, name resolution; validated on real data (`docs/data-sources.md`)

## In flight (check `git worktree list` and `git log` before touching)
- [x] M1b `mtg-db`: schema, upserts, card search, precon storage (merged)
- [x] M2a Engine core + `mtg-testkit`: zones, turns, priority, stack, combat, layers skeleton, bots (merged)

## M1 Card data (rest)
- [ ] `mtg-db` follow-ups: reap stale `running` import runs, skip NUL-containing records instead of failing the batch, give `mtg-server` its own migration table (`mtg-db` owns `_sqlx_migrations`)
- [ ] CLI binary `mtg-cli`: download Scryfall bulk + MTGJSON deck index/decks, ingest into Postgres (resumable, polite rate limit)
- [ ] Verify Moxfield, Archidekt, MTGO and Arena export parsers against real exports (needs samples from the user)
- [ ] Card search HTTP endpoints in `mtg-server` (name, filters, color identity, pagination)

## M2 Engine and first playable
- [x] Merge M2a; review against `CLAUDE.md`; engine design notes in `docs/engine.md`
- [x] `mtg-protocol`: Decision/Event/State-view wire types, generated TypeScript bindings, drift test
- [ ] `mtg-server`: game actor, WebSocket, event log persisted, reconnect
- [ ] Web: replace spike simulator with live game state; Arena-style priority/attack/block prompts and "nothing to do" banners
- [ ] Playwright e2e: full 2-player vanilla game in the browser

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
