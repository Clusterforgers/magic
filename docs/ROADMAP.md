# Roadmap

What is open and what comes next. Continue with the first open piece of "Live game", then "Next up".

One item is one piece of about 15 minutes and one line: the outcome, the cards it frees and a few examples. The commit that finishes it deletes its line, turns a leftover into a new lettered item and names a new area under "Done"; designs are in `docs/`. Open card, compiler and wording items are in `docs/cards-backlog.md` under the same rules.

## Status

- The game is live at https://magic.hivemindcloud.dk (k3s; `docs/deploy.md`); `scripts/release.sh` releases a landed batch.
- Live: foundations, team play (Two-Headed Giant and N v N teams), card data, engine core, protocol and server, game flow, Commander, the web table with prompts for every decision kind, lobby, deck builder and precon browser, manual play, card review, tutorials and the Learn page, deployment.
- Cards: 94.7% of the 32,116 Commander cards play automatically (Verified 22,622, Read 4,863, Unverified 2,935); Partial (973) and Manual (723) cards play through by-hand clauses. Run `mtg-cli compile` and `coverage` before quoting newer numbers.
- In progress: refusal explanations, tutorial fixes and the card coverage of M3 and M6.
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
- Protocol and server: generated bindings with a drift test, the game actor, a WebSocket per seat, persistence and resume, accounts (the admin makes, resets and removes them), lobbies, open games, precons, concurrent keep and mulligan, seat presence, the snapshot log, spectators (`docs/protocol.md`, `docs/server.md`).
- Game flow: auto-pass, response clocks, yields, stops (own declare attackers and postcombat main pinned by default), Resolve and End turn as separate buttons, Full Control, the timer bank, the by-hand window, the answer to a one-option question, payment checks that count a hand card once, auto choices (trigger order, damage), smart stops, per-ability yields, trigger auto-skips, a clockless opening, the turn time tally per seat, Cancel of a cast or activation at every step (mana tapped for it untapped), take-backs with the table's approval (a house rule, off in tournaments) and leak tests (`docs/ui-design.md`).
- Commander: house rules, format rules, deck check, every partner pair kind end to end, companion in the engine with every printed condition, tax, commander damage and any number of seats, clocks and hidden hands tested up to 64 (`docs/server.md`).
- Unfinity: Attractions (open, roll to visit, visit and prize abilities, the junkyard, the deck warning), tickets and stickers (three random sheets, name, ability and power/toughness stickers), the sheet and sticker chips at the table (`docs/engine.md`).
- Manual play: the Commands panel, by-hand clauses with chips, manual actions at any time on any turn, under a clock and behind a replacement question, and fix marks with a cap and a budget (`docs/web.md`, `docs/protocol.md`).
- Web table: lanes and card sizes, the seat corner, battles with their defense and protector, click combat, the mana strip, legible play (flights, spotlight, target arrows, marks, combat strikes), Equipment tucked at its host with its grants, hand reasons and plain refusal sentences, counters on players at every seat, a crowded stack folded into counted chips and a big attack bundled by target, fit at six sizes, the many-seat layout, a shared large card preview with pins, the win celebration on every seat, a 12 px text floor, glyph marks on narrow faces and crowded tables (Delves heroes, six-seat grids, the preview near 1024 px) (`docs/web.md`, `docs/ui-design.md`).
- Tournaments: Swiss and single elimination for 1v1 and 2v2 Commander, sign-up by invite, games made and results read per match, the organiser's corrections, drops, byes and standings with tiebreakers (`docs/server.md`, `docs/web.md`).
- Lobby, decks and precons: invites, the deck builder with pictures and stats, import and export, the deck check, the not-validated list, deck lines the owner plays by hand (also Verified cards), the art picker with preferred art per account and the precon browser (`docs/web.md`, `docs/server.md`).
- Reviewing cards: the read endpoints with the fix mark of a `wrong`, `stale_read` and account deletion, and the review screen `/review` (`/read` forwards): one place for a deck's, a precon's or every card's drafts to read and cards played by hand to check, most played first, the card large beside its rules and Oracle text, progress, bulk approval of wording-only drafts, keys C, W, J, K, H and ?, `ReadDialog` from the table's preview and a link from every deck check (`docs/server.md`, `docs/web.md`).
- Control and text: a player controlled during their next turn with the controller answering and seeing for them (CR 722), control of a spell on the stack, text changes in layer 3 (CR 612), cleave, a restart of the game (CR 727) and the cards an opponent finds exiled for the searcher's controller (`docs/engine.md`).
- Learn: the Learn page, tutorials at a table with a scripted practice bot, the guided and manual-panel lessons and page tours (`docs/web.md`).
- Card images: a server cache with a warm-up, and pictures in the table, builder and preview (`docs/server.md`, `docs/web.md`).
- Testing: scenarios, invariants and fuzz decks, `scripts/check.sh` with the `gate` and `affected` modes, content stamps, side-by-side test binaries, family test binaries in `mtg-testkit` and a throwaway Postgres per database test binary, and Playwright specs (a trace kept for every failed test) including the four-seat Commander game with precons or the owner's deck (`E2E_DECK=owner`).
- Polish: replays of finished games from any seat's side with only what the viewer may know, block options that read each attacker and blocker once, solo play with bot seats that never stall a game at four levels (Easy random, Normal and Hard searching engine copies, Very Hard an information-set Monte Carlo tree search, Hard and Very Hard in worlds whose hidden cards are redealt, team-aware, capped thinking slots on the server, a threat model and interaction policy scored by a decision-quality suite, chess-style search with a decision clock, minimax block and counter replies and an endgame probe, search authority with a guarded margin and judged rollouts, SPRT match testing on interaction decks and pods), synthesized sound cues with a mute (`docs/server.md`, `docs/web.md`, `docs/bots.md`).
- Deployment: two amd64 images, the Helm chart and ArgoCD app in `k3s-cluster-manifests`, `scripts/release.sh` (build, push, roll out, roll back) and a nightly `pg_dump` onto its own volume (`docs/deploy.md`).
- Delves: the co-op boss rush with own Commander decks for 1 to 6 heroes, the Warrens ladder of four bosses with phases, relics, loot, shop, campfire and events, Spirits at 0 life, runs stored as event logs with fights as recorded games, the web pages and the bot simulator (`docs/delves.md`).
- Delves at the table and between fights: boss mode (the boss's side on top with its abilities in plain words from the content the engine plays, the party together at the bottom), the boss intro in place of the game's lobby, decks fixed for the run with a lost deck held on the run page, the return to the run after a fight, the runs list without abandoned runs and with past runs folded (`docs/delves.md`).
- Delves fairness: the spotlight spreads the boss's side's attacks, removal and spotlight schemes over the heroes with a cap on any one hero's share, the grace and the lull open each fight, a boss takes commander damage but never loses to 21, the table shows whom each boss-side seat eyes, each hero pays the attack tax of their own creatures, and the boss bot attacks only within what it can pay (`docs/delves.md`).
- Delves difficulty: one scaling table by party size (life, hand, Reserves, Reinforcements, Warpath, minion seats that fall with the boss, the Last Bulwark), Escalation and Enrage, the scheme deck with its Omen, measured per party size; the bosses ramp by ladder rank and the three Heats (Casual, Seasoned, Heroic) by one rank × Heat table with the Enrage turn by Heat, worded on the new-run form and the run header (`docs/delves.md`).
- Delves boss roster: thematic boss decks written in brackets 1 to 5 (40, 50 and 60 cards) that carry the difficulty by Heat and ladder rank, a commander card each boss casts free as its last phase begins, no infinite combos and a loop cap of five for the boss's side, the five mono-colour bosses (Seraphine and Ysolde new) and upgraded final bosses, behind content generation 4 (`docs/delves.md`).
- Delves generation 5: five two-colour bosses (Hollowmere WU Spirits, Morwenna UB Faeries, Korrath RG Dinosaurs, Sahrra GW Cats, Valdric WB Vampires), upgraded Vex, Seraphine and Ysolde, and the Unmaker, an uber boss at Heat 2 whose engine rule draws the heroes' cost colours anew each round with the cost now on the card (`docs/delves.md`).
- Delves shared turns: the party and the boss's side each take one team turn (CR 805) with life totals per hero, each hero chooses their own creatures' attacks for one shared declaration, and the table names the party's turn (`docs/delves.md`).
- Delves endless: after a floor's final boss the party votes to bank the win or descend to a new ladder of bosses one bracket stronger per floor, with no early handicaps, a risky relic more and Intensify (earlier Enrage, passive once more), behind content generation 5; from generation 7 a descent first gives the final boss's loot (`docs/delves.md`).
- Delves generation 6: the last four two-colour bosses (Ozrith UR Wizards, Aurenne WR Knights, Vorthana BG Insects, Sivanne UG Snakes), so a boss stands for every colour pair, with generation-5 ladders pinned (`docs/delves.md`).
- Delves leaderboard: each player's deepest run (floors cleared, then bosses beaten below them) with their commanders, one tab per Heat from Heroic down, beside the Delves page (`docs/delves.md`).
- Delves rewards: the Satchel of earned cards cast from beside the command zone at every fight with the deck kept whole, the Mirror Pool's copy, commander upgrades fitted to each commander as emblems, relic rarities (Common, Rare, run-defining Mythic) that decide loot, treasure and shop offers and prices, and events as known trade-offs of life, life cap, a card, a relic or a commander bane against rewards that grow with the cost (`docs/delves.md`).
- Fights and the table: a fight that ends without a result counts as lost and the host can retreat, Spirits are asked only where their haunts matter, the rules lexicon covers every engine term, decks sit in several groups, token planeswalkers show loyalty, the round counter, the blocker timer and grid lands as choices, and the card preview stays clear of the prompt (`docs/delves.md`, `docs/web.md`)
- GUI sweep: every screen, table mode and prompt kind shot at the owners' sizes, the findings ranked in `docs/ui-audit.md`.
- Feel: the twenty rules of motion and interaction, press squash and 44 px hit areas, card travel on an arc with a wind-up and a settle, attacks, hits, entries and deaths pose to pose, the staged stack and attacked seat, the swelling pass button and snappier strikes; answers felt on the press (a working mark past 400 ms, a cast leaving the hand, a tap before the lunge, 44 px stops on touch, builder cards travelling into the list, Delves votes and buys that beat) and the side column on the log (`docs/ui-design.md`, `docs/ui-audit.md`).
- Holding still: card sizes and the board split hold until a board no longer fits or clearly has room, the prompt sheet lies in a band of one height per window kept free above the strip so it covers no card, the boss keeps a fair share of the heroes' card size, a board's bands keep their height until a size step, the lane of other permanents grows leftward, the hand dock changes its rows once a step, moved cards slide, and `scripts/layout-jumps.mjs` counts the jumps of a scripted stretch of play (`docs/web.md`, Holding still).
- Team sight: a teammate's zones open from every layout and teammates see each other's hands (`team_hands`, on at every team table), never an opponent or a spectator.
- Loops (CR 104.4b, 732): a loop found at priority points, a draw for a mandatory loop that never ends, a shortcut of N iterations its driver chooses, run as one batch with a summary line, bot answers, and the prompt in every layout (`docs/engine.md`, Loops).
- Choosing faster: quick selection buttons (all, owned, allied in team modes, none) in prompts that pick several objects, a random precon or own deck in every mode, hidden until the start, and "Always pay automatically" for payments without a real choice (`docs/web.md`, `docs/server.md`).
- Team tables: a tournament match has no player host, so its seat 0 gets only what every player gets in the lobby (`tournament_match`); at a table of three or more the pods of the seats the game waits on carry a "waiting for" mark in every layout; teams set in the lobby are stored at once and survive a restart, and My games carries each seat's team so a team's win counts for its members; an effect that sets each player's life sets a shared total once.
- Card preview modifiers: under the rules text the preview lists everything that changes the card with its source and duration, identical sources added together, a line pointing at its source on the board, and no hidden source named (`docs/protocol.md`, `docs/web.md`).
- Middle bar: a prompt band of one fixed line with the asking card's chip, and the decision's detail (the asking card with its clause, runs of triggers, ordering, paying) in the Stack column or a sheet over the hand (`docs/ui-design.md`, The band and the Stack column).

## Live game

Items that touch the running game come first; the play-session findings (pf) lead, the web items before the engine and card items.

- [ ] (d1b) The release script's steps as a GitHub Actions workflow on a published release, so a release needs no dev machine (`docs/deploy.md`)
- [ ] (pl2m) `join.spec.ts` "a guest follows an invite link" times out on the Join button only in a full run: the trace a failed run keeps (`web/test-results`) names the cause, and the fix makes the spec pass in full runs
- [ ] (pf9b2) Prompt cards under 76 px wide draw a compact face (name, cost, power and toughness) whole instead of cutting the full face's foot line
- [ ] (pf9b3) At 1280 × 720 a card prompt's sheet holds its first line and one row of whole cards without scrolling, and more than eight target cards wrap at 1400 × 900
- [ ] (pf9d2) An eight-seat grid at 1400 × 900 shows a row of each opponent's cards
- [ ] (pf9d3) Your pod column in boss mode at 1400 × 900 shows all its chips without scrolling
- [ ] (pf10m) Dev fixtures for the deck list and groups, the replay and the open games list, so a sweep shoots them without a server
- [ ] (pf9f) Stable tests under load: the 40-tap budget in `perf.test.tsx` and the `check_sched` lock-wait case
- [ ] (pf9j) The view tests' seat loops iterate instead of indexing, and the `needless_range_loop` and `map_unwrap_or` allowances Rust 1.100 needed are dropped
- [ ] (pf9k) Priority offers a spell that only an announced mana ability (Vexing Puzzlebox) makes affordable, so it can be cast and paid from the payment prompt
- [ ] (pf9i) An announced mana ability (a die roll or discard besides the mana: Vexing Puzzlebox) can be chosen in the PayMana prompt while paying, not only at priority before the cast; automatic payment still leaves it out
- [ ] (lp2a) A loop that makes a permanent each cycle runs thousands of iterations in one shortcut: the trigger scan, the target lists and the state-based actions of each step no longer read every permanent

## M5 Table and prompts

The table and its prompts are live; the items below polish them.

- [ ] (a2f) The real database holds every precon printing (`fetch scryfall --printings`), so the table draws each card in its precon art instead of the card's default picture
- [ ] (st1c) Stunning Strike reads whole: writers for "tap enchanted creature and remove it from combat" and "As long as enchanted creature isn't legendary, it doesn't untap during its controller's untap step." (Stasis Field and Stasis Cell are Verified)
- [ ] (dk1b2b) The other Sieges' unread clauses: the enter triggers of Invasion of Arcavios (search outside the game), Ergamon, Kamigawa, Karsus, Lorwyn (X lands) and Regatha (two targets), and back-face lines of Eldraine, Ergamon, Fiora, Kamigawa and Regatha
- [ ] (hx29a) Detail for the refusals still told only by their reason: a cast taken back for its cost or mana (`AnnouncementRewound.why`), an Aura, bestow or mutate spell with no host (`attach`, `merged`), the abilities of cards in a graveyard, exile or the command zone (`PriorityOptions.inactive` covers the battlefield), and a `-X` or X ability whose least X has no target
- [ ] (dk1c4a) The soulbond cards the compiler still leaves out: Donna Noble (a damage trigger), Joint Assault ("if it's paired"), Doom Weaver, Imperious Mindbreaker, Breathkeeper Seraph, Mirage Phalanx (quoted grants with their own amount), Flowering Lumberknot, Tarelily, Azra Matchthrower
- [ ] (pm1) The preview's "Modified by" also names cost changes (reductions, taxes, the Unmaker's drawn colours) by source, and a seat pod lists what changes the player by source
- [ ] (dk1c5c) Rooms: lock a door (Marina Vendrell, Keys to the House), a token copy of a half (Blu) and the copiable door state (CR 707.2)
- [ ] (mb1) Pay for me previews its payment: `PayOptions.auto_plan` (the `Payment` the game would make) fills the circles on hover before the press
- [ ] (mb2) An asking source that has left its zone keeps its picture: a last-known face (`AskingView` identity) for the chip and the column's card
- [ ] (mb3) The asked clause for every decision of a spell or ability: the spell's own text and the clause of a mode, a number, an option and a choice of cards from the engine's position in the effect, not from the words
- [ ] (mb4) The keyboard walks from the band's answers into the Stack column and back without crossing the boards, and the phone's board split holds the layout script to six jumps again (`layoutJumps.test.ts`)
- [ ] (ft1) The opening hands at 1920x1080 (dev table and a real game) leave the hand dock and your life uncovered by the shared sheet, or the mulligan detail moves to the Stack column where one is open
- [ ] (ft2) With the Commands panel open the block prompt's sentence stays visible and pressable (phone, short phone, tablet, rail), and on a 360x740 phone the Commands sheet leaves your life in view
- [ ] (ft3) The duel board at 1920x1080 keeps your creatures at 190 px under the band, the opponent's corner at 1440x900 draws pictures, and the opponent's pod column does not scroll at 1366x768
- [ ] (ft4) The crowded scene's bundle labels stay whole and off the strip at 390x844; `fit.spec.ts`'s real game-over test reaches the end without a 30 s stall
- [ ] (ft5) `layout.spec.ts` passes again under the band (43 of the first 99 failed)
- [ ] (mb5) `/dev/table?cards=real` over a card fixture of its own, so the asking card's art shows without a running server

