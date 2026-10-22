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
- Card system so far: the DSL, card assembly and effect pipeline (`docs/cards-dsl.md`); the keywords of combat, hexproof and flash; triggers with APNAP order, delayed triggers and intervening if; pumps and keyword grants through layers 6 and 7c; discard, sacrifice, mill, the library effects, destroy, exile, counter, tap, untap, conditions, winning the game, pay, search, tokens (Treasure, Clue, Food), counters on permanents and players (+1/+1 and -1/-1 in layer 7c, poison, energy, experience), planeswalkers (loyalty abilities, damage to loyalty), Auras and `Attach` with the state-based actions of attachments, the legend rule, modes, yes or no and options

## M3 Card system

Design and task order: `docs/cards-dsl.md` (Tasks). The clusters run in the order listed.

### Compiler and coverage
- [x] (r1) Compiler core in `mtg-cards`: reminder text and the card's name stripped, clauses split and classified (keyword line, spell, triggered, activated, loyalty, static, modal), keyword lines read, an unreadable clause kept as `Ability::Manual` with its span (`docs/cards-dsl.md`, Compiler and Manual fallback)
- [x] (r1b) A Scryfall keyword counts as present in `check_printed` when an ability has it or a `Manual` clause names it, so compiled cards that grant a keyword or use a typecycling form validate (`docs/cards-dsl.md`, Validation)
- [x] (r2) Reader framework and renderer: a template reads a clause into the DSL and renders it back as English; goldens for each template, and a ratchet over the corpus (every ability the compiler reads equals the hand-written one, and the cards it reproduces whole never shrink in number)
- [x] (r3a) Templates: plain mana abilities and spell effects (draw, life, damage, destroy, exile, counter, scry, pump) with their common targets and amounts
- [x] (r3b) Templates: triggers (enters, dies, leaves, attacks, cast, step, sacrifice) and "you may"; a line without a period no longer blocks the effects of its spell
- [x] (r3b2) Templates: modes ("choose one"), "pay" ("unless its controller pays"), compound predicates ("discards a card and loses 3 life"), the older "enters the battlefield" wording, and the effects "exile ~", "return target creature to its owner's hand" and "all creatures get -N/-N"
- [x] (r3c) Templates: activated abilities with `{T}`, mana and sacrifice costs, and the wordings of search and the library effects, and of removing and moving counters
- [x] (r4) `mtg-cli coverage`: the state of every precon card (and of each precon) from its compiled rules and the engine's `Caps`, with the unread wordings and missing primitives ranked by the cards each holds back and by the cards it alone holds back
- [x] (r4b) Audit before any compiled rule reaches a game: 100 random compiled cards are listed as oracle text beside the rendered reading and the JSON, and each disagreement fixes a template
- [x] (r5) Value nodes (devotion, greatest, total, card types, "if you do", the monarch, the city's blessing, mana of a choice, the chosen colour, a commander's identity), fourteen keyword-action effects, five activated-ability costs and eleven trigger patterns are in the DSL, each with its `Caps` name and a stub; the templates for surveil, proliferate, the monarch, a choice of mana, paying energy and the life, draw and discard events read them
- [x] (r3d) Templates for the unread wordings the report ranks highest whose primitives the DSL and the engine already have (enters tapped, also unless a count is controlled, enters with counters, a choice or a payment as it enters, can't be countered, can't block, attacks each combat, bonuses and keywords for an attached or a group of creatures, a land returned to the hand), each with goldens
- [x] (r5b) Can't be blocked (also by a kind of creature), no maximum hand size, an additional land play, looking at the top card any time, "can be your commander", choosing a player or an opponent as a permanent enters, the number of opponents and the lowest life total as values, and the keywords Crew, Changeling, Devoid, Mutate, Encore, Myriad, Squad and Infect are in the DSL, each with its `Caps` name and a stub; the templates read the statics, the choices, "enters tapped unless you have two or more opponents or a player has 13 or less life" and the keyword lines
- [x] (r1c) Generated rules stored as `card_rules` rows next to the hand-written files and read by the server; `mtg-cli compile` writes the ones that validate; a compiled mode without a label shows its rendered text in the mode prompt

### Small effects
- [x] (q) `Effect::If` with its conditions, `Tap`, `Untap`, `Mill`, `WinGame` and the keyword Flash; Felidar Sovereign is Verified

