# Roadmap

Build order of the platform: what is done, what is open and what comes next. Continue with the first open piece of "Next up".

One item is one piece of about 15 minutes; the commit that finishes it deletes its line, turns a leftover into a new lettered item and names a new area under "Done"; designs are in `docs/`. Only open items are listed in the milestones.

## Status

- Built: foundations, card data, the engine core, protocol and server, game flow, the web client and the deck builder (M0 to M2), and Commander in the engine, server and lobby (M4).
- In progress: the card system and its coverage tail (M3, M6): 2,068 of the 6,429 stored precon cards (32.2%) and 59.2% of the copies are Verified; the open pieces are a flat long tail that frees a few cards each.
- Not started: the web for the final card-system types (M5), the product pages (M7), polish and deploy (M8).

## Order of work

- Structure before additions: work that changes how abilities, characteristics, events, costs or views are shaped runs first and alone; work that fills one stub (a keyword, an effect) follows in any order.
- A cluster is the items that touch one area of code. They run back to back, and a decision kind is built with the first task that asks it.
- The card system (M3) and Commander (M4) are complete in the DSL, engine and protocol types before the web shows them (M5), so each screen is built once. Until then a new event, decision or view field gets a placeholder in the web that keeps typecheck and e2e green.
- Rules data leads the engine: support is computed, so a compiled card is Partial until the engine runs its primitives and then Verified without an edit. The coverage report ranks what holds cards back, and clusters that do not need each other (`docs/cards-dsl.md`, Tasks) run in that order.
- A cluster ends with its corpus cards Verified, the compiler templates and goldens for its wordings, a fuzz deck under every invariant, hidden-information tests for its events and current generated bindings.
- Items that depend on nothing (import checks, account settings, deploy) wait in their milestone and can be taken at any time without rework.
- A piece is claimed before it starts: its `[ ]` becomes `[~]` with the branch name, the commit is pushed to origin, and agents on other machines then see it taken; the commit that finishes it (it deletes the line) is pushed too. Local `main` is pushed after every merge, so `origin/main` is what worktrees and other machines start from.

## Next up

Ready pieces, grouped by the code they touch. A group runs one piece at a time, groups run side by side (about six agents at once); a piece is claimed with `[~]` and pushed before it starts, and `git merge main` plus the full `scripts/check.sh` end it.

- Costs and casting: (g2e), (g5c), (g6d), (c1h), (c12b), (c13b).
- Cast ways and zones: (c5b), (c2e), (n2) face-down objects, (n1d3) meld.
- Triggers and keywords: (c6b), (c4b), (c3b), (c3d), (c2c), (c2d), (n1d5) day and night effects, (o2c).
- Statics and layers: (c8), (c7b), (c7c), (o1d), (m4).
- Compiler wordings, no engine change: (c10) one piece per wording family with (c10c) to (c10e) and (c10r), (o2e), (c11) and (c11c) layouts, (c9c), (c1i).
- Commander and server: (hr5b), the `manual_play` flag stored with the game, the turn without an active player in the table view, Companion.
- Tooling: (dep) dependencies, alone between rounds; (t1) a flaky test.
- After the card system: "Views complete", the Gate, then the web (M5 prompts for the decision kinds the engine asks, the manual-action menu, table states) and M7.
- After each round: `mtg-cli ingest` and `compile` refresh the real database, `coverage` is re-run and the numbers go on the M6 target line; `fetch scryfall --printings` fills the printings once.

## Done

