# Roadmap

What is open and what comes next. Continue with the first open piece of "Live game", then "Next up".

One item is one piece of about 15 minutes and one line: the outcome, the cards it frees and a few examples. The commit that finishes it deletes its line, turns a leftover into a new lettered item and names a new area under "Done"; designs are in `docs/`. Open card, compiler and wording items are in `docs/cards-backlog.md` under the same rules.

## Status

- The game is live at https://magic.hivemindcloud.dk (k3s; `docs/deploy.md`); `scripts/release.sh` releases a landed batch.
- Live: foundations, card data, engine core, protocol and server, game flow, Commander (the companion conditions of m4c aside), the web table with prompts for every decision kind, lobby, deck builder and precon browser, manual play, card review, tutorials and the Learn page, deployment.
- Cards: 94.7% of the 32,116 Commander cards play automatically (Verified 22,622, Read 4,863, Unverified 2,935); Partial (973) and Manual (723) cards play through by-hand clauses. Run `mtg-cli compile` and `coverage` before quoting newer numbers.
- In progress: team Commander (tm1), tournaments (tm2), spectators (v1f), handicaps (w2d), replays, bots and sounds, refusal explanations, tutorial fixes and the card coverage of M3 and M6.
- Open: an off-queen copy of the backups (M7).

## Order of work

- Structure before additions: work that changes how abilities, characteristics, events, costs or views are shaped runs first and alone; work that fills one stub (a keyword, an effect) follows in any order.
- Rules data leads the engine: support is computed, so a compiled card is Partial until the engine runs its primitives and then Verified without an edit. The gap ledger (`mtg-cli check-rules --gaps`) and `coverage --all` rank what holds cards back by card popularity.
- A cluster ends with its corpus cards Verified, the compiler templates and goldens for its wordings, a fuzz deck under every invariant, hidden-information tests for its events and current generated bindings. A new event, decision or view field comes with its web support in the same piece.
- A piece is claimed before it starts: its `[ ]` becomes `[~]` with the branch name and the commit is pushed to origin; the commit that finishes it deletes the line and is pushed too. Local `main` is pushed after every merge.

## Next up

- Live game first: the cards friends marked (`mtg-cli fix-marks`) and the games that ended in `failed` outrank everything (`docs/deploy.md`, Operations).
- Cards (`docs/cards-backlog.md`): the Forge translator drafts rules automatically (`mtg-cli translate-forge`, `promote --draft --source forge`, `compile --all`), the checker judges, and agents build the missing readers, writers and engine pieces from the top of the gap ledger.
- Table and prompts: the open items under "M5 Table and prompts", one Opus validator walk per batch.
- Tooling: (dep) dependencies, alone between rounds.

## Done