### Abilities and characteristics
- [x] (k1) One reader of an object's abilities with stable addresses: `abilities_of` and `ability` in `ability_reader` read the printed abilities of an object
- [x] (k2) Priority options, trigger detection, casting and resolution read abilities through it
- [x] (k3) Static abilities gathered at every derivation; every `Modification` runs as an effect and as a static (types, subtypes, colours, set and switched power and toughness, abilities added and lost, control, can't attack or block); the reader lists granted and lost abilities, and the can-anything-trigger flag follows abilities that appear in play (tokens, granted abilities)
- [x] (k3b) `Effect::Rule` and `Ability::Rule` with the rules of the game (must attack or block, can't be countered, can't gain life, draw or search, trigger doubling, reveal top), gathered once for a state; Darksteel Juggernaut, Supreme Verdict, Rampaging Ferocidon, Maralen of the Mornsong, Mindlock Orb, Goblin Spy and Panharmonicon are Verified
- [x] (k4a) The derivation of CR 613 as one pass: dependencies (CR 613.8), characteristic-defining abilities in every zone (the `defining` flag of `Static`), static abilities that read the other objects as the earlier layers leave them, static abilities that effects give; Nightmare, Maro, Transguild Courier and Kormus Bell are Verified
- [x] (k4) Keywords in snapshots (those after layer 6, none that an effect removed), read by last-known information: the damage of a source that has left, conditions on a keyword
- [x] (e2) Keyword abilities built from triggers: ward, prowess and battle cry come from `adds_abilities`, listed by the reader under their own addresses; Hero of Bladehold, Monastery Swiftspear and Hulking Raptor are Verified
- [x] (i3) State triggers (CR 603.8) and abilities that work from a zone other than the battlefield (`from`); Reassembling Skeleton, Soul of Zendikar, Elemental Masterpiece, Squee, Boneyard Scourge, Anger and Covetous Dragon are Verified

### Counters, planeswalkers and attachments
- [x] (l1) Counters on permanents and players: the fold, put, remove and move, `Amount::Counters`, +1/+1 and -1/-1 counters in layer 7c with their state-based actions, views; Tireless Tracker, Cathars' Crusade, Inspiring Call and Caress of Phyrexia are Verified
- [x] (l2) Planeswalkers: loyalty abilities (once a turn, at sorcery speed), damage removes loyalty (CR 120.3c), zero loyalty dies, an attacker keeps attacking a planeswalker that left (CR 506.4c); Jace, the Mind Sculptor, Jace Beleren, Ajani, Caller of the Pride and Garruk Wildspeaker are Verified
- [x] (l3) The legend rule (CR 704.5j): a player who controls two legendary permanents of one name keeps one; Arvad the Cursed, Kongming and Sram are Verified
- [x] (m1) `Attach`, Aura spells that target through `Enchant`, and the state-based actions of attachments (CR 704.5m to 704.5p); Pacifism, Epic Proportions, Control Magic, Squirrel Nest and Shielded by Faith are Verified
- [x] (m2) Equipment with `Equip`, `Reconfigure` and living weapon; Bonesplitter, Swiftfoot Boots, Batterskull, Lizard Blades, Bronzeplate Boar and Armguard Familiar are Verified

### Replacement and prevention
- [x] (j1) The replacement step: `Ability::Replacement` found through the reader, one `ReplaceWhen` for each event it can replace, self-replacement first, once per event (CR 614 to 616), and `ChooseReplacement` in engine, protocol, bots and the server's pace; Anointed Procession, Parallel Lives, Doubling Season, Hardened Scales, Corpsejack Menace, Rhox Faithmender, Thought Reflection, Furnace of Rath and Rest in Peace are Verified
- [x] (j2) Enters tapped or with counters; a choice made as it enters and remembered on the permanent; Idyllic Beachfront, Sodden Verdure, Steam Vents, Faithful Watchdog, Hall of Triumph and Rally the Ranks are Verified
- [x] (j3) Damage prevention shields, regeneration and `GameRule::DamageCantBePrevented`
- [x] (j4) Protection from a quality (colour, card type, player, everything) with its four checks (damage prevented, no Aura or Equipment attaches, no blocking by it, no targeting) and shroud; Black Knight, Kor Firewalker and Mother of Runes are Verified
- [x] (j5) A prevention shield that stops only the next N damage covers the events of one damage batch in the order the affected player picks, not in batch order

### Costs and casting
- [x] (g1) One announcement procedure (CR 601.2): modes, X, optional, alternative and additional costs, targets and division, then the total cost; `ChooseNumber`, `ChooseCosts` and `DivideAmount` are built with it
- [x] (g2) `{X}` in spell and ability costs and the `-X` of a loyalty ability; the support walker names `Cost.Mana.X` and `Cost.Loyalty.X`; Blaze, Stroke of Genius, Kessig Wolf Run and Tezzeret the Seeker are Verified
- [x] (g2b) X read after the spell or ability has resolved: a permanent enters with X counters, its enters trigger and a delayed trigger read the X of the spell, a target filter reads the X announced and the mana value of a spell on the stack counts X; Walking Ballista, Hangarback Walker, Gadwick, Disembowel and Forced March are Verified
- [x] (g2c) X in the places (g2b) left: the intervening "if" of an enters trigger and the cast triggers of a spell read X, a spell with X in a target filter is offered only with the X values that have a target, and a library search reads X in a mana value; Wargate is Verified
- [ ] (g2d) A modal spell or ability with X in the target filter of a mode is offered at priority only when some X has a target, as a spell without modes is
- [x] (g3) Optional costs: kicker and multikicker are asked with `ChooseCosts` and `ChooseNumber`, added to the total cost, remembered by the spell, the permanent and its triggers, and read as `Condition::Kicked` and `Amount::Kicks`; Burst Lightning, Gatekeeper of Malakir and Wolfbriar Elemental are Verified
- [x] (g3b) Buyback (the spell returns to its owner's hand as it resolves, not when countered or fizzled), entwine (asked before the modes, chooses all), "as an additional cost, you may ..." (`Ability::AdditionalCost`, `Condition::AdditionalCostPaid`), kicker, buyback, entwine, flashback, evoke and madness costs with life and picked objects, and the kicks of a permanent that left before its trigger went on the stack; Capsize, Dream's Grip, Voltage Surge, Vicious Offering, Deep Analysis and Endurance are Verified
- [x] (g4) Alternative costs chosen with the cast (`ChooseOption`, `Choices.way`) and cast permissions (`GameRule::MayCast`: from a graveyard, exile or a library, without paying, as though with flash): flashback, evoke and overload; Mulldrifter, Faithless Looting, Cyclonic Rift, Mizzium Mortars, Omniscience and Vedalken Orrery are Verified
- [x] (g4b) A spell cast during a resolution (`Effect::CastFromZone`), cascade and discover, and the hidden-information tests of a cast from a library or exile; Bloodbraid Elf is Verified
- [x] (g4c) Madness (the discard is replaced by an exile and the card is cast for its madness cost as a trigger resolves) and suspend (the special action `PriorityAction::Suspend`, time counters on a card in exile, the upkeep removal, the free cast with haste); the keyword Foretell is in the DSL with its `Caps` name (r5c); Fiery Temper, Basking Rootwalla, Arrogant Wurm, Rift Bolt, Ancestral Vision and Durkwood Baloth are Verified
- [x] (g4d) Foretell: the special action `PriorityAction::Foretell` from the hand, the face-down exile that only its owner may look at (`FaceDown`, `GameState::may_look_at`, the face-down rule of the views and the log) and the cast for the foretell cost on a later turn (`CastWay::Foretold`); Behold the Multiverse, Saw It Coming, Demon Bolt and Augury Raven are Verified
- [x] (g5) Cost modifiers (`Ability::CostModifier`): increases and reductions of the total cost in the order of CR 601.2f, from the spell itself and the permanents of every player; Fireball, Thalia, Guardian of Thraben and Foundry Inspector are Verified
- [x] (g5b) Convoke and delve: payment taps creatures or exiles graveyard cards for part of the cost, offered beside the mana sources in `PayMana`; Stoke the Flames, Siege Wurm, Treasure Cruise, Murderous Cut and Gurmag Angler are Verified
- [ ] (g5c) Convoke and delve are shown to every seat by the protocol projection (a test over `mtg-protocol` hidden-information helpers) and a payment of a copy of a spell uses them (waits for the spell copies of (o))
- [x] (g6) Costs of activated abilities beyond `{T}`, mana and sacrificing the source: life, counters removed, and permanents and cards picked when the ability is announced (sacrificed, returned to hand, tapped, discarded, exiled from a graveyard); cycling from a hand; a card's support counts the abilities of the predefined tokens it makes; Flooded Strand, Nantuko Husk, Rummaging Goblin, Soul Shepherd, Llanowar Behemoth, Tumble Magnet, Souldrinker and Lurching Rotbeast are Verified
- [x] (g6b) Costs with a counted amount (X life, X counters removed, X creatures sacrificed, X cards discarded or exiled from a graveyard, X permanents tapped, X energy) are announced with `ChooseNumber` and then their picks, energy is paid as a cost and gained as an effect, a `Pay` effect pays life, energy, counters and picked objects beside mana (ward with a cost other than mana runs), and the walker names `Cost.<part>.Counted` only for an amount that is neither fixed nor X; Copper-Leaf Angel and Solstice Zealot are Verified
- [x] (g6c) The compiler reads and writes "you may pay 2 life. If you do, ...", "you may pay {1}. If you do, ...", "unless that player pays 3 life" and "unless you discard a card" as a `Pay` with one cost part
- [ ] (g6d) Costs of multikicker, suspend, overload and foretell with parts other than mana, a Verified card for a `Pay` with a part (Arrogant Poet) and for a ward with a cost other than mana (Owlin Shieldmage, Sunset Saboteur), the compiler reading "Ward—Sacrifice a creature", "sacrifice ~ unless you pay {G}" (a self-sacrifice effect), a hidden-information test of the picks of a `Pay` at two, three and four seats, a card that removes X counters (needs a negated amount `Amount::Neg` for "-X/-X", Skullmane Baku, or X targets)
- [x] (g7) Mana abilities of several mana, of a count and of a choice of type, with further effects (painlands, Eggs) and sacrificing their source (Treasure, Lotus Petal) are activated without the stack and are mana sources of every payment; `is_mana_ability` holds them whole and the compiler reads the painland wording again; Sol Ring, Gaea's Cradle, Battlefield Forge and Lotus Petal are Verified
- [x] (g7b) Restrictions on how mana is spent ("spend this mana only to cast creature spells"): the pool keeps what each mana may be spent on, payment honours it, and restricted mana beside hybrid costs is paid and refused as the rules say; Ancient Ziggurat, Mishra's Workshop, Pillar of the Paruns and Rosheen Meanderer are Verified
- [x] (g8) A `Pay` the player cannot afford is still asked, with No as the only answer, so the pace does not show whether they could pay (`docs/arena-flow.md`)
- [x] (g9) Divided amounts at resolution: `Amount.Divided` and `Amount.EvenShare` read `Choices.division` (`DealDamage` and `PutCounters` give each target its share) and the support walker stops naming them; Fire // Ice (n1) needs it

### Trigger families
- [x] (i4a) Patterns for the events the engine logs (damage, life gained and lost, draw, discard, cycle with its `Cycled` event, tap, untap, tap for mana, counters put and removed, block, attach) with their templates and detection; Ajani's Pridemate, Thieving Magpie, Underworld Dreams, Megrim, Manabarbs, Mesmeric Orb, Wanderbrine Preacher and Renewed Faith are Verified
- [x] (i4b) Patterns for the creature an Equipment or Aura is attached to (`Is(Attached)`), batches (`Batch`), the nth card, spell or life gain of a turn (`Nth` with per-turn tallies in the state) and the amount of an event (`Amount::Triggering`), with their templates, detection and tests
- [x] (i4c) Skullclamp, Sword of Body and Mind, The Thing, Ben Grimm (the batch card), Lat-Nam Adept, Sanguine Bond, Fear of Failed Tests and Bloodghast are Verified with fuzz games at 2 to 4 seats and a replay test; `Amount::Players`, `LowestLife` and `HighestLife` run (an opponent, each opponent, a player) with their templates
- [x] (i4d) Evasion that runs: `Mod.CantBeBlocked` and `Mod.CantBeBlockedBy` as a static ability and as an effect, checked when blockers are declared; Questing Beast gets its combat damage that can't be prevented and its trigger on a planeswalker the damaged player controls, and is Verified; Invisible Stalker is Verified, Aven Mindcensor is the corpus card that stays Partial, and the compiler reads "can't be blocked this turn"
- [x] (i4h) The compiler reads and writes the static "combat damage that would be dealt by creatures you control can't be prevented" (`Ability::Rule` of `DamageCantBePrevented`), and a Questing Beast test where its planeswalker target leaves before the trigger resolves
- [x] (i4e) Impulse draw: "exile the top card of your library, you may play that card this turn" (lands included, until end of turn or the end of your next turn) for the cards an effect exiled; Professional Face-Breaker, Abbot of Keral Keep and Reckless Impulse are Verified
- [x] (i4f) "For the first time each turn" for an object (an attack, becoming a target, being dealt damage) with a per-turn tally by object; Seedglaive Mentor is Verified
- [x] (i4g) The compiler reads a static ability with a condition ("has haste as long as an opponent has 10 or less life"), "equipped creature gets +2/+2 and has protection from white and from black", the trigger "you create a token and that player mills ten cards" and a plural subtype outside the known list ("Heroes you control"); The Thing, Ben Grimm, Bloodghast and Sword of Body and Mind are then reproduced from their text

### Faces and copies
- [x] (n1a) Multi-face cards: the face model (`CardDefinition.layout` and `faces`, the face an object is in use as), split, adventure and modal double-faced cards chosen at the announcement and listed in `PriorityOptions.faces`, the Adventure exiled on an adventure and cast as the creature, `Card.Faces` lifted for those layouts; Bonecrusher Giant, Fire // Ice, Hengegate Pathway // Mistgate Pathway and Selfless Glyphweaver // Deadly Vanity are Verified
- [x] (n1b) Transform, fuse and daybound/nightbound: `Effect::Transform` turns a transforming double-faced permanent over as the same object, the day and night is a hashed game state shown in `TurnView`, and a split card with fuse is cast as both halves; Delver of Secrets, Ludevic's Test Subject, Tavern Ruffian, Bird Admirer, Tireless Hauler and Wear // Tear are Verified
- [ ] (n1d) Meld (Bruna, Gisela, Brisela: the pair as two cards leaving, the combined back face; the importer's `melds_into` reaches `PrintedCard`), Turn // Burn (a modification that sets creature types for "becomes a Weird"), compiler templates and goldens for "transform ~" and the day and night wording, and the copy of a transformed permanent (o)
- [ ] (n1c) Faces, what (n1a) left: compiler goldens for faces, the walker names of transform and meld, a test that a modal back face leaves the battlefield as its front face, the bot plays `PlayLandFace`, the enter replacement of a back-face land ("enters tapped") is read from the face, and the Adventure exile is seen by graveyard replacements such as Rest in Peace
- [ ] (n2) Face-down objects (morph, disguise, manifest, cloak): a `FaceDown` kind each with its characteristics, who may look at it and turning it face up, on the face-down rule of (g4d)
- [ ] (o) Copies: a token that copies an object, spell copies, enters as a copy, populate, storm; copiable values as the first layer

### Manual cards and views
- [x] (p) Manual cards: a card the engine does not run is cast and resolves without effects, and its controller applies generic logged actions (move, counters, life, token, tap) through the same primitives, visible to every seat
- [ ] The game's `manual_play` flag is stored with the game and listed by `GET /api/lobbies` and `GET /api/games/{id}` (`mtg_protocol::manual_play` derives it from the roster today)
- [ ] Views complete: they carry every state of an object (counters, attachments, face, face-down, copies, support) and the abilities a player may activate with their cost and target wording (Equip, Reconfigure, loyalty and other activated abilities in one list), a spectator audience sees public information, and a card revealed and then moved to a hidden zone in one batch reaches the other seats' views (`docs/protocol.md`)
- [x] The combat forecast and block preview apply replacement and prevention effects, so Furnace of Rath, a life doubler and a damage shield show in the numbers they preview
- [x] Corpus tests derive their counts (cards, sources, compiled, reproduced, fingerprints) from the card list and the fixture, so adding a card edits no count table and agents' branches stop conflicting there
- [ ] Gate: every corpus card is Verified except the two kept Partial and Manual by design, and fuzz games with a deck from each cluster pass every invariant

## M4 Commander
- [x] (hr1) House rules, foundation: a per-game `HouseRules` the table's creator sets, layered over `FormatRules` (starting life, opening hand size, free mulligans so a mulligan costs no card, a cap on mulligans, the first player's draw, maximum hand size); stored with the game recipe so a resume and a replay play the same game, validated at `POST /api/games`, hashed into the game state, shown in the lobby list and the views; table tests per rule, a replay test and fuzz games with random rules
- [x] (hr2) House rules, turn and life: cards drawn each turn, land drops each turn, mana burn (unspent mana costs life), an empty library shuffles the graveyard back in instead of losing, a turn limit after which the highest life wins, a life total that wins the game, the poison threshold, handicaps (starting life and hand size per seat)
- [x] (hr3) House rules, table: attack range (any opponent, the neighbours, the left neighbour only), open hands, a revealed library top for every seat, opening-hand smoothing (two hands drawn, the one closer to the usual land count kept), mulligan style (London, Vancouver with a scry, Paris with one card fewer)
- [ ] (hr4) House rules, Commander (after the format): the tax per cast (none, 1, 2), the commander damage threshold or none, deck-check relaxations (singleton, colour identity, deck size, copies)
- [ ] `Commander` format rules: 100-card singleton, colour identity, 40 life, command zone, tax as a cost modifier, return to the command zone (CR 903.9), 21 commander damage; the views carry commander damage and tax so the pod can show them
- [x] Fix: when the active player leaves mid-turn the turn continues without an active player (CR 800.4; `docs/engine.md`), with a scenario test
- [ ] The table view says when a turn has no active player (`TurnView.active` names the seat that left; the turn indicator and the step bar show a turn without one), with a projection test
- [ ] Partner, companion, Commander-specific tests and Commander-format fuzz games with 4-6 players

## M5 Table and prompts

The server is settled first, then the web is built once against the final types.
- [x] Response clocks for every decision kind without one, each with a fallback answer, and a clock test through the actor for YesNo, ChooseOption and ChooseMode (`docs/server.md`, Response clocks)
- [x] (d9) Timer bank: extensions earned and spent at expiry, a delay before the countdown shows, an optional clock for the active seat
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

## M6 Coverage
- [ ] Choose the set of Commander precons that is the coverage target; `mtg-cli coverage` ranks over all stored precons until then (`docs/deck-check.md`)
- [ ] Coverage dashboard in the web over `Library::support`: states, reasons and clause spans per card, ranked by saved-deck demand; target: 100% of the chosen precons (`docs/deck-check.md`)
- [ ] (h) Keywords, keyword actions and wordings the DSL and the compiler lack, sagas, classes, designations, extra turns and the other long-tail mechanics: one task each (primitive, template, golden), in the order of the report's demand

## M7 Product
- [ ] (B3) Deck check, remaining in the web: each precon's state in the precon browser (the server lists it, `GET /api/precons`; `docs/server.md`, Precons)
- [ ] Precon browser with one-click play, remaining in the web: the page over `GET /api/precons` and a create with `deck: "precon:<code>"` (the server half is done; Commander precons are not `playable` before the Commander format, M4)
- [ ] House rules in the web: the create-game form sets them with the format's values as defaults, and the lobby row and the table name the ones that differ from the format; the Mulligan prompt names the style's hand size and scry
- [ ] Choose a deck in the lobby, remaining in the web: precons as a third kind of deck in the picker (the server takes `precon:<code>` as a seat's deck; `docs/server.md`, `docs/deck-check.md`)
- [ ] The importer fixtures hold real records: `scryfall_oracle_cards.json`, `scryfall_rulings.json` and `mtgjson_deck_commander.json` in `mtg-import` carry invented oracle ids and printings (the card fixtures of `mtg-cards` are checked by `fixture_real`); the tests of five crates that assert on their sets and counts follow, and the synthetic edge cases (a nameless record, art series, emblems) stay marked as synthetic
- [ ] Verify Moxfield, Archidekt, MTGO and Arena export parsers against real exports: the 29-deck corpus passes; open: a real MTGO `.dek`, an Arena client export, companions, custom cards (`docs/data-sources.md`)
- [x] Importer 2b, name resolution on the corpus: by printing (set + collector number) before name, plus flavor, digital and multi-face names (`docs/data-sources.md`); a real-data test resolves all 183 exports
- [x] Deck import by printing in the server: fetch and ingest Scryfall Default Cards (every printing, with flavor and printed names stored on `printings`) and resolve imports with `CardRepo::card_index`, which already holds the stored printings (`docs/data-sources.md`, `docs/server.md`)
- [x] The stored card index equals the index built from records: meld names (a `cards.melds_into` column) resolve, and a line matched by a flavor or digital name selects that printing (`docs/data-sources.md`)
- [ ] Spectators in the web and a "waiting on" message for tables of three or more
- [x] Account settings: change the password, delete the account (its seats are freed)
- [x] A game that failed to resume shows in My games with its reason
- [x] Layout gate for the pages that need an account (Decks, My games, the builder with a real deck), in `accounts.spec.ts`

## M8 Polish and deploy
- [ ] Animations beyond the table feedback (zone moves to hand, exile and library, a spell on the stack; `docs/web.md`), sounds, replays, solo play against bots
- [ ] Dockerfile, Kustomize manifests for k3s, Postgres, ingress + TLS on the user's domain (`docs/PLAN.md`, Deployment)
- [ ] Performance profile on real hardware with real card art