- Foundations: workspace, lint rules, CI, `CLAUDE.md`, local Postgres, the React stack (`docs/spike-react.md`)
- Card data: `mtg-import` (Scryfall, MTGJSON decks, deck-list parsers, name resolution by printing), `mtg-db` (schema, upserts, card search, precons), `mtg-cli` (fetch, ingest, compile, coverage) and the card search endpoints
- Engine: zones, turns, priority, stack, combat, bots and `mtg-testkit`; combat forecast and block preview
- Protocol and server: wire types with generated bindings and a drift test, the game actor, axum transport with a WebSocket per seat, table-state sync, persistence and resume with deck fingerprints, accounts and settings, "my games", invite-link lobbies, open games and join by code
- Game flow: auto-pass, response clocks, yields, stops and Full Control, idle windows as short hidden clocks, the timer bank, leak tests (`docs/arena-flow.md`, P1 to P8)
- Web: core client, game UI with prompts for all eight decision kinds, N-seat table with log and stack panel, table animations, concede, lobby, account pages, Playwright e2e; the UI rebuild U1 to U12 (tokens, fit engine, grid, controls dock, turn feedback, game browser, layout gate)
- Decks: list, import, export, builder, and the deck check with the Verified marker; the importers pass the 29-deck corpus
- Compiler: reader framework and renderer with goldens and a corpus ratchet, templates for mana, spell, trigger, activated, modal, pay, search, copy and static wordings, value nodes, `mtg-cli coverage` with its audit, generated rules stored as `card_rules`, one Scryfall fixture per corpus card
- Cast and limit wordings: triggers on subtype, permanent, multicolored and historic spells, "This ability triggers only once each turn.", and counts of the times a commander was cast (`Amount::CommanderCasts`)
- Abilities and characteristics: one reader of an object's abilities, the derivation of CR 613 in one pass, `GameRule` effects, keyword abilities built from triggers, state triggers and abilities that work from other zones
- Counters, planeswalkers and attachments: counters on permanents and players, loyalty abilities, the legend rule, Auras, Equipment, Reconfigure and living weapon
- Replacement and prevention: the replacement step, enters tapped or with counters, damage shields, regeneration, protection
- Costs and casting: the announcement of CR 601.2 with modes, X, optional, alternative and additional costs, kicker, multikicker, buyback, entwine, cascade, discover, madness, suspend, foretell, cost modifiers, convoke, delve, life, counter and counted costs, divided amounts, mana abilities with restrictions, filter lands, triggered mana and mana in any combination, required additional costs, free casts and Strive
- Triggers: patterns for every logged event, batches, the nth card or spell, "first time each turn", evasion, impulse draw, the Monarch and the city's blessing
- Keywords: combat, characteristic and evasion keywords; Surveil, Proliferate, Explore, Connive, Amass, Monstrosity, Adapt, Bolster, Fabricate, Learn, Ascend; Crew, Saddle, Station, Level up, Outlast; Unearth, Escape, Rebound, Aftermath, Dash, Ninjutsu, Jump-start, Retrace, Embalm, Eternalize, Blitz, Encore, Dredge, Miracle, Prowl, Demonstrate, Hideaway; Echo, Cumulative upkeep, Undying, Persist, Myriad, Squad, Ravenous, Vanishing, Fading, Backup, Devour, Casualty, Offspring, Gift; Curses, Bestow, Mutate, Morph, Megamorph, Disguise; Split second, Undaunted, Improvise, Affinity, typecycling, Scavenge, Transmute, Awaken, Gravestorm, Sunburst, For Mirrodin!, Job select, restricted Equip; Partner and its forms
- Statics: Auras that enchant a player, lords by subtype, characteristic-defining abilities, conditions after "as long as", the tax on attacking, block restrictions and granted mana
- Faces and copies: multi-face cards, transform, fuse, daybound and nightbound, Sagas, Classes, flip cards, copiable values, token and spell copies, copies of abilities
- Commander: house rules (turn, life, table and Commander options) on the server and in the lobby, the Commander format rules in the engine, the deck check for a table, Partner, "can be your commander", commander replacement of a draw, the mid-turn departure of the active player
- Tooling: `scripts/check.sh` with test slots, `scripts/agent-status.sh`, the quick test-time suite, derived corpus counts, current Cargo dependencies

## M3 Card system

Design and task order: `docs/cards-dsl.md` (Tasks). The clusters run in the order listed.

