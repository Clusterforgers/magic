# Roadmap

Build order of the platform: what is done, what is open and what comes next. Continue with the first open piece of "Next up".

One item is one piece of about 15 minutes and one line: the outcome, the cards it frees and a few examples. The commit that finishes it deletes its line, turns a leftover into a new lettered item and names a new area under "Done"; designs are in `docs/`.

## Status

- M0 to M2 (foundations, card data, engine core, protocol and server, game flow, web client, deck builder): done.
- M3 card system and M6 coverage: in progress; 4,141 of the 6,429 stored precon cards (64.4%) and 80.7% of the copies are Verified.
- M4 Commander: done in the engine, server and lobby except the companion conditions that need more than the card (m4c).
- M5 table and prompts, M7 product, M8 polish and deploy: not started.

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

- Costs and casting: (g2e), (g5c), (g6d), (r2d), (r1va), (r1vb), (c1j), (c1k), (c1l), (c12f), (c13b), (c14a), (r2dgc-a).
- Cast ways and zones: (c5e), (c2l3a) to (c2l3c), (r2de-c), (n2b) face-down objects, (n1d3) meld, (r1ap), (r2q).
- Triggers and keywords: (c6d), (c4e), (c3b), (c3d), (c2g), (c2d), (c13h), (r2ja), (r2jb), (r2ad), (n1d6) day and night wordings, (o2c).
- Statics and layers: (c8ja), (c8jb), (c7l) to (c7n), (c7q), (c7s), (c7e), (o1d), (m4), (r1ab), (r2az-a), (r2az-c).
- Triggers and keywords: (c6d-c), (c6d-d), (c6d-e), (c6d-f), (c4e), (c3b), (c3d), (c2g), (c2d), (c13h), (r2ja), (r2jb), (r2ad), (n1d6) day and night wordings, (o2c).
- Statics and layers: (c8ja), (c8jb), (c7l) to (c7n), (c7q), (c7s), (c7e), (o1d), (m4), (r1ab), (r2az-a), (r2az-b).
- Statics and layers: (c8ja), (c8jb), (c7l) to (c7n), (c7q), (c7s), (c7e), (o1d), (m4), (r1ab), (r2az-b), (r2az-c), (r2az-e), (r2az-f).
- Compiler wordings, no engine change: (c10) one piece per wording family with (c10s-a) to (c10s-e), (c10d), (c10e) and (c10r), (o2fa) to (o2fd), (c11) and (c11c) layouts, (c9c), (r2dgb), (r2dgc-b), (r2dgc-c), (r1auec), (r1aued).
- Model-assisted reading: (r3) the next sample, (r2dge), (r2ec) a specialist model.
- Commander and server: (m4c) the companion conditions and a corpus card.
- Tooling: (dep) dependencies, alone between rounds; (t1) a flaky test; (t5a) the Postgres lock scope; (t5b) the testkit time.
- After the card system: "Views complete", the Gate, then the web (M5 prompts for the decision kinds the engine asks, the manual-action menu, table states) and M7.
- After each round: `mtg-cli ingest` and `compile` refresh the real database, `coverage` is re-run and the numbers go on the M6 target line; `fetch scryfall --printings` fills the printings once.

## Done