- Foundations: workspace, lint rules, CI, `CLAUDE.md`, local Postgres and the React stack.
- Card data: Scryfall and MTGJSON import, deck-list parsers and exporters, the schema with card search and precons, and `mtg-cli` (fetch, ingest, migrate, compile, coverage, fix-marks, promote).
- Engine: zones, turns, priority, stack, combat with forecast, layers (CR 613), replacement and prevention, counters, planeswalkers, battles and Sieges, attachments, copies and faces, Rooms, soulbond, mutate, phasing, day and night, vehicles, the Monarch, the initiative and speed, Attractions and tickets, banding, splice, coin flips with Krark's Thumb, bots, `mtg-testkit` (`docs/engine.md`).
- Card language and compiler: the DSL, the reader framework, the renderer with goldens, the corpus ratchet, templates for every ability shape and stored `card_rules` (`docs/cards-dsl.md`).
- Protocol and server: generated bindings with a drift test, the game actor, a WebSocket per seat, persistence and resume, accounts (the admin makes, resets and removes them), lobbies, open games, precons, concurrent keep and mulligan, seat presence, the snapshot log (`docs/protocol.md`, `docs/server.md`).
- Game flow: auto-pass, response clocks, yields, stops (own declare attackers and postcombat main pinned by default), Resolve and End turn as separate buttons, Full Control, the timer bank, the by-hand window, the answer to a one-option question, auto choices (trigger order, damage), smart stops, per-ability yields, trigger auto-skips, a clockless opening, the turn time tally per seat, Undo of a cast or activation at every step and leak tests (`docs/ui-design.md`).
- Commander: house rules, format rules, deck check, every partner pair kind end to end, companion in the engine, tax, commander damage and any number of seats (`docs/server.md`).
- Unfinity: Attractions (open, roll to visit, visit and prize abilities, the junkyard, the deck warning), tickets and stickers (three random sheets, name, ability and power/toughness stickers), the sheet and sticker chips at the table (`docs/engine.md`).
- Manual play: the Commands panel, by-hand clauses with chips, manual actions at any time on any turn and fix marks (`docs/web.md`, `docs/protocol.md`).
- Web table: lanes and card sizes, the seat corner, battles with their defense and protector, click combat, the mana strip, legible play (flights, spotlight, target arrows, marks, combat strikes), Equipment tucked at its host with its grants, hand reasons, counters on players at every seat, a crowded stack folded into counted chips and a big attack bundled by target, fit at six sizes, the many-seat layout, a shared large card preview with pins, the win celebration on every seat, a 12 px text floor and glyph marks on narrow faces (`docs/web.md`, `docs/ui-design.md`).
- Lobby, decks and precons: invites, the deck builder with pictures and stats, import and export, the deck check, the not-validated list, deck lines the owner plays by hand (also Verified cards) and the precon browser (`docs/web.md`, `docs/server.md`).
- Reviewing cards: the read endpoints with the fix mark of a `wrong`, `stale_read` and account deletion, and the review screen `/review` (`/read` forwards): one place for a deck's, a precon's or every card's drafts to read and cards played by hand to check, most played first, the card large beside its rules and Oracle text, progress, bulk approval of wording-only drafts, keys C, W, J, K, H and ?, `ReadDialog` from the table's preview and a link from every deck check (`docs/server.md`, `docs/web.md`).
- Control and text: a player controlled during their next turn with the controller answering and seeing for them (CR 722), control of a spell on the stack, text changes in layer 3 (CR 612), cleave, a restart of the game (CR 727) and the cards an opponent finds exiled for the searcher's controller (`docs/engine.md`).
- Learn: the Learn page, tutorials at a table with a scripted practice bot, the guided and manual-panel lessons and page tours (`docs/web.md`).
- Card images: a server cache with a warm-up, and pictures in the table, builder and preview (`docs/server.md`, `docs/web.md`).
- Testing: scenarios, invariants and fuzz decks, `scripts/check.sh` with the `gate` and `affected` modes, content stamps, side-by-side test binaries, family test binaries in `mtg-testkit` and a throwaway Postgres per database test binary, and Playwright specs including the four-seat Commander game with precons or the owner's deck (`E2E_DECK=owner`).
- Deployment: two amd64 images, the Helm chart and ArgoCD app in `k3s-cluster-manifests`, `scripts/release.sh` (build, push, roll out, roll back) and a nightly `pg_dump` onto its own volume (`docs/deploy.md`).

## Live game

Items that touch the running game come first; the play-session findings (pf) lead, the web items before the engine and card items.

- [ ] (pf23b) An affordability check can count one hand card twice, as a Spirit Guide and as the card a Cadaverous Bloom-style cost exiles (the payment itself is right)

- [ ] (d1c) A copy of the nightly database dumps off queen (a Longhorn backup target or a scheduled copy to another machine), so losing queen does not lose the accounts and decks (`docs/deploy.md`)
- [ ] (d1b) The release script's steps as a GitHub Actions workflow on a published release, so a release needs no dev machine (`docs/deploy.md`)
- [ ] (v91k) The house rule key `colour_identity` is spelled `color_identity` on the wire, in the stored rules and in `mtg-core` `house.rs`, with the stored games migrated
- [ ] (fx1b) A cap on the fix marks of one account and a rate limit on the fix-mark routes
- [ ] (pl2l) `join.spec.ts` "a guest follows an invite link" timed out once on the Join button in a full run and never in 50 solo runs: a recurrence needs a Playwright trace (`retain-on-failure`) to name the cause

