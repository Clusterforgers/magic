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
- [x] Protocol and engine gaps the prompts exposed (one task each, with tests): (a) `MulliganOptions` carries `free_mulligans`, `bottom_if_keep` and `bottom_if_mulligan`, and the prompt reads them; (b) `PayOptions` carries `can_cancel`, `PayChoice::Complete` pays the rest of a partial payment, and `check_payment` rejects two abilities of one permanent and tapped sources; (c) `ManaCost` is a canonical list of `ManaSymbol` with the hybrid, monocolored hybrid, Phyrexian, hybrid Phyrexian, colorless hybrid and colorless Phyrexian symbols (`docs/engine.md`): every Scryfall cost string reads or is refused by name, payment tries each way to pay them, `Payment.life` and the `LifePaid` event pay life for Phyrexian symbols, `PayOptions` carries `life` and `max_life`, and the PayMana prompt has a switch per Phyrexian symbol. The two gaps that wait for M3 features moved to the Keywords and the Tokens, copy effects, X costs items below
- [x] Playwright e2e: full 2-player vanilla game in the browser (`web/e2e`, `pnpm e2e`; command and driver in `docs/web.md`)
- [x] `mtg-server` persistence (a): Postgres `EventStore` and migration table
- [x] `mtg-server` persistence (b): game and seat records (store a serializable setup recipe), resume running games on boot; `sqlx::test` tests
- [ ] Game flow, from the first hands-on test (one task each, with tests): (a) done: `PriorityOptions.can_act` says whether the player can do anything useful (a land to play, a castable spell, an ability that is not a mana ability; tapping a land for mana alone does not count, so after the last land with nothing affordable there is nothing to do), and the server passes for a seat that cannot act (`ClientMessage::AutoPass`, per seat, on by default, `MTG_AUTO_PASS=0` changes the default; `Welcome.auto_pass` reports it; the web switch "Skip turns when I can't act"), so nobody waits on a "nothing to play" prompt; (b) done: response clock: when a seat that is not the active player can act, its priority decision carries `deadline_ms` (default 10 s, `MTG_RESPONSE_SECS`; blocking has 45 s, `MTG_BLOCK_SECS`; every other decision has none), the server answers at the deadline (a pass, or no blocks) through the normal path so a late answer is stale, and the prompt shows the countdown, so one player cannot hold the table; (c) still open: replace the bare "Pass priority" button with contextual controls (Resolve with a spell on the stack, Next step, Attack, End turn) plus "pass until something happens" yields and per-step stops kept per seat on the server
- [x] Combat readability: (a) done: engine `combat_forecast` (who deals what to whom, which creatures die, which players take lethal; keywords join it as they land) carried as `combat_forecast` in the table view while damage is ahead; (b) done except the pending-answer preview, which is the next item: `net/parse.ts` checks the forecast, an SVG overlay (`web/src/features/table/combat`, `docs/web.md`) draws a muted line from each attacker to what it attacks and a strong line from each blocker to its attacker, coloured by the pair's outcome, with a key; doomed creatures look destroyed and creatures that would take damage show "-N"; attacked seats show "life → life after" and a lethal mark; the DeclareBlockers prompt lists the blocks made ("Your Grizzly Bears blocks Bo's Hill Giant", each can be undone) and draws them, and DeclareAttackers draws the attack chosen so far; `/dev/table?prompt=forecast|block`
- [ ] Block preview: Engine preview of a pending DeclareBlockers answer (`ClientMessage::Preview` / `ServerMessage::Forecast`) so the prompt shows who would die while still choosing, replacing the seam `useBlockPreview`
- [x] Table feedback: animations for attacking, blocking, damage, creatures dying, entering permanents and life changes (`features/table/fx`, `docs/web.md`): derived from each batch's before, after and events, ghosts for the dead, a still highlight under `prefers-reduced-motion`, off in vitest, `/dev/table?fx=` and `?slow=` to look at them
- [x] Concede: a button with a confirm step wired to the existing `ClientMessage::Concede`, conceded seats marked in the seat panel and log (`ConcedeButton`; docs/web.md)
- [ ] Lobby shows who has pressed Ready: the server tracks Ready per seat, lists it in the REST lookup and pushes changes, and the lobby shows every seat's state
- [x] Users (pulled forward from M4 Auth), (a) server: users table, username + argon2, session cookie, register/login/logout/me endpoints, games and seats bound to a user, a "my games" listing (running and finished) and a rejoin that needs the session instead of the seat token, with the seat-token link kept for guests; `sqlx::test` tests. Done, see `docs/server.md` (Accounts): `/api/auth/{register,login,logout,me}`, `GET /api/me/games`, `GET /ws/{game}` rejoins a bound seat with the cookie alone, a signed-in creator owns seat 0 and the first signed-in visitor of a link claims its seat. Open for (b): `POST /api/games` takes no choice of the creator's seat (it is seat 0), and the account endpoints answer 503 `accounts_unavailable` without `DATABASE_URL`, so the in-memory e2e server has no sign-in
- [x] Users, (b) web: register and login pages, the signed-in state in the header, a "My games" page that lists games and rejoins a seat, create-game binding the creator's seat to the account. Done, see `docs/web.md` (Accounts): `/login`, `/register` (checked before sending, every error code in words, `next` only as a same-site path), `AccountNav`, `/games` with Rejoin through the session (no View result: the server forgets a finished game ten minutes after it ends, so the row carries the result), the game page joins by `/ws/{game}` without a seat link when signed in, the create page and lobby name the account, and `e2e/accounts.spec.ts` runs with `E2E_DATABASE_URL` (skipped otherwise, so `pnpm e2e` stays hermetic)
- [ ] Resume checks the deck contents: the stored recipe names demo decks only by id, so a changed deck would replay a log under the wrong cards unnoticed; store a content hash (or the deck list) in the recipe and fail the game on mismatch