- Foundations: workspace, lint rules, CI, `CLAUDE.md`, local Postgres and the React stack (`docs/spike-react.md`).
- Card data: Scryfall and MTGJSON import, deck-list parsers, the schema with card search and precons, and `mtg-cli` fetch, ingest, compile and coverage.
- Engine core: zones, turns, priority, stack, combat with forecast and block preview, bots with a repeat guard, `mtg-testkit`, and crowded tables of thousands of permanents.
- Protocol and server: generated wire bindings with a drift test, the game actor, a WebSocket per seat, persistence and resume, accounts, lobbies, open games and precons.
- Game flow: auto-pass, response clocks, yields, stops, Full Control, idle windows, the timer bank and leak tests (`docs/arena-flow.md`).
- Web: the game UI with prompts for all eight decision kinds, the N-seat table with log, stack and animations, lobby, account pages, the UI rebuild and clock-free e2e tests.
- Decks: list, import, export, builder and the deck check with the Verified marker; the importers pass the 29-deck corpus.
- Compiler: reader framework, renderer with goldens, corpus ratchet, templates for every ability shape, counted amounts with domain and stored `card_rules` (`docs/cards-dsl.md`).
- Changes of control: targets, groups, the enchanted permanent, one target for each opponent with the untap and keywords after it, and exchanges of control (`docs/engine.md`, `docs/cards-dsl.md`).
- Abilities and characteristics: one ability reader, CR 613 in one pass, `GameRule` effects, state triggers and abilities that work from hands, graveyards, exile and the stack.
- Counters and planeswalkers: counters on permanents and players, keyword counters, counters of any kind, counter costs on other permanents, counters of any kind removed by a cost or an effect, "if it had counters on it", multiplied counters, rad counters with their mill as the precombat main phase begins, loyalty abilities and the legend rule.
- Attachments: Auras on permanents and players, Equipment, Reconfigure, living weapon, umbra armor and Aura grants.
- Replacement and prevention: enters tapped or with counters, damage shields, regeneration, protection, death shields and conditional "instead" replacements.
- Costs and casting: CR 601.2 announcement with modes, X, alternative and additional costs, kickers, cost modifiers and conditional reductions, reductions by counts of the turn and for the first spell of a kind, activated abilities that change their own cost, free casts and Strive, casts of a target card with the exile of that spell, and permissions that work once during each of your turns.."
"- Mana: restricted mana and spend filters, filter lands, triggered and counted mana, storage lands, mana costs with mills and counters, and mana kept past the step.
- Emblems: objects in the command zone with abilities of their own that work from there, read and written by the compiler.
- Incubate: the Incubator token with its Phyrexian back face, read and written by the compiler.
- Expend: the trigger on the turn's Nth mana spent on spells, read and written by the compiler.
- Clash: the clash with an opponent, its win branch and "repeat this process", read and written by the compiler.
- Crimes: "whenever you commit a crime" and "whenever an opponent commits a crime" on targeting another player, what they control or their graveyard.
- Clash wordings: a won clash that may put the returned creature on top instead, and the clashed player's creatures held through their next untap step.
- Triggers: every logged event, batches, the nth card or spell, "first time each turn", turn tallies, cast triggers, the Monarch and the city's blessing.
- Keywords: combat, evasion and characteristic keywords, keyword actions, cast ways, upkeep and death keywords, Cipher, Hideaway, Emerge, Harmonize and Read ahead (`docs/engine.md`).
- Keywords given to spells: convoke, improvise, delve and affinity from statics; offered helpers never take what the additional cost takes.
- Vehicles and Mounts: crew, saddle and station, crewed and saddled events, the crew record of the turn and Vehicles animated by abilities.
- Phasing and day and night: phased-out permanents in their own zone, life totals that can't change, player protection, phase-outs held until a source leaves, phase-out and phase-in triggers, day and night effects and conditions.
- Statics: lords, characteristic-defining abilities, conditions, attack taxes in mana and life, "can't attack you or planeswalkers you control", blocker limits, limits on the whole attack and block declaration and on the attacks at one defender, player keywords, stay-tapped statics, grants outside the battlefield, group prevention, groups of spells that can't be countered and conditions on the attached permanent.
- Mutate: merged permanents under copy effects, mutation counts, merged cards leaving to libraries and the command zone (`docs/engine.md`, Mutate).
- Faces and copies: multi-face cards, transform, fuse, daybound, Sagas, Classes, flip cards, copiable values, token, spell and ability copies, copy groups and prepare cards.
- Face-down objects: morph, megamorph, disguise, manifest and cloak with their compiler wordings and views.
- Commander: house rules, the format rules, the deck check, Partner, the companion in the engine and at the table, and both modes for a commander's controller.
- Zone moves: moves by a choice, linked exiles, exact and X targets, random picks from a graveyard, reveal until a match and mass moves.
- Tokens: kinds, named and legendary tokens, sized tokens, quoted abilities on tokens and results bound to the tokens made.
- Chance and choices: dice tables, coin flips, votes, piles, repeat for each player or object, planechase votes and modes at random, once each turn, once ever, repeated or one or more.
- Fights and targets: fights of two targets, ability counters, second targets kept apart, targets for each opponent and the chosen player.
- Turn structure: extra turns, added phases, phase conditions, skipped extra turns and time travel.
- Model-assisted reading: `mtg-cli check-rules`, `propose` (Anthropic or a local OpenAI-style server), `promote`, `dsl-reference` and `dsl-schema` (schema-constrained output of a local server), the census of stored proposals, writers for every accepted sample wording and the shapes of the local-model proposals, and rewordings that join, order and shorten sentences as printed.
- Reference engines: Forge, XMage and Magarena surveyed, and `mtg-cli forge-check` maps Forge's scripts to DSL verbs (`docs/reference-engines.md`).
- Manual cards: the `manual_play` flag of a table, stored with the game and listed by the game browser.
- Tooling: `scripts/check.sh` with the `affected` mode and passed-tree stamps, `scripts/agent-status.sh`, fixture checks against the cached Oracle Cards and web clock budgets stretched by the load.
- Replacements that name their event: `Instead` steps read the replaced player, amount and source of damage, and "tap or untap" is a choice of the controller.
- Preventions that name their damage (counters for it, all but 1) and their shields, and token replacements for one kind of token.
- X in triggers: an `{X}` paid in an effect and read by what follows it, the X of a cycling cost or a monstrosity in its trigger, and "up to X" targets.
- Exert: the choice in the attack declaration, `Exerted` and `ExertEnded`, the untap step of the exerting player it skips and its "When you do" and "Whenever you exert a creature" triggers.
- Mill and rad counters: mill triggers per card, per batch and per player, target bounds from a mill batch, cards milled this turn, the radiation rule, entering tapped for rad counters, costs reduced per rad counter and losing all rad counters.

## M3 Card system

Design and task order: `docs/cards-dsl.md` (Tasks). The clusters run in the order listed.

### Costs and casting
- [ ] (g2e) The mode prompt offers a mode with X in its target filter only when some payable X has its targets, so choosing a mode never backs the cast out
- [ ] (g5c) Convoke and delve picks are public to every seat in the protocol projection, with a hidden-information test
- [ ] (g6d) Cost parts other than mana for multikicker, suspend, overload, foretell, ward and `Pay`; frees about 5 cards (Arrogant Poet, Owlin Shieldmage, Skullmane Baku)