## M4 Commander

- [ ] (m4c) Companion conditions that need more than the card (mana value parity, an activated ability, a shared type, one mana symbol) in rules, engine and deck check; frees about 4 cards (Jegantha, Kaheera, Gyruda)

## M5 Table and prompts

The table and its prompts are live; the items below polish them.

- [ ] (hd1) On a 390x844 phone the commanders in the hand dock leave the hand line about 53 px, narrower than one card; the hand keeps at least one whole card in view (`features/table/HandDock.tsx`)
- [ ] (a2g) A token lookup that finds several equal records prefers the one with the most printings (`token_pictures` returns the printing count) instead of the smallest oracle id
- [ ] (a2f) The real database holds every precon printing (`fetch scryfall --printings`), so the table draws each card in its precon art instead of the card's default picture
- [ ] (st1a) The Stasis family (Stasis Field, Stasis Cell, Stunning Strike) is measured in a scratch database after `compile`: each becomes Verified or names the clause the checker still lacks, with its Scryfall record in the corpus
- [ ] (st1b) "Its activated abilities can't be activated" (Arrest, Stasis Cocoon) is a restriction the compiler reads and the activation rules enforce (`Modification` and the activation check)
- [ ] (eq1j) `gives` of an Aura on a player names the rules it gives outside keywords ("Enchanted player can't cast spells", skipped draws), which the layers do not record for players (`gives.rs`, `layers.rs` `PlayerHeld`)
- [ ] (dk1b2a) Sieges in the compiler and checker: readers and writers for the Aura/God/Demigod search, "lifelink and indestructible as long as you control three other enchantments" and "this Siege", so Invasion of Theros joins `reproduced.txt` and the other Sieges read
- [ ] (dk1c4a) The soulbond cards the compiler still leaves out: Donna Noble (a damage trigger), Joint Assault ("if it's paired"), Doom Weaver, Imperious Mindbreaker, Breathkeeper Seraph, Mirage Phalanx (quoted grants with their own amount), Flowering Lumberknot, Tarelily, Azra Matchthrower
- [ ] (dk1c5c) Rooms: lock a door (Marina Vendrell, Keys to the House), a token copy of a half (Blu) and the copiable door state (CR 707.2)
- [ ] (dk1d2a) Orzhov Charm: the reader of "Return target creature you control and all Auras you control attached to it to their owner's hand." (the Auras then the creature) and "Destroy target creature and you lose life equal to its toughness.", so the compiler makes the card without its rules file
- [ ] (pn1a) Many-seat tests of the actor for response clocks, timer banks and hidden information per viewer at 13 to 64 seats (the actor tests cover 2 to 4)
- [ ] (pm1a) A server test drives a clocked game and shows that a manual action keeps the deadline of a changed decision of the same seat (`carried_clock`), and that a manual action sent while a replacement question holds a batch is applied once it is answered (`flush_deferred`).

- [ ] Playwright games with corpus decks that ask every decision kind

## Cards

Open card, compiler and wording items: `docs/cards-backlog.md`.