- [ ] Playwright games with corpus decks that ask every decision kind

## Cards

Open card, compiler and wording items: `docs/cards-backlog.md`.

- [ ] Target: every stored card Verified, or as near as the rules allow; after each round `compile` and `coverage` are re-run and the numbers go on the Status line
- [ ] (sa23b) `semantic-audit` checks reach 70% precision: they read the DSL forms that hold a clause otherwise (conditions folded into trigger filters, `Shield` and `Rule` replacements, `Sum`/`When` arithmetic), measured on a fresh 40-finding sample
- [ ] (fg1j) `translate-forge`: the proposals that read back as other text (about 950 cards that are not Verified yet: the creature type with and without "creature", `enchanted` for `equipped`, colours in tokens, "its controller" for "that player") name the DSL or writer wording to settle; the ledger of `check-rules --gaps` over `proposals/forge` ranks them, and the `Unchecked` ones (about 1,050) wait for the missing writers it lists
- [ ] (fg2) Audit before promoting translated cards: read the review sheets of 100 random translated cards the checker calls Verified and count the wrong ones (an Opus or Sonnet reader, sheet beside the printed text); promote in bulk only when the error rate is low, and show "from Forge" in the review dialog of translated drafts (`meta/<slug>.json` `source`)
- [ ] (fg3t) `translate-forge` gaps by cards blocked, as `translate-gaps.tsv` ranks them: `duration:Perpetual` (116), `keyword:StickerSheet` (48), `trigger:CrankContraption` (45), `keyword:Banding` and `effect:Draft` (39), `effect:PutSticker` (34), `keyword:Splice` (30), `key:Seek.RememberFound` (28), `key:Trigger.SpellCast.Static` (26) and `effect:StoreSVar` (24); nodes the language lacks are in `engine-needs.tsv`
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