## M3 Rules depth
- [ ] `mtg-cards` DSL: Cost, Selector, Condition, Effect, Duration, TargetSpec primitives with data files
- [ ] Decision kinds the effects need (one task each across `mtg-engine`, `mtg-protocol` and a web prompt, with tests; today the engine has no targeting or choices): `ChooseTargets`, `ChooseMode`, `ChooseNumber` (X, "any number"), `ChooseCards` from a zone or revealed set (search, reveal, "choose a creature"), `OrderCards` (scry, surveil, put back, bottom in any order), `YesNo` for "may", `OrderTriggers` (same controller), `ChooseReplacement`
- [ ] Keywords, one task each with a table test: flying/reach, first strike/double strike, deathtouch, trample, lifelink, vigilance, haste, menace, defender, indestructible, hexproof/ward. Also here: `DamageOptions` gets player and trample targets, and the views carry keywords so the damage prompt's lethal hint knows deathtouch (`docs/web.md`, AssignDamage)
- [ ] Keyword actions (CR 701), one engine primitive each, table-tested, with a web prompt where the player chooses: draw/discard/mill/shuffle, scry, surveil, fateseal, search, reveal, sacrifice/destroy/exile/counter, create tokens and populate (the "summon" family), fight, explore, proliferate, investigate and other predefined tokens (Clue, Treasure, Food), amass, connive, learn, transform/meld, attach/equip, regenerate, goad, monstrosity, adapt, bolster, support, venture. Arena-only digital actions (conjure, seek, specialize) stay out until asked for
- [ ] Keyword abilities beyond combat (CR 702), one task each: flash, equip/reconfigure, enchant, cycling, kicker/multikicker, flashback, madness, prowess, convoke, delve, cascade, storm, morph/disguise, suspend, evoke, overload, and the Commander-relevant ones (partner, companion, ward is above)
- [ ] Triggered abilities (incl. APNAP ordering, delayed triggers)
- [ ] Replacement and prevention effects
- [ ] Layers: full CR 613 ordering with dependencies, counters, auras, equipment
- [ ] Tokens, copy effects, X costs, alternative and additional costs, modal spells. X costs also need `ManaSymbol::X` in `ManaCost` (the parser refuses `{X}` today, by name), a `ChooseNumber` for the value, and the PayMana prompt and `payment.ts` counting it in the mana value

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