### Costs and casting
- [ ] (g2e) The mode prompt of a spell or ability with X in a mode's target filter offers a mode only when some payable X has its targets, so choosing a mode never backs the cast out to priority
- [ ] (g5c) Convoke and delve are shown to every seat by the protocol projection (a test over `mtg-protocol` hidden-information helpers)
- [ ] (g6d) Costs of multikicker, suspend, overload and foretell with parts other than mana, a Verified card for a `Pay` with a part (Arrogant Poet) and for a ward with a cost other than mana (Owlin Shieldmage, Sunset Saboteur), the compiler reading "Ward—Sacrifice a creature", "sacrifice ~ unless you pay {G}" (a self-sacrifice effect), a hidden-information test of the picks of a `Pay` at two, three and four seats, a card that removes X counters (needs a negated amount `Amount::Neg` for "-X/-X", Skullmane Baku, or X targets)

### Faces and copies
- [ ] (n1d3) Meld (CR 701.37, 712.4): `melds_into` reaches `PrintedCard` and the definition, the melding effect exiles both cards and returns them as one permanent with the combined back face, the permanent's two cards leave as separate objects, and the walker names of meld; Bruna, Gisela and Brisela are Verified (records in `crates/mtg-import/tests/fixtures/scryfall_meld_pair.json`)
- [ ] (n1d5) Day and night effects: the DSL has no node for "it becomes day" or "if it's neither day nor night" (`Effect` that sets the designation, `Condition` on it), the engine runs them, and the compiler reads their sentences with goldens
- [ ] (n2) Face-down objects, remaining: manifest and cloak (a `FaceDown` kind each), morph costs with a part besides mana (Fathom Seer, Gift of Doom) or an {X} (Bane of the Living), and the reveal of a face-down permanent that returns to a hand or library
- [ ] (o2c) A storm trigger or copy effect that resolves after its spell has left the stack copies it from last known information (the snapshot keeps the announced choices)
- [ ] (o2e) The compiler and engine read the copy wordings left: the cast trigger of the card itself ("When you cast this spell", a `SpellCast` of `This` from the stack) that holds back the storm cards whose copy count now reads (Echo Storm, Empyrial Storm, Fury Storm, Hatut Zeraze Strike Force, Skull Storm, Genesis Storm), the exceptions "isn't legendary" (Helm of the Host, Living Laser, Quantum Misalignment, Tempestra, Loki, the Deceiver) and "loses all other card types" (Imprisoned in the Moon, Espers to Magicite, Imposter Mech) as a `Modification` that removes a supertype or sets the types, "when you next cast" (Galvanic Iteration, Solar Array, Magus Lucea Kane, Thunderclap Drake) as a delayed trigger on a cast, and the spells "that's two or more colors" (Ancient Cornucopia) and "noncreature or Dragon" (Firespitter Whelp)
- [ ] (o1d) A transformed permanent that dies reads its trigger from the face it showed, and a static that a copy granted reads it from the copy when the copy left in the same batch (last known abilities, as (o1c) does for a copy that dies)

### Attachments
- [ ] (m4) A Fortification that is also a creature (CR 301.6): the attachment check and a test; the corpus has no Fortification

### Manual cards and views
- [ ] The game's `manual_play` flag is stored with the game and listed by `GET /api/lobbies` and `GET /api/games/{id}` (`mtg_protocol::manual_play` derives it from the roster today)
- [ ] Views complete: they carry every state of an object (counters, attachments, face, face-down, copies, support) and the abilities a player may activate with their cost and target wording (Equip, Reconfigure, loyalty and other activated abilities in one list), a spectator audience sees public information, and a card revealed and then moved to a hidden zone in one batch reaches the other seats' views (`docs/protocol.md`)
- [ ] Gate: every corpus card is Verified except the two kept Partial and Manual by design, and fuzz games with a deck from each cluster pass every invariant