- [ ] (tm1c) Two-Headed Giant choices by team: the team mulligan, priority by team, and the team choosing which player an effect that sets each player's life affects (now its first player named; CR 810)
- [ ] The importer fixtures of `mtg-import` hold real records instead of invented oracle ids and printings; synthetic edge cases stay marked as synthetic
- [ ] Verify the MTGO and Arena export parsers against real exports, with companions and custom cards (`docs/data-sources.md`)
- [ ] (ai36a) Hard beats Normal by 70% or more in 1v1 and Very Hard beats Hard by 65% or more; Normal with Hard's former searches beats main's Normal 59.6% but aims Pacifism at a teammate in Two-Headed Giant (target ranking of auras), so it waits for that fix (`crates/mtg-testkit/src/brain`, `examples/bot_tournament.rs`)
- [ ] (ai54a) Very Hard beats main's Very Hard by 62% or more and Hard main's Hard by 58% in 4-player pods and Commander pods with SPRT significance (Hard 1v1 59.1%, accepted; Very Hard 1v1 49.6% over 121 games, Hard pods 56.1% over 82), so Very Hard's guarded margin and its importance budget want their own tuning; Texel weights fitted on self-play (held-out loss 0.229 to 0.188) wait for a match test, and a pod Commander game of seed 51405 loops a choose-option question at turn 44 (`crates/mtg-testkit/src/brain`, `examples/bot_tournament.rs`)

