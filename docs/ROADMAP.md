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
- Web table: lanes and card sizes, the seat corner, battles with their defense and protector, click combat, the mana strip, legible play (flights, spotlight, target arrows, marks, combat strikes), Equipment tucked at its host with its grants, hand reasons and plain refusal sentences, counters on players at every seat, a crowded stack folded into counted chips and a big attack bundled by target, fit at six sizes, the many-seat layout, a shared large card preview with pins, the win celebration on every seat, a 12 px text floor and glyph marks on narrow faces (`docs/web.md`, `docs/ui-design.md`).
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
- Delves fairness: the spotlight spreads the boss's side's attacks, removal and spotlight schemes over the heroes with a cap on any one hero's share, the grace and the lull open each fight, a boss takes commander damage but never loses to 21, and the table shows whom each boss-side seat eyes (`docs/delves.md`).
- Delves difficulty: one scaling table by party size and Heat (life, hand, Reserves, Reinforcements, Warpath, minion seats that fall with the boss, the Last Bulwark), Escalation and Enrage, the scheme deck with its Omen, measured per party size (`docs/delves.md`).

## Live game

Items that touch the running game come first; the play-session findings (pf) lead, the web items before the engine and card items.

- [ ] (d1c) A copy of the nightly database dumps off queen (a Longhorn backup target or a scheduled copy to another machine), so losing queen does not lose the accounts and decks (`docs/deploy.md`)
- [ ] (d1b) The release script's steps as a GitHub Actions workflow on a published release, so a release needs no dev machine (`docs/deploy.md`)
- [ ] (pl2m) `join.spec.ts` "a guest follows an invite link" times out on the Join button only in a full run: the trace a failed run keeps (`web/test-results`) names the cause, and the fix makes the spec pass in full runs

## M5 Table and prompts

The table and its prompts are live; the items below polish them.

- [ ] (a2f) The real database holds every precon printing (`fetch scryfall --printings`), so the table draws each card in its precon art instead of the card's default picture
- [ ] (st1c) Stunning Strike reads whole: writers for "tap enchanted creature and remove it from combat" and "As long as enchanted creature isn't legendary, it doesn't untap during its controller's untap step." (Stasis Field and Stasis Cell are Verified)
- [ ] (dk1b2b) The other Sieges' unread clauses: the enter triggers of Invasion of Arcavios (search outside the game), Ergamon, Kamigawa, Karsus, Lorwyn (X lands) and Regatha (two targets), and back-face lines of Eldraine, Ergamon, Fiora, Kamigawa and Regatha
- [ ] (hx29a) Detail for the refusals still told only by their reason: a cast taken back for its cost or mana (`AnnouncementRewound.why`), an Aura, bestow or mutate spell with no host (`attach`, `merged`), the abilities of cards in a graveyard, exile or the command zone (`PriorityOptions.inactive` covers the battlefield), and a `-X` or X ability whose least X has no target
- [ ] (dk1c4a) The soulbond cards the compiler still leaves out: Donna Noble (a damage trigger), Joint Assault ("if it's paired"), Doom Weaver, Imperious Mindbreaker, Breathkeeper Seraph, Mirage Phalanx (quoted grants with their own amount), Flowering Lumberknot, Tarelily, Azra Matchthrower
- [ ] (dk1c5c) Rooms: lock a door (Marina Vendrell, Keys to the House), a token copy of a half (Blu) and the copiable door state (CR 707.2)

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

- [ ] (tm1a) Team life and choices of Two-Headed Giant beyond the shared total: an effect that sets each player's life (CR 810.9e), the team mulligan, teammates who see each other's hands, priority by team and each player declaring their own attackers
- [ ] (tm1b) Teams the host sets in the lobby are stored when set, so a restart before the start keeps them, and My games carries each seat's team so a team win counts for its members
- [ ] (tm2b) A tournament match game offers its seat-0 player no host controls (visibility, Close game, Remove), so no player can strand a match; the organiser's correction stays the way to settle one (`docs/server.md`, Tournaments)
- [ ] The importer fixtures of `mtg-import` hold real records instead of invented oracle ids and printings; synthetic edge cases stay marked as synthetic
- [ ] Verify the MTGO and Arena export parsers against real exports, with companions and custom cards (`docs/data-sources.md`)
- [ ] (ai36a) Hard beats Normal by 70% or more in 1v1 and Very Hard beats Hard by 65% or more; Normal with Hard's former searches beats main's Normal 59.6% but aims Pacifism at a teammate in Two-Headed Giant (target ranking of auras), so it waits for that fix (`crates/mtg-testkit/src/brain`, `examples/bot_tournament.rs`)
- [ ] (ai54a) Very Hard beats main's Very Hard by 62% or more and Hard main's Hard by 58% in 4-player pods and Commander pods with SPRT significance (Hard 1v1 59.1%, accepted; Very Hard 1v1 49.6% over 121 games, Hard pods 56.1% over 82), so Very Hard's guarded margin and its importance budget want their own tuning; Texel weights fitted on self-play (held-out loss 0.229 to 0.188) wait for a match test, and a pod Commander game of seed 51405 loops a choose-option question at turn 44 (`crates/mtg-testkit/src/brain`, `examples/bot_tournament.rs`)
- [ ] (v1h) A "waiting on <player>" mark on the pods of a table of three or more, beside the strip's line, so a spectator and a player see at a glance whom the game waits for

## Delves

- [ ] (dl2) The party shares one team turn against the boss (CR 805 beyond Two-Headed Giant), so four heroes do not wait three turns
- [ ] (dl3b) Delves balance at every size: measure four to six heroes with the spotlight and ramp-up, and lift two heroes' fight win rate to 50 %
- [ ] (dl3a) Boss personalities: each boss weighs threats its own way (the healer, the biggest board, the combo) within the spotlight's cap
- [ ] (dl4) A won card names the card it replaces (a basic land today), chosen from the hero's deck list on the loot screen
- [ ] (dl5) Campfire upgrades put a sticker on a card of the hero's deck for the rest of the run
- [ ] (dl6) Risky relics are party decisions: the party votes to take one, and events can be party votes with a shared outcome
- [ ] (dl7) The scouts' forecast: background bot fights measure each boss of a new ladder and the ladder shows the measured win rate
- [ ] (dl8a) Boss brain priorities: the boss and its minions focus the biggest threat, punish the healer and break combos, at Very Hard once its clock suits the server
- [ ] (dl8b) Whole runs won in 20 to 35 % at every party size, measured for 1 to 6 heroes and with Very Hard heroes
- [ ] (dl9) A boss fight whose game is closed or expires without a result counts as lost and is made again, and the host can retreat from a fight
- [ ] (dl10) The fight table re-measured on the current bosses (two Hard heroes win 0/4 of seeds 1 to 4) and the Spirits' worth, acting against idle, at three or more heroes
- [ ] (dl10) The run page follows the run over a socket instead of reading it every 4 s
- [ ] (dl11) Custom cards open in the card inspector at the table with their frame and text, and the boss's phases sit in the top bar
- [ ] (dl12) e2e: a one-player run with bot allies from the lobby through the first boss fight to the loot screen
- [ ] (dl13) A second theme with its bosses, events and reward pool; Heat 3 and above

## M8 Polish and deploy

- [ ] Animations beyond the table feedback (`docs/web.md`)
- [ ] Performance profile on real hardware with real card art
- [ ] (p1b) A death re-derives the layers only when an effect reads it, so a sweep of a table full of static abilities costs time in proportion to the table (`layers.rs` `leaves_alone`, `crowded_turn.rs`)
