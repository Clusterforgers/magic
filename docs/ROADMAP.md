# Roadmap

Build order of the platform: what is built and what comes next. Continue with the first unchecked item.

One item is one piece of about 15 minutes; tick it in the commit that finishes it; designs are in `docs/`.

## Order of work

- Structure before additions: work that changes how abilities, characteristics, events, costs or views are shaped runs first and alone; work that fills one stub (a keyword, an effect) follows in any order.
- A cluster is the items that touch one area of code. They run back to back, and a decision kind is built with the first task that asks it.
- The card system (M3) and Commander (M4) are complete in the DSL, engine and protocol types before the web shows them (M5), so each screen is built once. Until then a new event, decision or view field gets a placeholder in the web that keeps typecheck and e2e green.
- Rules data leads the engine: support is computed, so a compiled card is Partial until the engine runs its primitives and then Verified without an edit. The coverage report ranks what holds cards back, and clusters that do not need each other (`docs/cards-dsl.md`, Tasks) run in that order.
- A cluster ends with its corpus cards Verified, the compiler templates and goldens for its wordings, a fuzz deck under every invariant, hidden-information tests for its events and current generated bindings.
- Items that depend on nothing (import checks, account settings, deploy) wait in their milestone and can be taken at any time without rework.

## Done

- Foundations: workspace, lint rules, CI, `CLAUDE.md`, local Postgres, the React stack (`docs/spike-react.md`)
- Card data: `mtg-import` (Scryfall, MTGJSON decks, deck-list parsers, name resolution), `mtg-db` (schema, upserts, card search, precons), `mtg-cli` (fetch, ingest) and the card search endpoints
- Engine: zones, turns, priority, stack, combat, bots and `mtg-testkit`; combat forecast and block preview
- Protocol and server: wire types with generated bindings and a drift test, the game actor, axum transport with a WebSocket per seat, table-state sync, persistence and resume with deck fingerprints, accounts, "my games", invite-link lobbies, open games and join by code
- Game flow: auto-pass, response clocks, yields, stops and Full Control, idle windows as short hidden clocks, leak tests (`docs/arena-flow.md`, P1 to P8)
- Web: core client, game UI with prompts for all eight decision kinds, N-seat table with log and stack panel, table animations, concede, lobby, account pages, Playwright e2e; the UI rebuild U1 to U12 (tokens, fit engine, grid, controls dock, turn feedback, game browser, layout gate)
- Decks: list, import, export, builder, and the deck check with the Verified marker
- Card system so far: the DSL, card assembly and effect pipeline (`docs/cards-dsl.md`); the keywords of combat, hexproof and flash; triggers with APNAP order, delayed triggers and intervening if; pumps and keyword grants through layers 6 and 7c; discard, sacrifice, mill, the library effects, destroy, exile, counter, tap, untap, conditions, winning the game, pay, search, tokens (Treasure, Clue, Food), modes, yes or no and options

## M3 Card system

Design and task order: `docs/cards-dsl.md` (Tasks). The clusters run in the order listed.

### Compiler and coverage
- [x] (r1) Compiler core in `mtg-cards`: reminder text and the card's name stripped, clauses split and classified (keyword line, spell, triggered, activated, loyalty, static, modal), keyword lines read, an unreadable clause kept as `Ability::Manual` with its span (`docs/cards-dsl.md`, Compiler and Manual fallback)
- [x] (r1b) A Scryfall keyword counts as present in `check_printed` when an ability has it or a `Manual` clause names it, so compiled cards that grant a keyword or use a typecycling form validate (`docs/cards-dsl.md`, Validation)
- [x] (r2) Reader framework and renderer: a template reads a clause into the DSL and renders it back as English; goldens for each template, and a ratchet over the corpus (every ability the compiler reads equals the hand-written one, and the cards it reproduces whole never shrink in number)
- [x] (r3a) Templates: plain mana abilities and spell effects (draw, life, damage, destroy, exile, counter, scry, pump) with their common targets and amounts
- [x] (r3b) Templates: triggers (enters, dies, leaves, attacks, cast, step, sacrifice) and "you may"; a line without a period no longer blocks the effects of its spell
- [x] (r3b2) Templates: modes ("choose one"), "pay" ("unless its controller pays"), compound predicates ("discards a card and loses 3 life"), the older "enters the battlefield" wording, and the effects "exile ~", "return target creature to its owner's hand" and "all creatures get -N/-N"
- [ ] (r3c) Templates: activated abilities with `{T}`, mana and sacrifice costs, and the wordings of search and the library effects
- [x] (r4) `mtg-cli coverage`: the state of every precon card (and of each precon) from its compiled rules and the engine's `Caps`, with the unread wordings and missing primitives ranked by the cards each holds back and by the cards it alone holds back
- [ ] (r4b) Audit before any compiled rule reaches a game: 100 random compiled cards are listed as oracle text beside the rendered reading and the JSON, and each disagreement fixes a template
- [ ] (r5) The vocabulary the report ranks highest and the DSL lacks (value nodes such as devotion, energy and chosen colour; effects; costs; trigger patterns) is added in one pass, each with its `Caps` name and a stub
- [ ] (r1c) Generated rules stored as `card_rules` rows next to the hand-written files and read by the server; `mtg-cli compile` writes the ones that validate; a compiled mode without a label shows its rendered text in the mode prompt