### Faces and copies
- [ ] (n1d3) Meld (CR 712.4): two cards exile and return as one permanent with the combined back face and leave as two objects; frees 3 cards (Bruna, Gisela, Brisela)
- [ ] (n1d6) The day and night wordings left; frees about 10 corpus cards (Celestus Sanctifier, Angel of Eternal Dawn, Tovolar, Dire Overlord)
- [ ] (n2b) Face-down remaining: manifest dread and of chosen cards, face-up and face-down predicates, turn-up conditions, morph costs beyond mana; frees about 14 cards (Write into Being, Whisperwood Elemental, Fathom Seer)
- [ ] (o2c) A storm trigger or copy effect that resolves after its spell has left the stack copies it from last known information
- [ ] (o2fa) "When you next cast" left: inside "If you do," a chapter with an ability word or a mana ability, "an additional time", a spell or ability, and effects other than a copy (counters, sunburst, rebound); frees about 12 cards (Najal, Mercurial Spelldancer, Ether, Magus Lucea Kane)
- [ ] (o2fb) Copy exceptions that set a name or depend on the copy (Sakashima, Lazav, Spark Double's counter by type); frees about 9 cards
- [ ] (o2fc) Token copies that attack a player ("for each opponent, create a token copy that attacks that opponent") and storm riders (Skull Storm's "each opponent who can't loses half their life", Storm of Forms' count of counter kinds); frees about 10 cards (Skull Storm, The Scarab God)
- [ ] (o2fd) Populate X times and populate for each (Full Flowering)
- [ ] (o1d) A transformed permanent that dies reads its trigger from the face it showed, and a copy-granted static reads it from the copy that left in the same batch

### Attachments
- [ ] (m4) A Fortification that is also a creature (CR 301.6): the attachment check and a test

### Manual cards and views
- [ ] Views complete: every object state, the activatable abilities with cost and target wording, a spectator audience, and reveals moved to a hidden zone in one batch (`docs/protocol.md`)
- [ ] Gate: every corpus card is Verified except the two kept Partial and Manual by design, and fuzz games with a deck from each cluster pass every invariant

## M4 Commander
- [ ] (m4c) Companion conditions that need more than the card (mana value parity, an activated ability, a shared type, one mana symbol) in rules, engine and deck check; frees about 4 cards (Jegantha, Kaheera, Gyruda)

## M5 Table and prompts

The server is settled first, then the web is built once against the final types.

- [ ] The companion in the web: a seat-panel slot from `PlayerView.companion`, `BringCompanion` in the priority menu and the deck builder's companion issue
- [ ] (i2) Arena flow P10: smart stops, per-ability yields, an order-by-timestamp preference, damage auto-assign and the answer to a one-option decision (`docs/arena-flow.md`)
- [ ] Prompts for ChooseNumber, DivideAmount, ChooseCosts, ChooseReplacement, scry and cards put back; the payment prompt for X, restricted mana, convoke and delve
- [ ] Clock words in the web (`clockText.ts`) for every decision with a clock the web does not name yet
- [ ] The `YesNo` prompt disables Yes and its key when `YesNoOptions.only_no` is set
- [ ] Cast options in the table: face, optional and alternative costs, modes, X
- [ ] Attack and block prompts lock the creatures a rule requires, and a seat panel shows a revealed library top (`PlayerView.revealed_top`)
- [ ] The forecast and block preview mark numbers that depend on a choice (`CombatForecast.depends_on_choice`)
- [ ] Table states: counters, attachments, loyalty, faces, face-down cards, copies, command zone, commander damage and support marks, with `/dev/table` screenshots at six sizes in both themes
- [ ] The turn indicator and step bar show a turn without an active player (`TurnView.active_left`), with `/dev/table` screenshots at six sizes in both themes
- [ ] Manual actions on any card: a menu and log entries
- [ ] (U5b) One step ladder: the ladder that shows the turn also sets the stops (click a step), as in Arena, and the second row of step buttons goes
- [ ] Playwright games with corpus decks that ask every decision kind
- [ ] (c5h) The table's event text has a line for the rule `MustAttackPlayers` (Encore) instead of the generic trigger line
- [ ] (c5i) The attack prompt offers the exert of a creature that may be exerted (`AttackerOption.exert`, `Answer::DeclareExerting`) and the table marks an exerted permanent

## M6 Coverage

### Target
- [ ] Target: every stored card Verified, or as near as the rules allow; after each round `compile` and `coverage` are re-run and the numbers recorded here (4,386 of 6,429 precon cards, 68.2%; 83.0% of the copies)
- [ ] Choose the set of Commander precons that is the coverage target; `mtg-cli coverage` ranks over all stored precons until then (`docs/deck-check.md`)

### Mana and costs
- [ ] (c1j) A mana ability that takes permanents and is used more than once in a payment may take its own source with its last use (Skirk Prospector)
- [ ] (c1k) The triggered mana abilities of a nonland permanent and with a condition (Kinnan, Bonder Prodigy)
- [ ] (c1l) Mana abilities still unread: Domain counts, X mana by a where clause, counted sacrifices, a cost's mana value, mana not for generic costs; frees about 8 cards (Springjack Pasture, Illuminor Szeras, Neheb)
- [ ] (c12f) Effects after the additional costs of a spell: divided X damage, two targets from X and what a cost took; frees about 12 cards (Fire Covenant, Disaster Radius, Momentous Fall)
- [ ] (r2d) Costs in the engine: `PayLife.Counted`, `PayEnergy.Counted`, `Discard.Counted`, `Untap`, `OneOf`, `ReturnToHand`, `Pay.ExileSelf`; frees about 12 cards (G'raha Tia, Confiscation Coup, Order of Whiteclay)
- [ ] (r1va) Reductions of other activated abilities: Equip lines with a reduction, "Equip, Cycling, Boast abilities you activate", "abilities of Foods", "the first activated ability each turn"; frees about 12 cards (Arm-Mounted Anchor, Fluctuator, Sam, Loyal Attendant, Tezzeret, Betrayer of Flesh)
- [ ] (r1vb) Reductions granted for later spells: "the next spell you cast this turn costs less", "spells you cast this turn" after a trigger, counts of opponents attacked and energy paid, the first face-down or X spell; frees about 12 cards (Kaza, Roil Chaser, Spellbinding Soprano, Fast Forward, Kadena)
- [ ] (c14a) The rest of the top coverage blockers: Multiclass Baldric, Yavimaya Bloomsage, a secretly chosen opponent, six spell lines and rules for the 61 other prepare cards; frees about 70 cards (Stalking Leonin, Ponder)
- [ ] (c13b) The rest of the commander free casts and strive spells; frees about 10 cards (Deflecting Swat, Twinflame, Reinterpret)

### Keywords and cast ways
- [ ] (c2g) Keywords given to spells beyond convoke, improvise and delve, and keywords with a cost given to cards in a hand or graveyard; frees about 20 cards (Ghost Ark, Past in Flames, Zhulodok)
- [ ] (c2d) The cards that print scavenge, transmute, awaken, gravestorm or sunburst and wait on another wording; frees about 15 cards (Brainspoil, Ondu Rising, Dodgy Jalopy)
- [ ] (c2l3a) Plot: the special action from a hand, the plotted card cast free as a sorcery on a later turn, plot cost reductions; frees about 4 cards (Lock and Load, Pyretic Charge)
- [ ] (c2l3b) Soulbond: pairing as either creature enters, "as long as ~ is paired" statics and paired triggers; frees about 3 cards (Silverblade Paladin, Breathkeeper Seraph, Donna Noble)
- [ ] (c2l3c) The backup, emerge, harmonize, replicate, mayhem and afflict cards that wait on another wording, and "Discard any number of cards, then draw that many cards"; frees about 15 cards (Guardian Scalelord, Adipose Offspring, The Elder Dragon War)
- [~] (r2da-c-a) (w14b-incubate-devour) Incubate wordings not read: "its controller incubates X", an X of the creatures "exiled this way", a cast trigger of a Phyrexian creature or artifact creature spell (Sunfall, Excise the Imperfect, Brimaz, Blight of Oreskos)
- [ ] (r2da-h) "Do this only once each turn" after a may as its own limit: a declined may leaves the ability open for the turn (now read as a trigger limited to once each turn)
- [~] (r2da-c-j) (w15c-forecast-clash-arena) Exert as a cost part and mana whose spell gains haste (Arena of Glory, Oasis Ritualist, Fervent Paincaster)
- [~] (r2da-c-g) (w15c-forecast-clash-arena) Forecast: an ability activated from the hand during its owner's upkeep by revealing the card (Skyscribing)
- [~] (r2da-c-i) (w15c-forecast-clash-arena) Clash leftovers: "at the beginning of your next main phase, you may add {C} equal to that spell's mana value", "clash with defending player" and "whenever you win a clash" (Scattering Stroke, Marvo, Deep Operative)
- [~] (r2da-c-e1) (w15a-dungeons-initiative) The rooms of Dungeon of the Mad Mage, Tomb of Annihilation and Undercity the compiler does not read (Runestone Caverns, Mad Wizard's Lair, Veils of Fear, Sandfall Cell, Oubliette, Throne of the Dead Three), so those dungeons can be entered
- [~] (r2da-c-e2) (w15a-dungeons-initiative) The initiative (CR 725): the designation, "you take the initiative", venture into Undercity as it is taken and in its holder's upkeep, taken by combat damage, "if you have the initiative" (Seasoned Dungeoneer, Loot Dispute, Sarevok's Tome, From the Catacombs)
- [~] (r2da-c-e3) (w15a-dungeons-initiative) The venture cards that wait on another wording, and the player view of the dungeon and room (Radiant Solar, Thorough Investigation, Immovable Rod, Sefris of the Hidden Ways, Hama Pashar)
- [ ] (r2da-d) Emblem abilities the compiler does not read ("Whenever a creature attacks you, it gets +5/+5", an activated ability with "where X"), then the other clauses of the emblem planeswalkers; frees about 20 cards (Garruk, Apex Predator; Ob Nixilis of the Black Oath)
- [ ] (r2dga) Rules for the partners of accepted proposals, so both halves become corpus cards; frees about 8 cards (Ukkima, Stalking Shadow, Pippin, Warden of Isengard, Brallin, Skyshark Rider)
- [ ] (c9c) Verified corpus cards for Doctor's companion, Partner—text, Choose a Background and a planeswalker commander, once their other text runs
- [ ] (c3d) The monarch cards still held back: "can't attack you", "the monarch controls", stun counters after a group's damage, a delayed attack on the monarch; frees about 6 cards (Fealty to the Realm, Okoye, Regal Sliver)
- [ ] (c3b) Time counters on suspended cards and permanents, in the views of exiled cards too, and the blessing badge on the table
- [ ] (c4e) Cards that read the crew record and crew events and wait on a wording, and a Reconfigure corpus card; frees about 10 cards (Smogbelcher Chariot, Mindlink Mech, The Fantasticar)
- [ ] (c5e) The rest of the cipher and hideaway cards; frees about 6 cards (Hidden Strings, Shelldock Isle)
- [~] (c6d-e) (w15b-out-of-time) Vanishing without a number and The Moment's destroy by time counters; frees Out of Time and The Moment
- [~] (c6d-f) (w15b-out-of-time) Batch triggers on cards put into exile from anywhere and The War Doctor's attack damage that exiles; frees The War Doctor
- [~] (c6d-c) (w14b-incubate-devour) Devour X, devoured kinds and conditions: Thromok, Voracious Dragon, Hellkite Hatchling, Kresh, and a discard for each devoured creature (Tar Fiend); none of them is in a precon
- [ ] (c6d-d) Gift "instead" spells left (Wildfire Howl, Dewdrop Cure, Into the Flood Maw) and entered-under-your-control-this-turn conditions (Zimone, Gargoyle Flock, Galadriel)
- [ ] (r2de-c) Cast permissions left: "Once each turn" on any turn, a spell cast through a permission exiled instead of a graveyard, a card cast from its own graveyard or exile, exiled cards cast in a trigger, spells chosen from a graveyard and cast free; frees about 20 cards (Kess, Dissident Mage, Squee, the Immortal, Dire Fleet Daredevil, Diviner of Mist)
- [ ] (r2r) Alternative costs left: life or energy instead of mana, fixed mana, impending, X in madness, a paid madness cost as a condition; frees about 8 cards (Xander's Pact, Nashi, Fist of Suns)
- [ ] (r1aug) Extra turns and phases: a loss at the extra turn's end step, its restrictions, attacks alone, combats with chosen attackers, extra-turn costs; frees about 20 cards (Final Fortune, Medomai, Moraug)

### Statics, attachments and mutate
- [ ] (c7l) The attacking player who chooses the target and the mode of a trigger the Curse controls (Curse of Inertia)
- [ ] (c7m) Quoted abilities that are rules or make tokens, and the outlaw spend restriction (Nerd Rage, Tarmogoyf Nest, Discreet Retreat)
- [ ] (c7n) "they gain decayed" after a choice of a card in each player's graveyard (Ghouls' Night Out)
- [ ] (c7q) Block requirements: "must be blocked if able", "by exactly one creature", "all creatures able to block it do so"; frees about 8 cards (Madame Vastra, Predatory Impetus, Nacatl War-Pride)
- [ ] (c7s) "If the creature an opponent controls would die this turn, exile it instead" after a fight of two targets (Unnatural Aggression)
- [ ] (c7e) The rest of mutate: exile until X permanent cards, a merged permanent that returns, commander replacements of a merge, a token or face-down top (Auspicious Starrix)
- [ ] (c8ja) Life gain replaced by another effect (`ReplaceBy::Instead` in the engine): a loss instead, cards drawn instead, no life gained; frees about 6 cards (Tainted Remedy, Plague Drone, Nefarious Lich)
- [ ] (c8jb) Prevention for "you and" a group, quoted static abilities given under a condition, and the Slivers' quoted abilities left (flip, becomes a type, top of library); frees about 20 cards (Rune of Flight, The Wanderer, Pulmonic Sliver)
- [ ] (r1ab) The rest of the `Continuous` statics: types for spells and cards, every land type, a color added, group unions, "modified", attachment conditions; frees about 25 cards (Arcane Adaptation, Death Baron, Song of the Dryads)
- [~] (r2az-c) (w14a-attack-domain-leftovers) Domain of another player and the effects around domain the compiler does not read: "lands they control" (the player of the sentence, read and written back), "lands that player controls" in an intervening "if", "discards a card for each", "unless its controller pays {1} for each", "look at the top X ... put up to two into your hand", "prevent the next X damage"; frees no precon card, about 8 corpus cards (Allied Strategies, Mask of Intolerance, Evasive Action, Shadow Prophecy)
- [~] (r2az-e) (w14a-attack-domain-leftovers) A limit on the attacks at you under a condition ("As long as ~ is tapped, no more than one creature can attack you each combat") and "each opponent can't block with more than one creature this combat": frees Mirri, Weatherlight Duelist
- [~] (r2az-f) (w14a-attack-domain-leftovers) The loyalty abilities of The Eternal Wanderer: exile until "that player's next end step" and "for each player, choose a creature that player controls ... sacrifices all creatures they control not chosen this way"
- [ ] (r2bhd) Untaps in other players' untap steps and groups held through their next untap step; frees about 8 cards (Seedborn Muse, Unwinding Clock, Lorthos, the Tidemaker)
- [ ] (r2u) Attached Auras and Equipment as a count, and attachments made by effects (any number, onto tokens, onto returned cards); frees about 15 cards (Kor Spiritdancer, Kemba, Goldwardens' Gambit)
- [ ] (r2db-a) Several statics, or a player rule and statics, in one sentence, and hexproof from the Aura controller's opponents; frees about 6 cards (Tenza, Godo's Maul, Nighthowler, Sigarda, Heron's Grace)

### Compiler wordings
- [ ] (c10) Unread spell, trigger and activated wordings ranked by `mtg-cli coverage`: one piece per family left (quoted tokens, goad, counters, pump, manlands, graveyard, flicker, damage, draw and discard)
- [ ] (c10s-a) Mana spent to cast a spell as an amount ("the amount of mana spent to cast this spell", "that spell"): Wurmquake, Manaform Hellkite, Prossh, Gyrus, Kurbis, Marath
- [ ] (c10s-b) Differences and "whichever is greater" as amounts: Sandstone Oracle, Slithermuse, Hit the Mother Lode, Jaws of Defeat, Tales of the Ancestors, Prophet of the Scarab, Willowdusk
- [ ] (c10s-c) Counts of another player ("the number of Islands target opponent controls", "creatures target player controls") and of what each player controlled that was destroyed (Deadly Tempest)
- [ ] (c10s-d) Token counts and X/X tokens from a count ("a token for each nontoken creature destroyed this way", "an X/X token, where X is ..."): Blood Money, Ceaseless Conflict, Phyrexian Rebirth
- [ ] (c10s-e) Coven and names left: a conditional cast permission (Augur of Autumn), "Then if" after a targeted step (Leinore), differently named tokens and different names (Gimbal, Sandsteppe War Riders, Neriv, Liliana's Contract)
- [ ] (r2de-a4) Counters of any kind left: one removed from the source as a cost (its kind asked while paying) and X of them from a picked permanent or a target; frees about 6 cards (Loch Mare, Burdened Stoneback, Moxite Refinery, Thornmantle Striker)
- [ ] (r2de-a3) Counters from among permanents as a cost or effect, the counters of a permanent that left put on another, and "that many" of its counters; frees about 10 cards (Tayam, Iron Apprentice, Reyhan, Last of the Abzan)
- [ ] (c10d) Top-of-library wordings left: reveals with a follow-up, each player's top cards, a land put from a look, places beneath the top X; frees about 20 cards (Risen Reef, Explorer's Scope, Unexpectedly Absent)
- [ ] (c10e) Subtype and type nouns, legendary names without a comma as `~`, and the events "enters from your graveyard", "is turned face up" and "modified creature"
- [ ] (c10r) The flicker with a delayed return, the exile that returns when the source leaves for an Aura with several hosts, and "until your next turn" durations
- [ ] (c13h) Draw replacements and triggers "except the first one they draw in each of their draw steps", and a token when N or more cards leave your graveyard (Notion Thief, Leela, Xyris)
- [ ] (r2ja) An activated ability as a trigger event: "an opponent activates an ability" of a kind of permanent, "that isn't a mana ability", loyalty abilities and "becomes tapped or a player activates"; frees about 8 cards (Harsh Mentor, Immolation Shaman, Kurkesh)
- [ ] (r2jb) Trigger events left: getting energy, "first time" variants (during each of your turns, investigate, nonland discards), "attacks a player and isn't blocked", "isn't blocked this combat", a token copied and several events in one trigger; frees about 15 cards (Valgavoth, Kalamax, Territorial Gorger)
- [ ] (r2ad) Delayed triggers left: "until end of turn, whenever", with a target of their own, on the next spell, on every block, repeating this turn; frees about 7 cards (Infested Thrinax, Jailbreak, Ride the Avalanche)
- [ ] (r2dgb) Effects "unless that player sacrifices or pays", a sentence for each player, and a token copy after a payment; frees about 12 cards (Demanding Dragon, All Is Dust, Bramble Sovereign)
- [ ] (r2dgc-a) A cost for each mode chosen: Spree costs and escalate in the engine, the DSL and the compiler; frees about 4 cards (Requisition Raid, Great Train Heist, Collective Resistance)
- [ ] (r2dgc-b) Modal headers left: "Each mode must target a different player" and "Choose one. Activate only as a sorcery."; frees about 3 cards (Vindictive Lich, Shadrix Silverquill, Aetheric Amplifier)
- [ ] (r2dgc-c) Charm and commander-pair mode sentences: "each of one or two targets", counters distributed among targets, "each creature deals 1 damage to its controller", "counter target spell that targets you", a power-limited "can't block", phasing and goad by a target player; frees about 15 cards (Prismari Charm, Abzan Charm, Will of the Abzan)
- [ ] (r2dgd) Intervening "if" with a may-pay, a removed counter or a cast from the hand, and "you win the game" under a count; frees about 8 cards (Ingenious Prodigy, Reiver Demon, Helix Pinnacle)
- [ ] (r2aa-a) Control changes left: a gift of the card to a player other than its owner (Crown of Doom), each player taking back what they own (Homeward Path, Alicia Masters), "untap target permanent and gain control of it" (Word of Seizing)
- [ ] (r2cxe-a) The X of a morph cost in "when it is turned face up" and of a madness cost under "if its madness cost was paid"; frees about 2 cards (Bane of the Living, From Under the Floorboards)
- [ ] (r2cxe-b) Half X on cast and a part of a payment in an effect that counts X ("you may pay X life", `Effect.Pay.X.Parts`)
- [ ] (r2ab) A color, creature type or card type chosen as a spell resolves or by each player; frees about 10 cards (Sudden Demise, Wash Out, Kindred Charge)
- [ ] (r2aca) Selectors left: the X of the spell a trigger names (Zaxara), what was exiled this way (Declaration in Stone), "if it was enchanted" from the last known information, and "first spell with {X} each turn" (Nev, Owlin Spiralmancer, Zimone)
- [ ] (r2bgd) Damage left: divided among any number of targets, redirected, replaced by the event's amount, "prevent half" and to battles; frees about 12 cards (Magma Opus, Heroic Sacrifice, Plague Drone)
- [ ] (r1ap) Zone moves left: "instead" branches that name targets or counts, returned permanents with counters, haste or attachments, and cards of a chosen type; frees about 15 cards (Tear Asunder, Unholy Heat, Admiral Brass)
- [ ] (r2q) Picks at random from a hand, library, graveyard or among players and targets, with a per-seat projection for hidden zones; frees about 15 cards (Tariel, Gamble, Territorial Hellkite)
- [ ] (r1r) Pump and animate wordings left: doubled power, a bigger change under a condition, union subjects, a land that stays a land; frees about 12 cards (Unleash Fury, Temur Battle Rage, Obuun)
- [ ] (r1ac) Token wordings left: tokens for other players, counts of mana spent or damage dealt, keywords with a parameter beside a quote; frees about 20 cards (Death by Dragons, Skrelv's Hive, Brass's Bounty)
- [ ] (r1aufb) Targets for each opponent or player left: without "up to one", a chosen creature each, the pass's player named again; frees about 15 cards (Hideous Taskmaster, Windgrace's Judgment, Guff Rewrites History)
- [ ] (r1auk) Reveal or dig until X cards or in another player's library, with free casts and the rest on the bottom; frees about 6 cards (Dazzling Sphinx, Gríma, Old Stickfingers)
- [ ] (r1auec) Die results left: a roll in a cost (Clay Golem), "if the result is" conditions (Arden Angel, Sword of Hours), each player rolls and the highest result (Chaos Dragon), X dice and their total (Neverwinter Hydra, Luck Bobblehead), dice ignored or added (Berserker's Frenzy, Barbarian Class), "roll a 4 or higher" triggers (Mr. House) and table rows after an amount; frees about 15 cards
- [ ] (r1aued) Endeavors left: a search for a number of lands, power at least an amount, a return with that many counters, a free cast up to a mana value, "have ~ deal damage to itself"; frees 5 cards (Wild, Valiant, Grave and Arcane Endeavor, Vrondiss)
- [ ] (r1aui) Coins left: lose triggers, stop flipping, counted flips and heads, "Otherwise" after a flip; frees about 11 cards (Krark, the Thumbless, Fiery Gambit, Ral Zarek)
- [ ] (r1auha) Votes left: more than two choices (Council Guardian), names of two words (Travel Through Caradhras), extra votes as a rule (Tivit, Ballot Broker, Brago's Representative), "whenever players finish voting" (Erestor, Model of Unity, Grudge Keeper), votes for players and objects with counts (Círdan, Mob Verdict, Trap the Trespassers); frees about 15 cards
- [ ] (r1auhb) Villainous choices left: an additional choice (The Valeyard), "Then" before the subject (Great Intelligence's Plan), "that opponent" and "each opponent who" subjects, a target's owner (This Is How It Ends), options the compiler does not read yet (Sycorax Commander, Missy, Hunted by The Family); frees about 10 cards
- [ ] (r1auj) Piles that the controller separates or of permanents and targets (Steam Augury, Boneyard Parley, Make an Example, Whims of the Fates)

### Model-assisted reading
- [~] (r3) (the local bulk run of gpt-oss:20b with --schema, run by the orchestrator) Keep sampling the precon cards that are not yet Verified with `mtg-cli propose` and `mtg-cli check-rules`, adding writers for the wordings the checker accepts (`docs/cards-dsl.md`, Model-assisted reading)
- [ ] (r4d-a) New proposals for the r4c cards whose rules differ from the printed text: Aeromoeba, Amazing Acrobatics, Barl's Cage, Battle Strain, Bearded Axe, Blur of Heroism, Briarberry Cohort, Carnivorous Death-Parrot, Chaoslace, Chronicler of Heroes, Clash of Wills, Consecrate Land, Crabapple Cohort, Daring Mechanic, Discombobulate, Dracogenesis, Dwarven Mauler, Dwarven Strike Force, Early Winter, Eidolon of Obstruction, Genetic Recombinator, Gilt-Leaf Seer, Glademuse, Goblin Artillery, Goblin Brawler, God-Pharaoh's Faithful, Heartwood Storyteller, Horobi, Death's Wail, Illuminated Folio, Indestructible Aura, Inquisitive Glimmer, Intervention of Keranos, Isolation Cell, Jolly Gerbils, Jumbo Cactuar, Lifelace, Mana Short, Misinformation, Mudbrawler Cohort, Muzzle, Opportunist, Out of Bounds, Pillaging Horde, Pit Imp, Plague Spores, Pradesh Gypsies, Psionic Sliver, Putrefaction, Rakshasa's Disdain, Relentless Rohirrim, Rhystic Deluge, Rogue Kavu, Saprazzan Breaker, Selective Memory, Shield of the Ages, Skittish Kavu, Solfatara, Spider-Man, New Champion, Staff of the Sun Magus, Steelclad Serpent, Sudden Spinnerets, Sundering Fork, The Warmonger, Thran Golem, Tragic Lesson, Vectis Agents, Vedalken Ghoul, Venerated Teacher, Walking Desecration, Wall of One Thousand Cuts, Wandering Stream, Waterveil Cavern, Well of Life, Wind Sail, Wondrous Revival, and the accepted-Partial ones that leave a clause out (Goblin Fire Fiend, Indigo Faerie, Moderation, Polterheist, Skeleton Ship, Stormchaser Drake)
- [ ] (r4d-d) Writers left for the valid r4c proposals: "Enchant red or green creature" in the keyword reader (Controlled Instincts), prevention of two groups by creatures (Eerie Interference), damage turned into counters (Phytohydra) and "creatures that are still lands" (Living Lands); "any other target" needs a target that leaves out an earlier `AnyTarget` (Arc Trail, Explosive Welcome); Elite Inquisitor and Oversoul of Dusk need new proposals (their protection is one quality of all three)
- [~] (r4d-c) (w14c-proposal-pieces) DSL and engine pieces the r4c proposals need: "can block an additional creature each combat" (High Ground, Night Market Guard, Two-Headed Giant of Foriys), an alternative cost of a sacrifice (Crash), `Ability.ManaAbility.Other` (Energy Refractor) and a count of transformed permanents (Mutagen Connoisseur)
- [ ] (r4b-a) New proposals for the r4a cards whose rules differ from the printed text: Anguished Recollection, Befoul, Bewildering Blizzard, Blessed Wine, Blorbian Buddy, Blue Ward, Clairvoyance, Cloudcrest Lake, Compost, Death Frenzy, Decompose, Educated Detective, Fire and Brimstone, First Little Pig, Flare, Icebind Pillar, Ivorytusk Fortress, Keldon Warcaller, Kindred Dominance, Krark's Thumb, Mannichi, Megatherium, Multani's Presence, Needle Drop, Quagmire Lamprey, Rishadan Pawnshop, Sacred Knight, Spell Snuff, Tresserhorn Skyknight, Turtle-Duck, Vegetation Abomination, Wall of Glare, Winds of Rath, Words of Wisdom, Wrench Mind, You Are Already Dead, the unchecked Barbarian Outcast, Blow Your House Down, Bow of the Hunter, Chandra's Pyrohelix, Crackle with Power, Elven Rite, Felhide Brawler, Heat Stroke, Hungry Ridgewolf, Ingenious Thief, Kitesail Apprentice, Kor Blademaster, Kozilek's Pathfinder, Meekstone, Nullify, Press the Advantage, Promising Duskmage, Skyrider Trainee, Stream of Acid, Structural Distortion, Stumpsquall Hydra, Synchronized Strike, Turn Aside, Two-Headed Giant of Foriys, Unscrupulous Agent, and Saddle of the Cavalier and Sandstorm Verge (a clause left Manual that the DSL says)
- [ ] (r4b-d) Writers left for the valid r4a proposals: Raid with an entry counter (Rigging Runner), a granted untap in each other player's untap step (Urban Burgeoning), damage to a Dinosaur (Dinosaur Hunter), a chosen creature as the source (Crush Underfoot), "the difference" (Balance of Power), "named ~" (Pack Mastiff), "chooses three permanents, then sacrifices the rest" (Razia's Purification); Woodland Champion, Tombfire, Jaheira and Jodah need new proposals
- [ ] (r4b-c) Engine and DSL pieces the accepted-Partial r4a proposals need: a mana ability whose land doesn't untap during the next untap step (Cinder Marsh, Vec Townships), +N/+N for each Aura attached (Auramancer's Guise), can't attack unless defending player controls a land type (Ethereal Whiskergill), a card revealed at random from the hand (Goblin Secret Agent), each player gains control of what they own (Homeward Path), attacks or blocks each combat (Iron Golem), a life total that becomes N (Magister Sphinx), damage not removed in cleanup (Patient Zero), instants only with a nonempty stack (Priority Avenger), mana only for power-up abilities (Quinjet Technician), "whenever you play a card" and a maximum hand size (Recycle), +1/+0 for each of its colors (Civic Saber), `Effect.Pay.DiscardSelf` (Hag of Mage's Doom), `Cost.OneOf` (Heartwood Shard), `Copy.Duration.WhileSourceOnBattlefield` (Shameless Charlatan), perpetual (Begin Anew) and standing (Standing Army)
- [ ] (r3w8a) New proposals for Myth Unbound ("your commander" is a commander you own) and Regenerations Restored (the extra turn is a reflexive "When you do"), whose data differ from the printed text
- [ ] (r3w8b) Trigger writers left: a spell cast with mana value greater than a count (Mizzix of the Izmagnus), "if it isn't that player's turn" (Tataru Taru), a chapter whose target becomes an artifact (Phyrexian Scriptures)
- [ ] (r3w9a) Spell shapes left: divided damage among any number of targets with more sentences (Magma Opus), "any number of target players each" (Priest of Forgotten Gods), "a creature of their choice" with its toughness (Consuming Vapors), an intervening land count with a land put from the hand (Archaeomancer's Map, Contaminant Grafter), "if another Human entered" and "Then if" (Éowyn, Shieldmaiden), a turn condition on a type and a grant (Gourmand's Talent), "that player's turn" (Tataru Taru), costs per a count (Mizzix, Myth Unbound); Genesis Ultimatum and Kinnan need new proposals (the choice has no card filter)
- [ ] (r3w2a-a) A writer for a group that becomes a creature "and gains flying" (Sarkhan the Masterless)
- [ ] (r3w2b) Writers for granted abilities in quotes whose trigger or effect is unwritten (The Reaver Cleaver, Unquenchable Fury), Clues that become Equipment (Armed with Proof), "creatures you control with toxic" (Skrelv's Hive) and an artifact type added without end (Phyrexian Scriptures)
- [ ] (r3w2c) New proposals for Dungeon Geists and Merchant Raiders (the duration is "for as long as you control" the creature) and Unnatural Growth (each creature doubles its own power and toughness)
- [ ] (r3w3b) Steps under "you may" left: a token copy with exceptions, "reveal that many cards", "the sacrificed permanent", "that player may ... if" and an exile of the card from the graveyard as a cost (Nightmare Shepherd, Glint Raker, Shadow, Ludevic, Kozilek's Return)
- [ ] (r3w1a) Conditional steps left: a card chosen at random (Deadbridge Chant), the top card revealed (Druidic Satchel), "remove all of them" with a named token (Replicating Ring), a bound token with a nested condition (Summoner's Sending), "choose target" before the condition (Meren of Clan Nel Toth) and phasing with a rule until the next turn (Perch Protection)
- [ ] (r3w4b) Delayed triggers and repeated steps left: a loyalty ability's delayed return and "tapped artifact, or tapped creature" (Nahiri, the Harbinger), steps repeated for each object ("each Dragon you control", "each of those creatures", "for each of them") and a delayed trigger on its own line (Forth Eorlingas!, Theoretical Duplication)
- [ ] (r3w6a) The A to L mismatches left: "you may have it fight", "you and target opponent each create", "another target player", a token "that's all colors", one of two counter removals as a cost; the proposals of Commodore Guff, Heroes in a Half Shell, Kazuul, Ever-Watching Threshold and Gyome need a rewrite
- [ ] (r3w7b) Propose again the M to Z Mismatch proposals whose rules differ from the text: Mirkwood Bats, Reconstruct History, Regal Sliver, Soaring Lightbringer, Squee, Tenza, Their Number Is Legion, Thor, Unbreakable Formation, Unquenchable Fury, Wake the Past, Windgrace's Judgment
- [ ] (r3w5a) Writers for the rules of the r3 samples: a draw replaced after the first in a draw step (Notion Thief), a type and a granted ability during your turn (Gourmand's Talent)
- [ ] (r3w5b) "any other target" in a mode (Special Move); re-propose Augury Adept, whose trigger leaves out "to a player"; re-check Éowyn, Shieldmaiden once (r3w1) lands
- [ ] (r2ec) A specialist model: `mtg-cli export-training` writes card text and rules pairs from the hand-written cards and accepted proposals with a held-out split, decoding is limited to the DSL grammar, and a LoRA fine-tune with a short prompt replaces the 22K-token reference
- [ ] (r2dge) The tail of the stored proposals: 32 unchecked, 14 invalid and 26 mismatched, mostly wrong proposals that need a rewrite; `mtg-cli check-rules --verbose` names each
- [ ] (r2cf) A card's name in its token or a name that starts with a creature type reads as `~` in both the checker and the corpus calibration (Kher Keep, Tuktuk the Explorer, Sliver Hivelord)
- [ ] (r1a) `mtg-cli forge-check` compares a Forge clause with the ability that stands for it and reads `Count$` amounts, `Valid*` filters and conditions (`docs/reference-engines.md`)
- [ ] (r1b) The prompt of `mtg-cli propose` takes examples from Forge clause-script pairs fetched at run time; examples kept in the repository come from XMage with its MIT notice
- [ ] (r1c) Rules proposals converted from Forge scripts for the API families `forge-check` shows unread, once the GPLv3 conditions for rules that reach browsers are decided
- [ ] (r1ag) The map names the 84 unnamed items of the `forge-check` report and the verb of each of the 594 marked without one that the DSL has

### Layouts
- [ ] (c11) Rooms, Battles, the initiative and dungeons, and the remaining long-tail mechanics found by the report, one task each
- [ ] (c11c) The other 24 transforming Sagas, the two precon flip cards and the `prepare` layout's back face; frees about 37 cards (Budoka Gardener, Nezumi Graverobber)

### Tooling
- [ ] (dep) Cargo and web dependencies are current; the piece runs alone between rounds (`docs/agents.md`)
- [ ] (t5a) The Postgres lock covers only the `sqlx::test` tests: the other tests of `mtg-server` and `mtg-cli` run outside it (one full check: 398 s under the lock, 1,087 s waiting for it)
- [~] (t5b) (w13c-testkit-seconds) The `mtg-testkit` `it` binary needs fewer test-seconds (9,310 over 15,155 tests in 460 modules, 22% in fuzz modules): shared per-test setup and a smaller default game count for diffs that cannot change engine behaviour
- [ ] (t1) The `mtg-cli` `fetch_scryfall` tests are stable under parallel load
- [ ] (t3c) DSL amounts for what an earlier effect did and for the mana value a cost took, and a partner by creature type (Foul-Tongue Shriek, Nahiri's Sacrifice, The Knight of Land Drops)
- [ ] The expected X symbols in `x_cards.rs` of `mtg-testkit` are derived from the card list, so a new X card edits no test
- [ ] Coverage dashboard in the web over `Library::support`: states, reasons and clause spans per card, ranked by saved-deck demand (`docs/deck-check.md`)

## M7 Product
- [ ] (B3) Each precon's deck-check state in the web's precon browser (`GET /api/precons`; `docs/server.md`, Precons)
- [ ] Precon browser with one-click play in the web: the page over `GET /api/precons` and a create with `deck: "precon:<code>"`
- [ ] House rules in the web: the create-game form sets them, the lobby and table name those that differ, and the Mulligan prompt names hand size and scry
- [ ] Precons as a third kind of deck in the lobby picker (`docs/server.md`, `docs/deck-check.md`)
- [ ] (hr5c) The lobby picker shows each deck's verdict at the table before joining, through `POST /api/decks/check` (`docs/server.md`, Precons)
- [ ] The importer fixtures of `mtg-import` hold real records instead of invented oracle ids and printings; synthetic edge cases stay marked as synthetic
- [ ] Verify the MTGO and Arena export parsers against real exports, with companions and custom cards (`docs/data-sources.md`)
- [ ] Spectators in the web and a "waiting on" message for tables of three or more

## M8 Polish and deploy
- [ ] Animations beyond the table feedback (`docs/web.md`), sounds, replays, solo play against bots
- [ ] Dockerfile, Kustomize manifests for k3s, Postgres, ingress and TLS on the user's domain (`docs/PLAN.md`, Deployment)
- [ ] Performance profile on real hardware with real card art
- [ ] (p1a) Block options test evasion once per attacker and per blocker (0.4 s at 1,500 against 1,500 today), and a death re-derives the layers only when an effect reads it
