# Roadmap

What is open and what comes next. Continue with the first open piece of "Live game", then "Next up".

One item is one piece of about 15 minutes and one line: the outcome, the cards it frees and a few examples. The commit that finishes it deletes its line, turns a leftover into a new lettered item and names a new area under "Done"; designs are in `docs/`. Open card, compiler and wording items are in `docs/cards-backlog.md` under the same rules.

## Status

- The game is live at https://magic.hivemindcloud.dk (k3s; `docs/deploy.md`); `scripts/release.sh` releases a landed batch.
- Done: foundations, card data, engine core, protocol and server, game flow, Commander (the companion conditions of m4c aside), the web table with prompts for every decision kind, lobby, deck builder and precon browser, manual play, deployment.
- In progress: M3 card system and M6 coverage. A card the compiler does not read whole is played through by-hand clauses. The owner's Moxfield deck is fully Verified. Run `mtg-cli compile` and `coverage` for the current numbers before quoting any.
- Open: spectators, handicaps, an off-queen copy of the backups (M7), sounds, replays and solo play (M8).

## Order of work

- Structure before additions: work that changes how abilities, characteristics, events, costs or views are shaped runs first and alone; work that fills one stub (a keyword, an effect) follows in any order.
- Rules data leads the engine: support is computed, so a compiled card is Partial until the engine runs its primitives and then Verified without an edit. The gap ledger (`mtg-cli check-rules --gaps`) and `coverage --all` rank what holds cards back by card popularity.
- A cluster ends with its corpus cards Verified, the compiler templates and goldens for its wordings, a fuzz deck under every invariant, hidden-information tests for its events and current generated bindings. A new event, decision or view field comes with its web support in the same piece.
- A piece is claimed before it starts: its `[ ]` becomes `[~]` with the branch name and the commit is pushed to origin; the commit that finishes it deletes the line and is pushed too. Local `main` is pushed after every merge.

## Next up