- [ ] Target: every stored card Verified, or as near as the rules allow; after each round `compile` and `coverage` are re-run and the numbers go on the Status line
- [ ] (rt1ea) The by-hand window after `AbilityLeftToTable`: the server holds the controller of the ability at every priority window until it passes and the Priority prompt reads "Apply the card's text by hand", as for a card with clauses left to the table; frees the table to apply a failed read ability (`mtg-server` stops, `web/src/play/byHand.ts`, `docs/server.md`, `docs/web.md`)
- [ ] (sa23a) The read sheet pairs a Saga clause of several chapters ("I, II, III —") with each of its chapter abilities, so the rows after it no longer show the abilities of earlier chapters (41 drafts, 24 corpus cards)
- [ ] (sa23b) `semantic-audit` checks reach 70% precision: they read the DSL forms that hold a clause otherwise (conditions folded into trigger filters, `Shield` and `Rule` replacements, `Sum`/`When` arithmetic), measured on a fresh 40-finding sample
- [ ] (fg1j) `translate-forge`: the proposals that read back as other text (about 950 cards that are not Verified yet: the creature type with and without "creature", `enchanted` for `equipped`, colours in tokens, "its controller" for "that player") name the DSL or writer wording to settle; the ledger of `check-rules --gaps` over `proposals/forge` ranks them, and the `Unchecked` ones (about 1,050) wait for the missing writers it lists
- [ ] (fg2) Audit before promoting translated cards: read the review sheets of 100 random translated cards the checker calls Verified and count the wrong ones (an Opus or Sonnet reader, sheet beside the printed text); promote in bulk only when the error rate is low, and show "from Forge" in the review dialog of translated drafts (`meta/<slug>.json` `source`)
- [ ] (fg3t) `translate-forge` gaps by cards blocked, as `translate-gaps.tsv` ranks them: `duration:Perpetual` (116), `keyword:StickerSheet` (48), `trigger:CrankContraption` (45), `keyword:Banding` and `effect:Draft` (39), `effect:PutSticker` (34), `keyword:Splice` (30), `key:Seek.RememberFound` (28), `key:Trigger.SpellCast.Static` (26) and `effect:StoreSVar` (24); nodes the language lacks are in `engine-needs.tsv`
- [ ] (fp1f) A by-hand trigger line whose event reads carries `when`: the engine logs a public `ByHandDue` as the event happens, the seat holds its by-hand window and the log says the line is due, with no change to any recipe fingerprint (`compile/read/triggers.rs`, `mtg-engine`, `mtg-protocol`, `mtg-server` stops, `web` log; after fp1c and rt1ea)
- [ ] (fp1g) The Commands panel opens with a read-only "To do by hand" group of the seat's due lines, cleared by Done or by passing (`features/table/manual/panel/`, after fp1f and the UI session's v96 items)
- [ ] (fg3c) `translate-forge`: the whole translations the checker reads back as other text (`Mismatch`): separate wording differences from wrong translations and fix the mappers
- [ ] (r4e2) A static ability that gives replicate or emerge at its card's mana cost (Djinn Illuminatus, Hatchery Sliver, Ian Chesterton, Herigast): the optional costs and ways read the printed card, not the layered keywords
- [ ] (rt1n) A 4-seat bot game with 16 copies of Exotic Orchard takes 94 s for 2,635 answers (about 40 ms an answer, against 0.1 ms usual), so the smoke gate stops it at its 20 s limit; find the slow step of its mana ability and make it fast (`crates/mtg-engine` mana; no dependency)
- [ ] (un33c) Stickers left: the kinds a card names ("put a name sticker", art stickers, an ability sticker with ticket cost X or less) for Baaallerina, Bird Gets the Worm, O-Saurus, Rocketship, Roxi and Pin Collection; "whenever you put a sticker" (A Good Day to Pie, Make a Splash, Unlawful Entry); "with a name sticker on it" and Forge `stickered`; stickers kept through public zones; the sticker sheets' oracle text read into `Ability::Sticker` with corpus sheets; the translator's `StickerSheet` and `StickerPlaced`
- [ ] (un33d) Attractions left: a database test of the lights a stored deck's Attraction deck plays with, and Motion Sickness's "attach this Aura to target tapped creature"
- [ ] Gate: every corpus card is Verified or played through manual clauses and actions that show each unread clause, and fuzz games with a deck from each cluster pass every invariant, and a four-seat Commander game plays from the lobby to a winner in the web

