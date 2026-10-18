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
- [x] Merge M2a; review against `CLAUDE.md`; engine design notes in `docs/engine.md`
- [x] `mtg-protocol`: Decision/Event/State-view wire types, generated TypeScript bindings, drift test
- [x] `mtg-server` actor (no network): `GameActor` owns an `Engine`, per-seat channels, projected events and decisions, stale-answer and reconnect handling, `EventStore` trait with an in-memory store; bot clients play full games in tests
- [x] `mtg-server` transport: axum binary, create-game REST with per-seat tokens, one WebSocket per seat, built-in demo decks, graceful shutdown; tests with real sockets (in-memory store)
- [x] Protocol sync: every `Events` batch carries the whole table state plus a before/after object diff, so the client never folds engine events; reference `StateView::apply`, per-seat property tests, transcript fixture for the web
- [x] Web core: typed REST + WebSocket client (reconnect, close codes, gap in `from_seq` sends `Resync`), zustand store applying snapshots and batches, tested against the transcript fixture and a fake socket
- [x] Web game UI: routes, create-game page, connection/lobby/game-over screens; read-only N-seat table in `GameTable` (grid and focus layouts, seat panels, battlefields, hand, stack, log, chat, card preview, `/dev/table`); prompts for all eight decision kinds (`features/table/prompt/`, `?prompt=` on `/dev/table`), legal-option highlights and "nothing to do" notes (`docs/web.md`). Commander damage and tax in the table wait for M5
- [ ] Protocol and engine gaps the prompts exposed (one task each, with tests): (a) done: `MulliganOptions` carries `free_mulligans`, `bottom_if_keep` and `bottom_if_mulligan`, and the prompt reads them; (b) payment: `PayOptions` has no cancel flag and there is no "complete my partial payment" answer (`Auto` ignores what was chosen), and `check_payment` accepts two abilities of one permanent, which would tap it twice; (c) `ManaCost` has no hybrid or phyrexian symbols. Two more wait for their M3 features: X in `ManaCost` goes with "X costs", and `DamageOptions` player and trample targets plus keywords in the views (deathtouch in the lethal hint) go with "trample" and "deathtouch"
- [x] Playwright e2e: full 2-player vanilla game in the browser (`web/e2e`, `pnpm e2e`; command and driver in `docs/web.md`)
- [x] `mtg-server` persistence (a): Postgres `EventStore` and migration table
- [ ] `mtg-server` persistence (b): game and seat records (store a serializable setup recipe), resume running games on boot; `sqlx::test` tests
- [ ] Game flow, from the first hands-on test (one task each, with tests): (a) `PriorityOptions` says whether the player can do anything useful (a land to play, a castable spell, an ability that is not a mana ability; tapping a land for mana alone does not count, so after the last land with nothing affordable there is nothing to do) and the server passes for a seat that cannot act (per-seat setting, on by default), so nobody waits on a "nothing to play" prompt; (b) response clock: when a seat that is not the active player can act, its decision carries a deadline (default 10 s, `MTG_RESPONSE_SECS`), the server passes at the deadline and the web shows the countdown, so one player cannot hold the table; (c) replace the bare "Pass priority" button with contextual controls (Resolve with a spell on the stack, Next step, Attack, End turn) plus "pass until something happens" yields and per-step stops kept per seat on the server
- [ ] Combat readability: (a) engine `combat_forecast` (who deals what to whom, which creatures die; keywords join it as they land) carried in the view during blocks; (b) web: lines from each attacker to its blockers, the forecast drawn on the cards (a doomed creature looks destroyed), declared-block state clear before confirming
- [ ] Table feedback: animations for attacking, blocking, creatures dying and a player taking damage (driven by the event diff, off under `prefers-reduced-motion`); a pulsing "your turn" cue on the table and in the tab title so nobody misses their turn
- [ ] Concede: a button with a confirm step wired to the existing `ClientMessage::Concede`, conceded seats marked in the seat panel and log
- [ ] Lobby shows who has pressed Ready: the server tracks Ready per seat, lists it in the REST lookup and pushes changes, and the lobby shows every seat's state
- [ ] Users (pulled forward from M4 Auth; needs persistence (b)): users table, username + argon2, session cookie, register/login/logout/me endpoints and pages; games and seats bound to a user; "My games" lists running and finished games and rejoins a seat by user, with the seat-token link kept for guests; `sqlx::test` tests

## M3 Rules depth
- [ ] `mtg-cards` DSL: Cost, Selector, Condition, Effect, Duration, TargetSpec primitives with data files
- [ ] Decision kinds the effects need (one task each across `mtg-engine`, `mtg-protocol` and a web prompt, with tests; today the engine has no targeting or choices): `ChooseTargets`, `ChooseMode`, `ChooseNumber` (X, "any number"), `ChooseCards` from a zone or revealed set (search, reveal, "choose a creature"), `OrderCards` (scry, surveil, put back, bottom in any order), `YesNo` for "may", `OrderTriggers` (same controller), `ChooseReplacement`
- [ ] Keywords, one task each with a table test: flying/reach, first strike/double strike, deathtouch, trample, lifelink, vigilance, haste, menace, defender, indestructible, hexproof/ward
- [ ] Keyword actions (CR 701), one engine primitive each, table-tested, with a web prompt where the player chooses: draw/discard/mill/shuffle, scry, surveil, fateseal, search, reveal, sacrifice/destroy/exile/counter, create tokens and populate (the "summon" family), fight, explore, proliferate, investigate and other predefined tokens (Clue, Treasure, Food), amass, connive, learn, transform/meld, attach/equip, regenerate, goad, monstrosity, adapt, bolster, support, venture. Arena-only digital actions (conjure, seek, specialize) stay out until asked for
- [ ] Keyword abilities beyond combat (CR 702), one task each: flash, equip/reconfigure, enchant, cycling, kicker/multikicker, flashback, madness, prowess, convoke, delve, cascade, storm, morph/disguise, suspend, evoke, overload, and the Commander-relevant ones (partner, companion, ward is above)
- [ ] Triggered abilities (incl. APNAP ordering, delayed triggers)
- [ ] Replacement and prevention effects
- [ ] Layers: full CR 613 ordering with dependencies, counters, auras, equipment
- [ ] Tokens, copy effects, X costs, alternative and additional costs, modal spells

## M4 Multiplayer product
- [ ] Invite-link lobbies on top of users (accounts land in M2)
- [ ] Deck builder UI with card search, import, validation, export
- [ ] Precon browser with one-click play
- [ ] N-player layout, game log, stack panel, stop settings
- [ ] Spectators and reconnect polish

## M5 Commander
- [ ] `Commander` format rules: 100-card singleton, color identity, 40 life, command zone, tax, 21 commander damage; `PlayerView`/`ObjectView` carry commander damage and tax so the table's "not tracked yet" slot (`SeatPanel`) fills
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