- Live game first: the cards friends marked (`mtg-cli fix-marks`) and the games that ended in `failed` outrank everything (`docs/deploy.md`, Operations).
- Cards (`docs/cards-backlog.md`): the local model proposes rules (`mtg-cli propose` on the owner's GPU, `--solved-examples 5`, in popularity order), the checker judges, agents build the missing writers and engine pieces from the top of the gap ledger, and the passing proposals are promoted into the corpus (`docs/cards-dsl.md`, Model-assisted reading). Once rt1f lands, agents draft the missing cards of one precon each for the read tier (`docs/cards-dsl.md`, Drafts).
- Table and prompts: the open items under "M5 Table and prompts", one Opus validator walk per batch.
- Tooling: (dep) dependencies, alone between rounds.

## Done

- Foundations: workspace, lint rules, CI, `CLAUDE.md`, local Postgres and the React stack (`docs/spike-react.md`).
- Card data: Scryfall and MTGJSON import, deck-list parsers and exporters, the schema with card search and precons, and `mtg-cli` (fetch, ingest, migrate, compile, coverage, fix-marks, promote).
- Engine: zones, turns, priority, stack, combat with forecast, layers (CR 613), replacement and prevention, counters, planeswalkers, battles and Sieges, attachments, copies and faces, Rooms, soulbond, mutate, phasing, day and night, vehicles, the Monarch and the initiative, bots, `mtg-testkit` (`docs/engine.md`).
- Card language and compiler: the DSL, the reader framework, the renderer with goldens, the corpus ratchet, templates for every ability shape and stored `card_rules` (`docs/cards-dsl.md`).
- Model-assisted reading: `check-rules`, `propose` (a local OpenAI-style server), `promote` (`--draft` writes the 355 first read-tier drafts, which `compile` stores in `card_drafts`), the repair of mechanical slips, writers for the accepted wordings, `coverage --all`, the gap ledger and `export-pairs` (`docs/cards-dsl.md`).
- Protocol and server: generated bindings with a drift test, the game actor, a WebSocket per seat, persistence and resume, accounts (the admin makes, resets and removes them), lobbies, open games, precons, concurrent keep and mulligan, seat presence, the snapshot log (`docs/protocol.md`, `docs/server.md`).
- Game flow: auto-pass, response clocks, yields, stops, Full Control, the timer bank, the by-hand window and leak tests (`docs/arena-flow.md`).
- Commander: house rules, format rules, deck check, Partner, companion in the engine, tax, commander damage and any number of seats (`docs/server.md`).
- Manual play: the Commands panel, by-hand clauses with chips, manual actions at any time on any turn and fix marks (`docs/web.md`, `docs/protocol.md`).
- Web table: lanes and card sizes, the seat corner, click combat, the mana strip, legible play (flights, spotlight, target arrows, marks), Equipment tucked at its host with its grants, hand reasons, fit at six sizes and a 12 px text floor (`docs/web.md`, `docs/ui-design.md`).
- Lobby, decks and precons: invites, the deck builder with pictures and stats, import and export, the deck check, the not-validated list and the precon browser (`docs/web.md`).
- Card images: a server cache with a warm-up, and pictures in the table, builder and preview (`docs/server.md`, `docs/web.md`).
- Testing: scenarios, invariants and fuzz decks, `scripts/check.sh` with the `affected` mode, and Playwright specs including the four-seat Commander game with precons or the owner's deck (`E2E_DECK=owner`).
- Deployment: two amd64 images, the Helm chart and ArgoCD app in `k3s-cluster-manifests`, `scripts/release.sh` (build, push, roll out, roll back) and a nightly `pg_dump` onto its own volume (`docs/deploy.md`).
- Reference engines: Forge, XMage and Magarena surveyed (`docs/reference-engines.md`).

## Live game

Items that touch the running game come first.

- [~] (v94a) (card-zoom) The highlighted card (under the pointer or selected, the spotlight or preview) is drawn larger so its text reads, without covering prompts or controls, at 1440x900 and on a phone (`docs/web.md`)
- [ ] (ad1c) The Accounts page in the layout gate at the six sizes in both themes (a dev route that stands for it) and an e2e spec of the admin flow on the Postgres server: sign-in, make an account, the friend signs in, the closed sign-up page (`docs/web.md`)
- [ ] (d1c) A copy of the nightly database dumps off queen (a Longhorn backup target or a scheduled copy to another machine), so losing queen does not lose the accounts and decks (`docs/deploy.md`)
- [ ] (d1b) The release script's steps as a GitHub Actions workflow on a published release, so a release needs no dev machine (`docs/deploy.md`)
- [ ] (v92l) Keyboard-only walk of a whole turn: focus order, the focus return after H and Escape across a resize, contrast of the by-hand and unplayable marks in both themes
- [ ] (v92m) Duel walk by hand of a counterspell, a combat trick and a sacrifice, and of the timer bank as the other seat sees it (needs a seat that has earned an extension)
- [ ] (v91k) The house rule key `colour_identity` is spelled `color_identity` on the wire, in the stored rules and in `mtg-core` `house.rs`, with the stored games migrated
- [ ] (w2j) An attach line of a rebuilt log has no old host: `Snapshot` could carry the host each attach event of its window replaced (its name in `names`), so the line reads "moves from A to B" as it did live (`play/store.ts` `mergeHistory`, `hosts.ts`)
- [ ] (fx1b) A cap on the fix marks of one account and a rate limit on the fix-mark routes
- [ ] (f1b) Playtest polish of the table, builder and lobby against the UI wishes (fits the viewport, clear turn indicators, open-games list, card images at every size), with `/dev/*` screenshots at six sizes in both themes, so the first real games can start
- [ ] (k3e) A legal special action the web cannot take (suspend, foretell, turn a face-down permanent up, cast a prepared copy) leaves its seat asked at every default window with nothing marked: mark the card and offer the action in `abilityPlays`
- [ ] (pl2l) `join.spec.ts` "a guest follows an invite link" timed out once on the Join button in a full run and never in 50 solo runs: a recurrence needs a Playwright trace (`retain-on-failure`) to name the cause

## M4 Commander

- [ ] (m4c) Companion conditions that need more than the card (mana value parity, an activated ability, a shared type, one mana symbol) in rules, engine and deck check; frees about 4 cards (Jegantha, Kaheera, Gyruda)

## M5 Table and prompts

The table and its prompts are live; the items below polish them.

- [ ] The companion in the web: a seat-panel slot from `PlayerView.companion`, `BringCompanion` in the priority menu and the deck builder's companion issue
- [ ] (i2) Arena flow P10: smart stops, per-ability yields, an order-by-timestamp preference, damage auto-assign and the answer to a one-option decision (`docs/arena-flow.md`)
- [ ] (w3k) A replacement candidate carries the words of its own effect, so the ChooseReplacement prompt shows the one ability that applies instead of the whole card text (`ReplacementCandidate`)
- [ ] (w3j) A scry is one screen: the engine asks the split and the order of both piles in one decision, so the player orders the top pile where the cards are split instead of in the two `OrderCards` that follow
- [ ] (w3f) The payment prompt spends restricted mana by hand, read against `PayOptions.spend`
- [ ] (w3g) The payment prompt taps creatures for convoke and artifacts for improvise
- [ ] (w3h) The payment prompt exiles graveyard cards for delve
- [ ] (w3l) `decisionName` (`turnText.ts`) names a decision kind the client does not know as Discard, so the waiting label and the clock words of a newer server's decision read wrong
- [ ] Clock words in the web (`clockText.ts`) for every decision with a clock the web does not name yet
- [ ] Attack and block prompts lock the creatures a rule requires, and a seat panel shows a revealed library top (`PlayerView.revealed_top`)
- [ ] The forecast and block preview mark numbers that depend on a choice (`CombatForecast.depends_on_choice`)
- [ ] (a2c) The support mark on the table tells Partial from Manual: `ObjectView` needs the card's support state (a view field), since manual clauses alone do not say which
- [ ] (a2g) A token lookup that finds several equal records prefers the one with the most printings (`token_pictures` returns the printing count) instead of the smallest oracle id
- [ ] (a2e) The precon details dialog shows each card's picture through `CardPicture` and the hover picture, as the builder does
- [ ] (a2f) The real database holds every precon printing (`fetch scryfall --printings`), so the table draws each card in its precon art instead of the card's default picture
- [ ] (k1b) The one-line pods of three and four seats draw each commander (both of a partner pair), the graveyard and the exile as small pictures beside their chips, and pass the layout gate
- [ ] (k1g) The layout gate and the fit spec also sweep 360, 768, 1400 and 1600x1000 (only the six design sizes today)
- [ ] (k1h) A tall prompt sheet on a phone (`commander-return` at 360x740, a 182 px overhang) keeps the opponents' tile strip and board in view: the sheet's height is held to what leaves them room
- [ ] (k2a) `/dev/builder?deck=...` shows the commander slot (the fixture check knows commander pairs) and screenshots at 1400 and 390 settle the slot's height and the identity switch
- [ ] (k2b) The lobby's deck chooser and the precon rows show each Commander deck's commanders as a picture and a name, as the deck list does
- [ ] (k2c) Playwright: build a Commander deck with a Partner pair, see a refused commander with its reason and the identity filter
- [ ] (ts3) `groupPermanents` keeps apart permanents that differ in copy (and of what), shown face, face down, merge, prepared, level and the player an Aura enchants, so a copy never hides inside a stack of its original's tokens
- [ ] (ts4) A phased-out permanent is drawn ghosted with a "Phased out" mark: `PermanentView` needs a `phased_out` field
- [ ] (ts5) A token's printed power and toughness reach the view (with `a2d`), so a buffed or debuffed token is tinted green or red as a card is; today a token's real numbers show plain
- [ ] (ts6) `/dev/table?states=1` screenshots at the six sizes in both themes settle the marks on compact faces (under 76 px, where an attachment mark is an icon) and on tapped pictures
- [ ] (pl1e) The opponent's pod column draws its corner as pictures at 1366x768 and below too: the hand and library chips join the name row, so the column leaves the 62 px a 44 px picture needs (it keeps its chips and scrolls there today)
- [ ] (pl1d) A permanent that dies as the last of its lane keeps its ghost while the effect plays (an empty lane has no place in the fit)
- [ ] (pl2a) The forecast lists the life an attacker's controller gains (lifelink of an attacker) so its pod shows the "+N" chip too (`PlayerForecast` covers attacked seats only)
- [ ] (pl2b) The tiles of focus mode and of the phone's strip show the life chip and take the defender click (only the pod does today)
- [ ] (pl2c) A pending block's arrow takes the colour of the preview's outcome for its pair, as a declared block does
- [ ] (pl2d) Playwright: attack and block by clicks in a real two-seat game and at four seats (the click flows are tested on the dev table only)
- [ ] (pl3a) `ObjectView` carries what a mana ability gives as data (mana, count, cost, sacrifice), so the corner strip and the preview stop reading the words of `activated` (an ability with a count by a condition, a chosen colour or an extra cost shows nothing today)
- [ ] (pl3b) The payment sheet takes the mana sources and the made total in its compact form: one scroll line of source chips, made and owed on one row, no extra height at 390 px
- [ ] (pl3c) The colour switch sits on the source chip and swaps the colour of a chosen source without taking it back; chips mark restricted mana (`only`), life, energy and taken permanents, and the pool marks the mana that outlasts the step (`kept`)
- [ ] (ui1i) `/dev/table?fx=` screenshots of every moment at the six sizes in both themes settle the look, and `fit.spec.ts` holds the table at rest with three targeting items on the stack
- [ ] (ui1e2) An ability's end (`resolved`, `countered`, `fizzled`) shows on its stack chip before it leaves, and the arrow of a counter cast in the same batch starts at its spotlight (`fx/marks.ts`, `fx/effectArrows.ts`, `StackChips.tsx`)
- [ ] (eq1g) An Equipment that had no host draws its equip arrow from where it lay, measured before the batch is drawn (`fx/FxTravel.tsx`, `combat/attachLines.ts`)
- [ ] (st1a) The Stasis family (Stasis Field, Stasis Cell, Stunning Strike) is measured in a scratch database after `compile`: each becomes Verified or names the clause the checker still lacks, with its Scryfall record in the corpus
- [ ] (st1b) "Its activated abilities can't be activated" (Arrest, Stasis Cocoon) is a restriction the compiler reads and the activation rules enforce (`Modification` and the activation check)
- [ ] (eq1j) `gives` of an Aura on a player names the rules it gives outside keywords ("Enchanted player can't cast spells", skipped draws), which the layers do not record for players (`gives.rs`, `layers.rs` `PlayerHeld`)
- [ ] (dk1b2a) Sieges in the compiler and checker: readers and writers for the Aura/God/Demigod search, "lifelink and indestructible as long as you control three other enchantments" and "this Siege", so Invasion of Theros joins `reproduced.txt` and the other Sieges read
- [ ] (dk1b2b) Battles in the protocol and web: an attack target that names a battle (`AttackTarget::Planeswalker` does today), battle labels, defense counters and the protector on the Siege, and a hidden-information test of the Invasion search
- [ ] (dk1c4a) The soulbond cards the compiler still leaves out: Donna Noble (a damage trigger), Joint Assault ("if it's paired"), Doom Weaver, Imperious Mindbreaker, Breathkeeper Seraph, Mirage Phalanx (quoted grants with their own amount), Flowering Lumberknot, Tarelily, Azra Matchthrower
- [ ] (dk1c5c) Rooms: lock a door (Marina Vendrell, Keys to the House), a token copy of a half (Blu) and the copiable door state (CR 707.2)
- [ ] (dk1c5d) Rooms on the web: the log line names the door unlocked ("Alice unlocks Grand Entryway"), and the preview of a Room marks a door the viewer can unlock now (`PriorityOptions.unlockable`, `CardPreview`)
- [ ] (dk1d2a) Orzhov Charm: the reader of "Return target creature you control and all Auras you control attached to it to their owner's hand." (the Auras then the creature) and "Destroy target creature and you lose life equal to its toughness.", so the compiler makes the card without its rules file
- [ ] (lb1a) The precon list item carries the commanders' cards (picture, colour identity), so a precon row in the lobby chooser shows its commanders and the chooser filters precons by colour identity
- [ ] (pn1a) Many-seat tests of the actor for response clocks, timer banks and hidden information per viewer at 13 to 64 seats (the actor tests cover 2 to 4)
- [ ] (pn1b) `fit.spec.ts` and `/dev/table` screenshots at 16, 24 and 40 seats in both themes, and the e2e create flow typing a count above 12
- [ ] (pn1c) The seat field builds a row per seat as it is typed: guard a count that would freeze the page (the server's ceiling is the 256 KiB create body, about 2,500 seats)
- [ ] (fx1c) `FixMarkButton` is mounted in the lobby deck view (the large card of the shared deck preview) and in the table card menu, for a card that is not Verified
- [ ] (db1a) A deck entry can be marked "play by hand" by its owner (also for a Verified card), so a card that misbehaves is fixed at the table: a flag on `DeckLineBody` and the stored line, the engine hosting the card as manual, the check counting it as played by hand
- [ ] (db1b) The builder's cards played by hand quote the words of the card each reason covers (the spans of `useCardTexts`), as the not-validated page does
- [ ] (db1c) At 1440 px and wider the stats and the cards played by hand sit in a rail beside the deck (the builder takes a page width beyond `wide`) instead of two closed disclosures
- [ ] (pm1a) A server test drives a clocked game and shows that a manual action keeps the deadline of a changed decision of the same seat (`carried_clock`), and that a manual action sent while a replacement question holds a batch is applied once it is answered (`flush_deferred`).
- [ ] The turn indicator and step bar show a turn without an active player (`TurnView.active_left`), with `/dev/table` screenshots at six sizes in both themes
- [ ] (ma7q) The Tokens row's custom creature also takes colours and types (artifact, enchantment, subtypes), which `CreateToken` already carries, and a token can copy a card on the table

- [ ] (ma6a) A card on the stack (`StackPanel`, `StackChips`) carries the by-hand mark, as a permanent does
- [ ] (ma6b) Playwright: the by-hand mark opens the clause list by a press and the `h` key opens it on the focused card, for a Partial card on a two-seat table at desktop and phone size, and the cast and entering lines are in both logs
- [ ] (U5b) One step ladder: the ladder that shows the turn also sets the stops (click a step), as in Arena, and the second row of step buttons goes
- [ ] Playwright games with corpus decks that ask every decision kind
- [ ] (c5h) The table's event text has a line for the rule `MustAttackPlayers` (Encore) instead of the generic trigger line
- [ ] (c5i) The attack prompt offers the exert of a creature that may be exerted (`AttackerOption.exert`, `Answer::DeclareExerting`) and the table marks an exerted permanent

## Cards

Open card, compiler and wording items: `docs/cards-backlog.md`.

- [ ] Target: every stored card Verified, or as near as the rules allow; after each round `compile` and `coverage` are re-run and the numbers go on the Status line
- [ ] (rt1ea) The by-hand window after `AbilityLeftToTable`: the server holds the controller of the ability at every priority window until it passes and the Priority prompt reads "Apply the card's text by hand", as for a card with clauses left to the table; frees the table to apply a failed read ability (`mtg-server` stops, `web/src/play/byHand.ts`, `docs/server.md`, `docs/web.md`)
- [ ] (g2a) Plan (Opus design, after ts1) for every Commander-legal card to run automatically within about two to three weekly budgets: cost per card of each route (templates, local model on the idle GPU, a model trained on the Verified pairs, agents), a tier for unread drafts that run marked with fail-soft and a first-use read, the order by popularity and the yield to expect; written into `docs/cards-dsl.md`
- [~] (fg1) (forge-translate) `mtg-cli translate-forge <DIR>` maps the Forge scripts of a read-only checkout (the top 30 effect APIs, the 10 trigger modes, keywords, costs) onto our rules as proposals in the `propose` format, so `check-rules` proves them (Verified) or they become read-tier drafts; the local model only gets what no mapper covers; needs the owner's yes for the checkout and the licence stance (`docs/reference-engines.md`)
- [ ] (fg2) Audit before promoting translated cards: read the review sheets of 100 random translated cards the checker calls Verified and count the wrong ones (an Opus or Sonnet reader, sheet beside the printed text); promote in bulk only when the error rate is low, and show "from Forge" in the review dialog of translated drafts (`meta/<slug>.json` `source`)
- [ ] (fg3) Translator round 2, general first and precons first: restrict `translate-forge` to the 6,429 precon cards and rank their gaps; replace case-by-case mappers with general parsers for Forge's valid-string selectors, `Defined$` words, `Count$` amounts and `ChangeType$`, so one parser frees a whole family; then the leftover single constructs by cards freed; report precon Verified before and after (`crates/mtg-cli/src/translate/`, `docs/cards-dsl.md`)
- [ ] (rt1g) The server loads the current drafts with their read state into the library, rebuilds it on `DraftRepo::version`, pins `reads` in the recipe so resume and replay build the same cards, and records its own `wrong` on `AbilityLeftToTable`; frees approved cards in games (`mtg-server` cards, registry; after rt1a, rt1b and rt1e)
- [ ] (rt1h) `GET /api/reads`, `GET` and `PUT /api/reads/{oracle_id}` with the rate limit, the fix mark of a `wrong`, `stale_read`, account deletion, `CardDetail.draft` and the wire types; frees reading from any client (`mtg-server` reads.rs, `mtg-protocol` rest; after rt1c and rt1g)
- [ ] (rt1j) The review screen: `/read?deck=` and `?precon=`, `ReadDialog` from the table's card preview, keys C and W, the required note, the 2 s delay, `stale_read`, a dev route and the layout gate at six sizes (`web/src/features/reads`; `docs/web.md`, Reading cards; after rt1h and rt1i)
- [ ] (rt1o) `ToRead` becomes a `Link` to the review screen (`/read?deck=` or `?precon=`, from `data-read-path`) in the builder check bar, decks list, lobby rows and preview, precon rows and details and the not-validated list; frees the ways in (`web/src/features/decks/ToRead.tsx`; after rt1j)
- [ ] (rt1p) The dev fixtures of the builder, lobby, precons and not-validated list show a deck on reads and one with drafts waiting, and the layout gate screenshots them in both themes at 390x844 and 1440x900 (`web/src/features/*/dev`, `web/e2e`; after rt1i)
- [ ] (rt1l) Validator walk (Opus): read a precon's queue on screen at 1400 and 390 px, judge one card wrong, start a game and see the approved cards run and the wrong one played by hand; defects become lettered items (after rt1f and rt1j)
- [ ] (t14c) A/B of `--forge-hint` against `--solved-examples` on the same 100 cards: which hint the checker accepts more often; the result goes into `docs/cards-dsl.md`
- [ ] (rt1m) The checker finishes on every proposal: `check` does not return on `pop2/rejected/tempt_with_vengeance.json` (a Spell with `RepeatEach` nested ten deep, the renderer writes every wording of each level), so a nested or very long effect list gets a bounded set of wordings and the verdict `Invalid` or `Unchecked`; then `NEVER_FINISH` in `tests/it/read_sheet.rs` goes (`crates/mtg-cards` render, proposal; no dependency)
- [ ] (rt1n) A 4-seat bot game with 16 copies of Exotic Orchard takes 94 s for 2,635 answers (about 40 ms an answer, against 0.1 ms usual), so the smoke gate stops it at its 20 s limit; find the slow step of its mana ability and make it fast (`crates/mtg-engine` mana; no dependency)
- [ ] (rt1q) The 22 proposals the smoke gate refuses (8 engine errors: `Effect.AddMana.OfTapped.Stack` is in `Caps` but not resolved, a payment picked among objects, a random pick among hidden cards, a `land` bound twice, an object missing from the library; 14 never seen: counterspells and costs above the library's lands) run once the engine or the gate is fixed, from the sweeps' rejected proposals through `promote --draft` (`crates/mtg-engine`, `crates/mtg-testkit` smoke; no dependency)
- [ ] Gate: every corpus card is Verified or played through manual clauses and actions that show each unread clause, and fuzz games with a deck from each cluster pass every invariant, and a four-seat Commander game plays from the lobby to a winner in the web

## Tooling

- [ ] (ts1) Deep dive (Opus design first): a check and a release take an hour, the target is a few minutes for a normal change; measure where the time goes (the corpus binary of about 17,000 per-card tests takes 650 s, a protocol change selects the whole workspace, the image builds follow the check), then re-test only what a change can break (results cached by card fingerprint and crate hash, sharded corpus tests, a fast gate before the full run before a tag, images built while the check runs, one shared test build across worktrees) (`scripts/check.sh`, `scripts/release.sh`, `docs/testing`)
- [ ] (dep) Cargo and web dependencies are current; the piece runs alone between rounds (`docs/agents.md`)
- [ ] (t5a) The Postgres lock covers only the `sqlx::test` tests: the other tests of `mtg-server` and `mtg-cli` run outside it (one full check: 398 s under the lock, 1,087 s waiting for it)
- [ ] (t5b) The `mtg-testkit` `it` binary needs fewer test-seconds: over 90% is `Engine::apply` (`priority_options` 36%, `triggers::detect` 24%, `layers::compute` 14%, whole-object clones), so the next cut is in the engine; opt-level 3 for `mtg-cards` and mimalloc save about 5% and slow every rebuild, so they are not adopted
- [ ] (t1) The `mtg-cli` `fetch_scryfall` tests are stable under parallel load
- [ ] (t3c) DSL amounts for what an earlier effect did and for the mana value a cost took, and a partner by creature type (Foul-Tongue Shriek, Nahiri's Sacrifice, The Knight of Land Drops)
- [ ] The expected X symbols in `x_cards.rs` of `mtg-testkit` are derived from the card list, so a new X card edits no test
- [ ] Coverage dashboard in the web over `Library::support`: states, reasons and clause spans per card, ranked by saved-deck demand (`docs/deck-check.md`)

## M7 Product

- [ ] (w2d) Handicaps (life and opening hand per seat) on the create form's house rules
- [ ] (w2g) `precon-game.spec.ts --repeat-each=6` plays out every time; a two-seat table that casts no spell in nine turns keeps its trail for the cause
- [ ] (f1c) The deck report carries the quoted span with each reason (`mtg-cards` `Reason`), so the not-validated list and the precon details read no card per flagged card from `GET /api/cards/{oracle_id}` (`docs/web.md`, the not-validated list)
- [ ] (f1d) The builder links its check bar to `/decks/$deckId/unvalidated` (`docs/web.md`)
- [ ] The importer fixtures of `mtg-import` hold real records instead of invented oracle ids and printings; synthetic edge cases stay marked as synthetic
- [ ] Verify the MTGO and Arena export parsers against real exports, with companions and custom cards (`docs/data-sources.md`)
- [ ] (v1f) A spectator's connection in the server: a socket that holds no seat, is sent `StateView::for_spectator` and the spectator's batches, and a rule for who may watch
- [ ] Spectators in the web and a "waiting on" message for tables of three or more

## M8 Polish and deploy

- [ ] Animations beyond the table feedback (`docs/web.md`), sounds, replays, solo play against bots
- [ ] Performance profile on real hardware with real card art
- [ ] (p1a) Block options test evasion once per attacker and per blocker (0.4 s at 1,500 against 1,500 today), and a death re-derives the layers only when an effect reads it