## Tooling

- [ ] (ts1e) The release images build while the batch run of the release commit runs, both images at once, and `scripts/release.sh` reuses them from the build cache (release after the check: 600 s to about 120 s; `scripts/release.sh`, `docs/deploy.md`; after ts1d)
- [ ] (ts1f) A diff of card files only tests those cards, the test modules and fixture decks that name them and a fixed sample, and the batch run tests the whole corpus (a card change to about 3 min; `scripts/check.sh`; after ts1a)
- [ ] (ts1j) The built-in library loads from a snapshot made at build time instead of 1.1 s of parsing per process, so nextest with per-test timeouts can replace the run cap (`crates/mtg-cards`; after ts1c)
- [ ] (ts1o) The tag run (`scripts/check.sh all`) sets `MTG_FUZZ_GAMES` to the long count and the fuzz binary still ends within the run cap (`scripts/check.sh`, `docs/testing.md`)
- [ ] (ts1n) `scripts/check.sh` puts `scripts/warm-bin` first in `PATH` for its cargo calls and runs `scripts/tests/warm_target.sh` with its script tests, so a warm worktree needs no prefix (`scripts/check.sh`; after ts1a to ts1d)
- [ ] (ts1m) A workspace-hack crate (cargo-hakari) fixes the dependency features, so every package selection shares one build and `cargo test -p mtg-engine` in a warm worktree compiles nothing (now 86 s; `Cargo.toml`, `crates/`; no dependency)
- [ ] (dep) Cargo and web dependencies are current; the piece runs alone between rounds (`docs/agents.md`)
- [ ] (t5b) The `mtg-testkit` `it` binary needs fewer test-seconds: over 90% is `Engine::apply` (`priority_options` 36%, `triggers::detect` 24%, `layers::compute` 14%, whole-object clones), so the next cut is in the engine; opt-level 3 for `mtg-cards` and mimalloc save about 5% and slow every rebuild, so they are not adopted
- [ ] (t1) The `mtg-cli` `fetch_scryfall` tests are stable under parallel load
- [ ] (t3c) DSL amounts for what an earlier effect did and for the mana value a cost took, and a partner by creature type (Foul-Tongue Shriek, Nahiri's Sacrifice, The Knight of Land Drops)
- [ ] Coverage dashboard in the web over `Library::support`: states, reasons and clause spans per card, ranked by saved-deck demand (`docs/deck-check.md`)

## M7 Product

- [ ] (tm1) Team Commander: 2v2 as Two-Headed Giant (shared turn and life, CR 810) and teams of any size N v N with shared victory (CR 808), chosen on the create form
- [ ] (tm2) Tournaments: an organiser sets up 1v1 or 2v2 Commander rounds (Swiss or single elimination), pairings, results and standings, each match a normal game
- [ ] (w2d) Handicaps (life and opening hand per seat) on the create form's house rules
- [ ] (f1c) The deck report carries the quoted span with each reason (`mtg-cards` `Reason`), so the not-validated list and the precon details read no card per flagged card from `GET /api/cards/{oracle_id}` (`docs/web.md`, the not-validated list)
- [ ] The importer fixtures of `mtg-import` hold real records instead of invented oracle ids and printings; synthetic edge cases stay marked as synthetic
- [ ] Verify the MTGO and Arena export parsers against real exports, with companions and custom cards (`docs/data-sources.md`)
- [ ] (v1f) A spectator's connection in the server: a socket that holds no seat, is sent `StateView::for_spectator` and the spectator's batches, and a rule for who may watch
- [ ] Spectators in the web

## M8 Polish and deploy

- [ ] Animations beyond the table feedback (`docs/web.md`), sounds, replays, solo play against bots
- [ ] Performance profile on real hardware with real card art
- [ ] (p1a) Block options test evasion once per attacker and per blocker (0.4 s at 1,500 against 1,500 today), and a death re-derives the layers only when an effect reads it