## Delves

- [ ] (dl3c) Delves balance at three to six heroes: measure them on seeds 1 to 8 (a fight takes 30 to 90 minutes of machine time or more) and lift any size that wins below 50 %
- [ ] (dl3a) Boss personalities: each boss weighs threats its own way (the healer, the biggest board, the combo) within the spotlight's cap
- [ ] (dl2d) Each hero gives up the permanents an attack tax takes (sacrifice a land) for their own creatures in the party's combined attack, not the leader
- [ ] (dl8c) The boss side's targeted spells say in the log why they leave the eyed hero, as its attacks do
- [ ] (dl8d) The glossary explains each boss's Enrage by its own rules, not as the Magic keyword
- [ ] (dl4a) Bot allies temper and buy commander upgrades, and the simulator's heroes bring their commander facts
- [ ] (dl4b) Bot allies see their deck list, so they can give up a deck card for an event's cost and not only a Satchel card
- [ ] (dl5) Campfire upgrades put a sticker on a card of the hero's deck for the rest of the run
- [ ] (dl6) Risky relics are party decisions: the party votes to take one, and events can be party votes with a shared outcome
- [ ] (dl7) The scouts' forecast: background bot fights measure each boss of a new ladder and the ladder shows the measured win rate
- [ ] (dl8a) Boss brain priorities: the boss and its minions focus the biggest threat, punish the healer and break combos, at Very Hard once its clock suits the server
- [ ] (dl8b) Whole runs won in 20 to 35 % at every party size, measured for 1 to 6 heroes and with Very Hard heroes
- [ ] (dl10a) The Spirits' worth, acting against idle, at three or more heroes
- [ ] (dl10) The run page follows the run over a socket instead of reading it every 4 s
- [ ] (dl11) Custom cards open in the card inspector at the table with their frame and text, and the boss's phases sit in the top bar
- [ ] (dl13) A second theme with its bosses, events and reward pool; Heat 3 and above
- [ ] (dl14a) Measure the Hard-boss rows of the Difficulty table (Heat 0 ranks 2 and 3, Heat 1 ranks 1 to 3, Heat 2) and parties of three or more against the tier targets, and tune the Normal-brain rows, which win 92 to 98 % against two Normal heroes
- [ ] (dl14b) Softer boss passives at Heat 0 and harsher ones at Heat 2, a variant of each passive by Heat as the Enrage has
- [ ] (dl16a) Measure deeper Delves floors with the simulator (floors 2 and 3 at each Heat) and tune the per-floor steps against the tier targets
- [ ] (dl15l) Measure the bracket generation against the tier targets with Hard bosses and tune each boss's brackets by its win rate
- [ ] (dl15n) More uber bosses with a game-changing rule each

## M8 Polish and deploy

- [ ] Animations beyond the table feedback (`docs/web.md`)
- [ ] Performance profile on real hardware with real card art
- [ ] (p1b) A death re-derives the layers only when an effect reads it, so a sweep of a table full of static abilities costs time in proportion to the table (`layers.rs` `leaves_alone`, `crowded_turn.rs`)