## M4 Commander
- [~] (hr5b) (branch hr5b-precon-check) The deck check for a table takes `precon:<code>` as its deck without an account, and the web's lobby picker shows each deck's verdict at the table (`game`) before joining
- [ ] The table view says when a turn has no active player (`TurnView.active` names the seat that left; the turn indicator and the step bar show a turn without one), with a projection test
- [ ] Companion (CR 702.139): the companion is a card from outside the game that costs {3} to put into the hand; no stored precon has a companion board

## M5 Table and prompts

The server is settled first, then the web is built once against the final types.

- [ ] (i2) Arena flow P10: smart stops, per-ability yields, an order-by-timestamp preference, damage auto-assign and the engine's answer to a one-option decision (`docs/arena-flow.md`)
- [ ] Prompts for ChooseNumber, DivideAmount, ChooseCosts, ChooseReplacement, scry and the order of cards put back; the payment prompt for X and restricted mana and for convoke and delve
- [ ] Clock words in the web (`clockText.ts`) for every decision with a clock the web does not name: number, optional costs, division, replacement, keep, bottoming, payment, damage, attackers and discard
- [ ] The `YesNo` prompt disables Yes and its key when `YesNoOptions.only_no` is set (a payment the seat cannot make)
- [ ] Cast options in the table: face, optional and alternative costs, modes, X
- [ ] Attack and block prompts mark the creatures a rule requires (`AttackOptions.required`, `BlockOptions.required`) as chosen and locked, and a seat panel shows a revealed library top (`PlayerView.revealed_top`)
- [ ] The forecast and block preview mark numbers that depend on a choice (`CombatForecast.depends_on_choice`)
- [ ] Table states: counters, attachments tucked beside their host, loyalty, faces and face-down cards, copies, command zone and commander damage, support marks; each in the `/dev/table` fixture with screenshots at six sizes in both themes
- [ ] Manual actions on any card: a menu and log entries
- [ ] (U5b) One step ladder: the ladder that shows the turn also sets the stops (click a step), as in Arena, and the second row of step buttons goes
- [ ] Playwright games with corpus decks that ask every decision kind
- [ ] (c5h) The table's event text has a line for the rule `MustAttackPlayers` (Encore) instead of the generic trigger line, with a test

## M6 Coverage

### Target
- [ ] Target: every stored card Verified, or as near as the rules allow; `mtg-cli coverage` (compile first) ranks the blockers, each piece below states the cards it frees, and after each round `compile` and `coverage` are re-run and the numbers recorded here (2,068 of the 6,429 stored precon cards, 32.2%, and 59.2% of the copies are Verified)
- [ ] Choose the set of Commander precons that is the coverage target; `mtg-cli coverage` ranks over all stored precons until then (`docs/deck-check.md`)

### Mana and costs
- [~] (c1h) (branch c1h-mana-once) A mana ability with no tap is used more than once in one payment (Cadaverous Bloom exiling two cards for {B}{B}{G}{G}); a payment uses each source once
- [ ] (c1i) The mana ability wordings still unread: storage-counter lands ("Remove X storage counters from this land: Add X mana in any combination of {R} and/or {G}": an X cost in a mana ability), a combination counted by X ("Add X mana in any combination of colors, where X is ..."), restrictions by subtype or by a use of abilities ("Spend this mana only to cast Elemental spells or activate abilities of Elementals"), and the triggered mana abilities of a nonland permanent (Kinnan) or with a condition (the monarch)
- [ ] (c12b) The rest of the additional costs of a spell: a choice between costs ("discard a card or pay 3 life", "sacrifice a creature or pay {3}"), an X ("pay X life", "discard X cards"), mana in a required cost and the read of what the cost took ("the sacrificed creature's power")
- [ ] (c13b) The rest of the commander free casts and strive spells: Deflecting Swat (choose new targets for a spell or ability on the stack) and the strive spells Launch the Fleet, Solidarity of Heroes, Call the Coppercoats and Twinflame (their effects are not read)