### Small effects
- [x] (q) `Effect::If` with its conditions, `Tap`, `Untap`, `Mill`, `WinGame` and the keyword Flash; Felidar Sovereign is Verified

### Abilities and characteristics
- [x] (k1) One reader of an object's abilities with stable addresses: `abilities_of` and `ability` in `ability_reader` read the printed abilities of an object
- [x] (k2) Priority options, trigger detection, casting and resolution read abilities through it
- [x] (k3) Static abilities gathered at every derivation; every `Modification` runs as an effect and as a static (types, subtypes, colours, set and switched power and toughness, abilities added and lost, control, can't attack or block); the reader lists granted and lost abilities, and the can-anything-trigger flag follows abilities that appear in play (tokens, granted abilities)
- [ ] (k3b) `Effect::Rule` with its game rules (must attack or block, can't be countered, can't gain life, draw or search, trigger doubling, reveal top)
- [x] (k4a) The derivation of CR 613 as one pass: dependencies (CR 613.8), characteristic-defining abilities in every zone (the `defining` flag of `Static`), static abilities that read the other objects as the earlier layers leave them, static abilities that effects give; Nightmare, Maro, Transguild Courier and Kormus Bell are Verified
- [x] (k4) Keywords in snapshots (those after layer 6, none that an effect removed), read by last-known information: the damage of a source that has left, conditions on a keyword
- [x] (e2) Keyword abilities built from triggers: ward, prowess and battle cry come from `adds_abilities`, listed by the reader under their own addresses; Hero of Bladehold, Monastery Swiftspear and Hulking Raptor are Verified
- [ ] (i3) State triggers (CR 603.8) and abilities that work from a zone other than the battlefield (`from`)

### Counters, planeswalkers and attachments
- [ ] (l1) Counters on permanents and players: the fold, put, remove and move, `Amount::Counters`, +1/+1 and -1/-1 counters in layer 7c with their state-based actions, views; Tireless Tracker is Verified
- [ ] (l2) Planeswalkers: loyalty abilities (once a turn, at sorcery speed), damage removes loyalty (CR 120.3c), zero loyalty dies, an attacker keeps attacking a planeswalker that left (CR 506.4c); Jace, the Mind Sculptor is Verified
- [ ] (m1) `Attach`, Aura spells that target through `Enchant`, and the state-based actions of attachments (CR 704.5m to 704.5p); Pacifism is Verified
- [ ] (m2) Equipment with `Equip` and `Reconfigure`; Bonesplitter is Verified

### Replacement and prevention
- [ ] (j1) The replacement step: `Ability::Replacement` found through the reader, one `ReplaceWhen` for each event it can replace, self-replacement first, once per event (CR 614 to 616), and `ChooseReplacement` in engine and protocol; Anointed Procession is Verified
- [ ] (j2) Enters tapped or with counters; a choice made as it enters and remembered on the permanent
- [ ] (j3) Damage prevention shields, regeneration and `GameRule::DamageCantBePrevented`

### Costs and casting
- [ ] (g1) One announcement procedure (CR 601.2): modes, X, optional, alternative and additional costs, targets and division, then the total cost; `ChooseNumber`, `ChooseCosts` and `DivideAmount` are built with it
- [ ] (g2) `{X}` in spell and ability costs; the support walker names `Cost.Mana.X`
- [ ] (g3) Optional and additional costs: kicker and multikicker; Burst Lightning is Verified
- [ ] (g4) Alternative costs and cast permissions: flashback, evoke, overload, madness, suspend, casting from another zone, without paying the cost or as though it had flash, cascade; Mulldrifter, Faithless Looting and Bloodbraid Elf are Verified
- [ ] (g5) Cost modifiers (`Ability::CostModifier`): reductions, increases, convoke, delve; Fireball is Verified
- [ ] (g6) Costs of activated abilities beyond `{T}`, mana and sacrificing the source: a chosen permanent (chosen when the ability is announced), life, discard, counters removed, a card exiled from a graveyard, untapped creatures tapped; cycling; a card's support counts the abilities of the predefined tokens it makes; Flooded Strand is Verified
- [ ] (g7) Mana: abilities of several mana or counting something, restrictions on how mana is spent, Treasure and other sacrifice sources offered while paying a spell; Sol Ring and Gaea's Cradle are Verified
- [ ] (g8) A `Pay` the player cannot afford is still asked, with No as the only answer, so the pace does not show whether they could pay (`docs/arena-flow.md`)

### Trigger families
- [ ] (i4) Patterns for every event the engine logs (damage, life, draw, discard, mill, tap, untap, counters, attach, cast, cycle), batches ("one or more"), first time each turn, the nth card, event amounts ("that much")

### Faces and copies
- [ ] (n1) Multi-face cards: split, adventure, modal double-faced, transform and meld; the face is chosen at announcement, `Card.Faces` is lifted, `Effect::Transform`; Delver of Secrets, Bonecrusher Giant and Fire // Ice (fuse) are Verified
- [ ] (n2) Face-down objects (morph, disguise, manifest, cloak) and the face-down rule in `zone_is_visible`
- [ ] (o) Copies: a token that copies an object, spell copies, enters as a copy, populate, storm; copiable values as the first layer

### Manual cards and views
- [ ] (p) Manual cards: a card the engine does not run is cast and resolves without effects, and its controller applies generic logged actions (move, counters, life, token, tap) through the same primitives, visible to every seat
- [ ] Views complete: they carry every state of an object (counters, attachments, face, face-down, copies, support), a spectator audience sees public information, and a card revealed and then moved to a hidden zone in one batch reaches the other seats' views (`docs/protocol.md`)
- [ ] Gate: every corpus card is Verified except the two kept Partial and Manual by design, and fuzz games with a deck from each cluster pass every invariant

## M4 Commander
- [ ] `Commander` format rules: 100-card singleton, colour identity, 40 life, command zone, tax as a cost modifier, return to the command zone (CR 903.9), 21 commander damage; the views carry commander damage and tax so the pod can show them
- [ ] Fix: when the active player leaves mid-turn the turn continues without an active player (CR 800.4; `docs/engine.md`), with a scenario test
- [ ] Partner, companion, Commander-specific tests and Commander-format fuzz games with 4-6 players

## M5 Table and prompts

The server is settled first, then the web is built once against the final types.
- [ ] Response clocks for every decision kind without one, each with a fallback answer, and a clock test through the actor for YesNo, ChooseOption and ChooseMode (`docs/server.md`, Response clocks)
- [ ] (d9) Timer bank: extensions earned and spent at expiry, a delay before the countdown shows, an optional clock for the active seat
- [ ] (i2) Arena flow P10: smart stops, per-ability yields, an order-by-timestamp preference, damage auto-assign and the engine's answer to a one-option decision (`docs/arena-flow.md`)
- [ ] Prompts for ChooseNumber, DivideAmount, ChooseCosts, ChooseReplacement, scry and the order of cards put back; the payment prompt for X and restricted mana
- [ ] Cast options in the table: face, optional and alternative costs, modes, X
- [ ] Table states: counters, attachments tucked beside their host, loyalty, faces and face-down cards, copies, command zone and commander damage, support marks; each in the `/dev/table` fixture with screenshots at six sizes in both themes
- [ ] Manual actions on any card: a menu and log entries
- [ ] (U5b) One step ladder: the ladder that shows the turn also sets the stops (click a step), as in Arena, and the second row of step buttons goes
- [ ] Playwright games with corpus decks that ask every decision kind

## M6 Coverage
- [ ] Choose the set of Commander precons that is the coverage target; `mtg-cli coverage` ranks over all stored precons until then (`docs/deck-check.md`)
- [ ] Coverage dashboard in the web over `Library::support`: states, reasons and clause spans per card, ranked by saved-deck demand; target: 100% of the chosen precons (`docs/deck-check.md`)
- [ ] (h) Keywords, keyword actions and wordings the DSL and the compiler lack, sagas, classes, designations, extra turns and the other long-tail mechanics: one task each (primitive, template, golden), in the order of the report's demand

## M7 Product
- [ ] (B3) Deck check, remaining: each precon's state in the precon browser (`docs/deck-check.md`)
- [ ] Precon browser with one-click play
- [ ] Choose a deck in the lobby, remaining: precons as a third kind of deck (`docs/server.md`, `docs/deck-check.md`)
- [ ] Verify Moxfield, Archidekt, MTGO and Arena export parsers against real exports: the 29-deck corpus passes; open: a real MTGO `.dek`, an Arena client export, companions, custom cards (`docs/data-sources.md`)
- [ ] Importer 2b, name resolution on the corpus: by printing (set + collector number) before name, plus flavor, digital and multi-face names (`docs/data-sources.md`); a real-data test resolves all 183 exports
- [ ] Spectators in the web and a "waiting on" message for tables of three or more
- [ ] Account settings: change the password, delete the account (its seats are freed); a game that failed to resume shows in My games with its reason
- [ ] Layout gate for the pages that need an account (Decks, My games, the builder with a real deck), in `accounts.spec.ts`

## M8 Polish and deploy
- [ ] Animations beyond the table feedback (zone moves to hand, exile and library, a spell on the stack; `docs/web.md`), sounds, replays, solo play against bots
- [ ] Dockerfile, Kustomize manifests for k3s, Postgres, ingress + TLS on the user's domain (`docs/PLAN.md`, Deployment)
- [ ] Performance profile on real hardware with real card art