### Keywords and cast ways
- [ ] (c2c) Improvise granted by a static ability (Inspiring Statuary) and affinity for historic permanents and for Equipment (Banish to Another Universe, Goldwardens' Gambit)
- [ ] (c2d) The cards that print scavenge, transmute, awaken, gravestorm or sunburst and wait on another wording: Brainspoil ("isn't enchanted", "can't be regenerated"), Dizzy Spell, Netherborn Phalanx, Bitter Ordeal (exile from a searched library), Spinal Parasite, Sawtooth Thresher and Heliophial (counter-removal costs with their effects), Ondu Rising, Encircling Fissure, Roil Spout, Rush of Ice, Part the Waterveil and the other awaken spells with unread effects, Dodgy Jalopy (a Vehicle with scavenge), Boneyard Mycodrax and the Modular—Sunburst artifacts
- [~] (c2e) (branch c2e-mayhem) Mayhem (`CastWay::Mayhem`, a tally of the cards discarded this turn, open from the graveyard), Afterlife, Spectacle, Horsemanship, Harmonize and the other keyword lines the coverage report ranks next; the engine keywords with a fuzz campaign over For Mirrodin!, Job select and the restricted equips
- [ ] (c3b) Keyword actions and designations, remaining: Time travel (time counters on suspended cards and permanents) and the blessing badge on the table
- [~] (c3d) (branch c3d-monarch) The monarch cards still held back: "can't attack you" (Fealty to the Realm, Queen Mother Ramonda, with (c8)), damage that does not cause life loss (Archon of Coronation), "your commander" as a source (Archivist of Gondor, Paliano), the attacked player as "that player" (M'Baku), "as the turn began" (Knights of the Black Rose), "until an opponent becomes the monarch" (Palace Jailer) and "while you're the monarch" after an event (Regal Behemoth, Tamiyo)
- [ ] (c4b) The "becomes an artifact creature until end of turn" shapes ("Tap two untapped tokens you control: This Vehicle becomes ...", "As long as this Vehicle has three or more fire counters on it, it's an artifact creature"), the events "becomes crewed" and "becomes saddled", the creatures that saddled a Mount (Rambling Possum), and a Reconfigure corpus card
- [ ] (c5b) The compiler's reading of the hideaway sentences ("you may play the exiled card without paying its mana cost" with its conditions, "play one of them", "if you do, return this Vehicle"), the conditions "you attacked with three or more creatures this turn" (Windbrisk Heights) and "an opponent was dealt 7 or more damage this turn" (Spinerock Knoll), Cipher, Miracle with X in its cost and the miracle spells the compiler does not read yet (Terminus, Banishing Stroke, Devastation Tide, Reforge the Soul, Redress Fate)
- [ ] (c6b) The count of permanents Devour sacrificed ("for each creature it devoured", Thromok), Octomancer (a copy of a token that entered this turn), Perch Protection (phasing out, a life total that can't change, protection from everything), the two targets of Wear Down and Peerless Recycling that may be the same object, and the compiler reading the gift spells that say "instead" or add a target when the gift was promised

### Statics, attachments and mutate
- [ ] (c7b) Umbra armor (4), the Aura grants of an ability ("enchanted land has '{T}: Add ...'", "whenever enchanted land is tapped for mana"), "when this Aura enters" attach triggers, Curse of Inertia (the attacking player chooses the target) and the unread statics and triggers of Conformity, Fool's Wisdom, Clinging Webs and Restless Dead
- [ ] (c7c) The rest of mutate: "the number of times this creature has mutated" (Archipelagore, Auspicious Starrix, Insatiable Hemophage), a merged permanent under a copy effect keeps the abilities of its other cards, a merged permanent that has left is copied by its topmost card alone, a merged permanent that returns to the battlefield enters each card with its enter triggers, a commander card of a merged permanent under the hand and library replacement, and a token or a face-down creature as the top of a merge
- [~] (c8) (branch c8-statics) Statics still unread: cost reductions for a party or a quantity of the turn, Strive's ability word, the tax on attacking in life ("pays 2 life for each", Sivitri), by a count of basic land types (Collective Restraint) or on blocking (Archangel of Tithes), granted keywords the engine lacks (melee, riot, myriad), "You have hexproof" and "You have shroud", granted triggered and static abilities in quotes ("All Slivers have \"When ...\""), the block restrictions "can block only creatures with flying", "can't be blocked by more than one creature" and "can't be blocked except by two or more creatures", a subtype group after a leading condition (Detective of the Month, Raksha Golden Cub), conditions on equipment attached to a host or an Aura on it (Balan, Timber Paladin), on the spells cast or the attacks made this turn (Lyse Hext, Agent Frank Horrigan), on an opponent's zone or on life above the starting total (Soaring Thought-Thief, Righteous Valkyrie) and "as long as you control your commander" (Lieutenant, with melee), "protection from each color", "also get", counts of a subtype's cards ("Elf cards in your graveyard") and of two zones in a characteristic-defining ability, life-gain replacements under a life total or by another effect ("draw that many cards instead", "that player loses that much life instead")

### Compiler wordings
- [ ] (c10) Unread spell, trigger and activated wordings ranked by the cards each holds back (`mtg-cli coverage`, the unread lists; a spell line such as "Draw a card." or "Exile ~." is listed beside the unread line that holds the card back): one piece per wording family left (tokens with quoted abilities, choose and vote (a vote effect, the tally by option, "Will of the council" and "Council's dilemma" wordings, the no-op planeswalk and chaos of "Will of the Planeswalkers", "While voting, you may vote an additional time"), the goad wordings (c10p) leaves ("goad each creature that player controls", "that player draws a card" after a goad, "must be blocked if able" beside "is goaded", "goaded creatures ... can't block", "goaded for the rest of the game", a goad of a creature the sentence did not name), counters ("Remove all", "each of up to N target creatures", "then proliferate", doubling, "that creature" after damage), pump ("loses all abilities and becomes", "Those creatures gain"), the manlands (c10n) leaves (a land that stays a creature ("that's still a land"), an X/X size, "Activate only if", a snow or reduced cost, a quote without its period before "until end of turn", a quoted static, "target land you control becomes"), "where X is its power" for mana and X/X tokens (a token's size as an amount), mill and graveyard ("Exile target player's graveyard"), exile "until ~ leaves the battlefield" with its return (engine), "sacrifice it unless {M} was spent to cast it" (engine), the graveyard wordings left ("Activate only during your upkeep" and "Activate only if you attacked this turn" on an activation from the graveyard (a step and an attack condition, engine), a return "attached to target creature" or "with a +1/+1 counter", "return it to its owner's hand at the beginning of the next end step" after a death), flicker with a delayed return, two targets of one kind, a count of what a destroy did, the damage and life wordings (c10i) leaves: "half their life", "lose life equal to ...", a second target, and the draw and discard wordings left: "each player discards their hand, then draws N cards" (a hand count per player, engine), "Draw N cards and create N Treasure tokens", "each other player's draw step"), each with its templates, goldens and Verified cards
- [~] (c10c) (branch c10c-counted) The counted amounts (c10f) leaves: "where X is the number of ..." and "equal to the number of ..." for draws, damage, life, pumps and mana; a count held by a target or another player ("creature target opponent controls", "card in their graveyard", "in your opponents' graveyards"); a quantity ("different power", "kind of counter", "colors among"); a count of what the resolution did ("creature destroyed this way", "time it was kicked", "spell you've cast this turn"); "counter on ~" of any kind; "for each opponent, ..." as one step per player
- [ ] (c10d) The wordings of "the top card(s) of your library" that (c10g) leaves: "Reveal the top card ... If it's a land card" with a follow-up, "each player / target opponent reveals or exiles the top card(s)", a count of X, "Then put the rest", a chosen type, a cast or an exile from among them, "manifest" and "cloak" the top card, and the riders after "You may cast ... spells from the top of your library"
- [ ] (c10e) The wordings (c10b) leaves: subtype plus type nouns ("artifact creature", "Human creature"), legendary names without a comma shortened to `~`, and the events "enters from your graveyard", "is turned face up", "is put into a graveyard from the battlefield" and "modified creature"
- [ ] (c10r) The flicker with a delayed return ("Exile target creature you control. Return it to the battlefield under its owner's control at the beginning of the next end step", also an exile that returns at the next end step of its owner), the same exile that returns when the source leaves for an Aura with several hosts (the owner chooses) and "until your next turn" durations
- [ ] (c9c) Verified corpus cards for Doctor's companion, Partner—text, Choose a Background (with a Background) and a planeswalker that can be a commander, each once its other text is a card the engine runs

### Layouts
- [ ] (c11) Rooms, Battles, Read ahead, the `prepare` layout (a "prepared" designation set by "enters prepared" and "becomes prepared", and a cast of a copy of the spell face from the battlefield that unprepares it, `docs/engine.md`), extra turns and extra combats (`Effect::ExtraTurn` is a stub), the initiative and dungeons, and the remaining long-tail mechanics found by the report, one task each
- [ ] (c11c) The other 24 transforming Sagas (their chapter wordings and back-face abilities that stay with the table, by the wording families of (c10)) and the wordings of the two precon flip cards (Budoka Gardener "put a land card from your hand onto the battlefield. If you control ten or more lands, flip", Nezumi Graverobber "If no cards are in that graveyard, flip") and the `prepare` layout (11 precon cards), which the engine plays as the front face only

### Tooling
- [ ] (t1) The `mtg-cli` `fetch_scryfall` tests are stable under parallel load (two of them failed once in a full run and passed alone and on rerun)
- [ ] The expected X symbols in `x_cards.rs` of `mtg-testkit` are derived from the card list, so a new X card edits no test
- [ ] Coverage dashboard in the web over `Library::support`: states, reasons and clause spans per card, ranked by saved-deck demand; target: 100% of the chosen precons (`docs/deck-check.md`)

## M7 Product
- [ ] (B3) Deck check, remaining in the web: each precon's state in the precon browser (the server lists it, `GET /api/precons`; `docs/server.md`, Precons)
- [ ] Precon browser with one-click play, remaining in the web: the page over `GET /api/precons` and a create with `deck: "precon:<code>"` (the server half is done; the Commander precons are `playable`, `docs/server.md`, Precons)
- [ ] House rules in the web: the create-game form sets them with the format's values as defaults, and the lobby row and the table name the ones that differ from the format; the Mulligan prompt names the style's hand size and scry
- [ ] Choose a deck in the lobby, remaining in the web: precons as a third kind of deck in the picker (the server takes `precon:<code>` as a seat's deck; `docs/server.md`, `docs/deck-check.md`)
- [ ] The importer fixtures hold real records: `scryfall_oracle_cards.json`, `scryfall_rulings.json` and `mtgjson_deck_commander.json` in `mtg-import` carry invented oracle ids and printings (the card fixtures of `mtg-cards` are checked by `fixture_real`); the tests of five crates that assert on their sets and counts follow, and the synthetic edge cases (a nameless record, art series, emblems) stay marked as synthetic
- [ ] Verify Moxfield, Archidekt, MTGO and Arena export parsers against real exports: the 29-deck corpus passes; open: a real MTGO `.dek`, an Arena client export, companions, custom cards (`docs/data-sources.md`)
- [ ] Spectators in the web and a "waiting on" message for tables of three or more

## M8 Polish and deploy
- [ ] Animations beyond the table feedback (zone moves to hand, exile and library, a spell on the stack; `docs/web.md`), sounds, replays, solo play against bots
- [ ] Dockerfile, Kustomize manifests for k3s, Postgres, ingress + TLS on the user's domain (`docs/PLAN.md`, Deployment)
- [ ] Performance profile on real hardware with real card art
