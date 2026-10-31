# Cards backlog

Open card, compiler and wording items by code area; the rules of `docs/ROADMAP.md` apply (one line per item, claims, ids). The gap ledger (`mtg-cli check-rules --gaps`, `coverage --all`) ranks them by the popularity of the cards they hold back. Design and task order: `docs/cards-dsl.md` (Tasks).

## Costs and casting

- [ ] (g2e) The mode prompt offers a mode with X in its target filter only when some payable X has its targets, so choosing a mode never backs the cast out
- [ ] (g5c) Convoke and delve picks are public to every seat in the protocol projection, with a hidden-information test
- [ ] (g6d) Cost parts other than mana for multikicker, suspend, overload, foretell, ward and `Pay`; frees about 5 cards (Arrogant Poet, Owlin Shieldmage, Skullmane Baku)

## Faces and copies

- [ ] (n1d3) Meld (CR 712.4): two cards exile and return as one permanent with the combined back face and leave as two objects; frees 3 cards (Bruna, Gisela, Brisela)
- [ ] (n1d6) The day and night wordings left; frees about 10 corpus cards (Celestus Sanctifier, Angel of Eternal Dawn, Tovolar, Dire Overlord)
- [ ] (n2b) Face-down remaining: manifest dread and of chosen cards, face-up and face-down predicates, turn-up conditions, morph costs beyond mana; frees about 14 cards (Write into Being, Whisperwood Elemental, Fathom Seer)
- [ ] (o2c) A storm trigger or copy effect that resolves after its spell has left the stack copies it from last known information
- [ ] (o2fa) "When you next cast" left: inside "If you do," a chapter with an ability word or a mana ability, "an additional time", a spell or ability, and effects other than a copy (counters, sunburst, rebound); frees about 12 cards (Najal, Mercurial Spelldancer, Ether, Magus Lucea Kane)
- [ ] (o2fb) Copy exceptions that set a name or depend on the copy (Sakashima, Lazav, Spark Double's counter by type); frees about 9 cards
- [ ] (o2fc) Token copies that attack a player ("for each opponent, create a token copy that attacks that opponent") and storm riders (Skull Storm's "each opponent who can't loses half their life", Storm of Forms' count of counter kinds); frees about 10 cards (Skull Storm, The Scarab God)
- [ ] (o2fd) Populate X times and populate for each (Full Flowering)
- [ ] (o1d) A transformed permanent that dies reads its trigger from the face it showed, and a copy-granted static reads it from the copy that left in the same batch

## Attachments

- [ ] (m4) A Fortification that is also a creature (CR 301.6): the attachment check and a test

## Mana and costs

- [ ] (c1j) A mana ability that takes permanents and is used more than once in a payment may take its own source with its last use (Skirk Prospector)
- [ ] (c1k) The triggered mana abilities of a nonland permanent and with a condition (Kinnan, Bonder Prodigy)
- [ ] (c1l) Mana abilities still unread: Domain counts, X mana by a where clause, counted sacrifices, a cost's mana value, mana not for generic costs; frees about 8 cards (Springjack Pasture, Illuminor Szeras, Neheb)
- [ ] (c12f) Effects after the additional costs of a spell: divided X damage, two targets from X and what a cost took; frees about 12 cards (Fire Covenant, Disaster Radius, Momentous Fall)
- [ ] (r2d) Costs in the engine: `PayLife.Counted`, `PayEnergy.Counted`, `Discard.Counted`, `Untap`, `OneOf`, `ReturnToHand`, `Pay.ExileSelf`; frees about 12 cards (G'raha Tia, Confiscation Coup, Order of Whiteclay)
- [ ] (r1va) Reductions of other activated abilities: Equip lines with a reduction, "Equip, Cycling, Boast abilities you activate", "abilities of Foods", "the first activated ability each turn"; frees about 12 cards (Arm-Mounted Anchor, Fluctuator, Sam, Loyal Attendant, Tezzeret, Betrayer of Flesh)
- [ ] (r1vb) Reductions granted for later spells ("the next spell you cast this turn costs less", counts of opponents attacked or energy paid, the first face-down or X spell); frees about 12 cards (Kaza, Roil Chaser, Kadena)
- [ ] (c14a) The rest of the top coverage blockers: Multiclass Baldric, Yavimaya Bloomsage, a secretly chosen opponent, six spell lines and rules for the 61 other prepare cards; frees about 70 cards (Stalking Leonin, Ponder)
- [ ] (c13b) The rest of the commander free casts and strive spells; frees about 10 cards (Deflecting Swat, Twinflame, Reinterpret)

## Keywords and cast ways

- [ ] (c2g) Keywords given to spells beyond convoke, improvise and delve, and keywords with a cost given to cards in a hand or graveyard; frees about 20 cards (Ghost Ark, Past in Flames, Zhulodok)
- [ ] (c2d) The cards that print scavenge, transmute, awaken, gravestorm or sunburst and wait on another wording; frees about 15 cards (Brainspoil, Ondu Rising, Dodgy Jalopy)
- [ ] (c2l3a) Plot: the special action from a hand, the plotted card cast free as a sorcery on a later turn, plot cost reductions; frees about 4 cards (Lock and Load, Pyretic Charge)
- [ ] (c2l3c) The backup, emerge, harmonize, replicate, mayhem and afflict cards that wait on another wording, and "Discard any number of cards, then draw that many cards"; frees about 15 cards (Guardian Scalelord, Adipose Offspring, The Elder Dragon War)
- [ ] (r2da-h) "Do this only once each turn" after a may as its own limit: a declined may leaves the ability open for the turn (now read as a trigger limited to once each turn)
- [ ] (r2da-c-j) Exert as a cost part and mana whose spell gains haste (Arena of Glory, Carnelian Orb of Dragonkind, Oasis Ritualist, Fervent Paincaster)
- [ ] (r2da-c-k) A delayed trigger reads the targets of the resolution that set it up by last known information: "at the beginning of your next main phase, you may add an amount of {C} equal to that spell's mana value" after a won clash (Scattering Stroke)
- [ ] (r2da-c-g) Forecast: an ability activated from the hand during its owner's upkeep by revealing the card (Skyscribing)
- [ ] (r2da-c-e4) Mad Wizard's Lair: "draw three cards and reveal them. You may cast one of them without paying its mana cost" with the drawn cards bound, so Dungeon of the Mad Mage is offered
- [ ] (r2da-c-e5) The initiative cards: "the player who has the initiative" as a reference (Loot Dispute), "add {C}{C} instead" if you have the initiative (Sarevok's Tome), Seasoned Dungeoneer's attack trigger, From the Catacombs
- [ ] (r2da-c-e3) The venture cards that wait on another wording, and the player view of the dungeon and room (Radiant Solar, Thorough Investigation, Immovable Rod, Sefris of the Hidden Ways, Hama Pashar)
- [ ] (r2da-d) Emblem abilities the compiler does not read ("Whenever a creature attacks you, it gets +5/+5", an activated ability with "where X"), then the other clauses of the emblem planeswalkers; frees about 20 cards (Garruk, Apex Predator; Ob Nixilis of the Black Oath)
- [ ] (r2dga) Rules for the partners of accepted proposals, so both halves become corpus cards; frees about 8 cards (Ukkima, Stalking Shadow, Pippin, Warden of Isengard, Brallin, Skyshark Rider)
- [ ] (c9c) Verified corpus cards for Doctor's companion, Partner—text, Choose a Background and a planeswalker commander, once their other text runs
- [ ] (c3d) The monarch cards still held back: "can't attack you", "the monarch controls", stun counters after a group's damage, a delayed attack on the monarch; frees about 6 cards (Fealty to the Realm, Okoye, Regal Sliver)
- [ ] (c3b) Time counters on suspended cards and permanents, in the views of exiled cards too, and the blessing badge on the table
- [ ] (c4e) Cards that read the crew record and crew events and wait on a wording, and a Reconfigure corpus card; frees about 10 cards (Smogbelcher Chariot, Mindlink Mech, The Fantasticar)
- [ ] (c5e) The rest of the cipher and hideaway cards; frees about 6 cards (Hidden Strings, Shelldock Isle)
- [ ] (c6d-c) Devour X, devoured kinds and conditions: Thromok, Voracious Dragon, Hellkite Hatchling, Kresh, and a discard for each devoured creature (Tar Fiend); none of them is in a precon
- [ ] (c6d-g) A phase-out and an exile from the battlefield in one batch trigger The War Doctor once instead of once each
- [ ] (c6d-d) Gift "instead" spells left (Wildfire Howl, Dewdrop Cure, Into the Flood Maw) and entered-under-your-control-this-turn conditions (Zimone, Gargoyle Flock, Galadriel)
- [ ] (r2de-c) Cast permissions left: "once each turn" on any turn, a permission spell exiled instead of graveyarded, a card cast from its own graveyard or exile, spells chosen from a graveyard and cast free; frees about 20 cards (Kess, Squee, Dire Fleet Daredevil)
- [ ] (r2r) Alternative costs left: life or energy instead of mana, fixed mana, impending, X in madness, a paid madness cost as a condition; frees about 8 cards (Xander's Pact, Nashi, Fist of Suns)
- [ ] (r1aug) Extra turns and phases: a loss at the extra turn's end step, its restrictions, attacks alone, combats with chosen attackers, extra-turn costs; frees about 20 cards (Final Fortune, Medomai, Moraug)

## Statics, attachments and mutate

- [ ] (c7l) The attacking player who chooses the target and the mode of a trigger the Curse controls (Curse of Inertia)
- [ ] (c7m) Quoted abilities that are rules or make tokens, and the outlaw spend restriction (Nerd Rage, Tarmogoyf Nest, Discreet Retreat)
- [ ] (c7n) "they gain decayed" after a choice of a card in each player's graveyard (Ghouls' Night Out)
- [ ] (c7q) Block requirements: "must be blocked if able", "by exactly one creature", "all creatures able to block it do so"; frees about 8 cards (Madame Vastra, Predatory Impetus, Nacatl War-Pride)
- [ ] (c7s) "If the creature an opponent controls would die this turn, exile it instead" after a fight of two targets (Unnatural Aggression)
- [ ] (c7e) The rest of mutate: exile until X permanent cards, a merged permanent that returns, commander replacements of a merge, a token or face-down top (Auspicious Starrix)
- [ ] (c8ja) Life gain replaced by another effect (`ReplaceBy::Instead` in the engine): a loss instead, cards drawn instead, no life gained; frees about 6 cards (Tainted Remedy, Plague Drone, Nefarious Lich)
- [ ] (c8jb) Prevention for "you and" a group, quoted static abilities given under a condition, and the Slivers' quoted abilities left (flip, becomes a type, top of library); frees about 20 cards (Rune of Flight, The Wanderer, Pulmonic Sliver)
- [ ] (r1ab) The rest of the `Continuous` statics: types for spells and cards, every land type, a color added, group unions, "modified", attachment conditions; frees about 25 cards (Arcane Adaptation, Death Baron, Song of the Dryads)
- [ ] (r2az-c) Domain of another player and the effects around it: "lands they control", "lands that player controls" in an intervening "if", "unless its controller pays {1} for each", "prevent the next X damage"; frees about 8 corpus cards (Allied Strategies, Evasive Action)
- [ ] (r2az-g) The compiler reads an exile whose card returns later ("Return that card ... at the beginning of the next end step"), which the renderer writes; frees The Eternal Wanderer and the flicker cards only proposals reach
- [ ] (r2bhd) Untaps in other players' untap steps and groups held through their next untap step; frees about 8 cards (Seedborn Muse, Unwinding Clock, Lorthos, the Tidemaker)
- [ ] (r2u) Attached Auras and Equipment as a count, and attachments made by effects (any number, onto tokens, onto returned cards); frees about 15 cards (Kor Spiritdancer, Kemba, Goldwardens' Gambit)
- [ ] (r2db-a) Several statics, or a player rule and statics, in one sentence, and hexproof from the Aura controller's opponents; frees about 6 cards (Tenza, Godo's Maul, Nighthowler, Sigarda, Heron's Grace)

## Compiler wordings

- [ ] (t16b) Mana that marks the spell it pays for ("If this mana is spent to cast a creature spell, that creature enters with an additional +1/+1 counter on it"): needs the provenance of a mana unit in the engine; frees Biophagus
- [ ] (t16c) The follow-up of a destroy read and written by the compiler ("If an artifact is destroyed this way, its controller draws a card" as an `If` of `Performed`; the checker writes it, the compiler does not read it); frees Acolyte Hybrid
- [ ] (t16d) The single wording left on 29 Warhammer 40,000 cards: "the next spell you cast this turn has cascade", "must attack with at least one creature each combat", "life lost this turn" as an amount, "a card exiled with it", "choose ... at random"
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
- [ ] (r2jb) Trigger events left: getting energy, "first time" variants, "attacks a player and isn't blocked", a token copied, several events in one trigger; frees about 15 cards (Valgavoth, Kalamax, Territorial Gorger)
- [ ] (r2ad) Delayed triggers left: "until end of turn, whenever", with a target of their own, on the next spell, on every block, repeating this turn; frees about 7 cards (Infested Thrinax, Jailbreak, Ride the Avalanche)
- [ ] (r2dgb) Effects "unless that player sacrifices or pays", a sentence for each player, and a token copy after a payment; frees about 12 cards (Demanding Dragon, All Is Dust, Bramble Sovereign)
- [ ] (r2dgc-a) A cost for each mode chosen: Spree costs and escalate in the engine, the DSL and the compiler; frees about 4 cards (Requisition Raid, Great Train Heist, Collective Resistance)
- [ ] (r2dgc-b) Modal headers left: "Each mode must target a different player" and "Choose one. Activate only as a sorcery."; frees about 3 cards (Vindictive Lich, Shadrix Silverquill, Aetheric Amplifier)
- [ ] (r2dgc-c) Charm and commander-pair mode sentences: "each of one or two targets", distributed counters, "counter target spell that targets you", a power-limited "can't block", phasing and goad by a target player; frees about 15 cards (Prismari Charm, Abzan Charm)
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
- [ ] (r1auec) Die results left: a roll in a cost, "if the result is" conditions, each player rolls and the highest result, X dice and their total, dice ignored or added, "roll a 4 or higher" triggers; frees about 15 cards (Clay Golem, Chaos Dragon)
- [ ] (r1aued) Endeavors left: a search for a number of lands, power at least an amount, a return with that many counters, a free cast up to a mana value, "have ~ deal damage to itself"; frees 5 cards (Wild, Valiant, Grave and Arcane Endeavor, Vrondiss)
- [ ] (r1aui) Coins left: lose triggers, stop flipping, counted flips and heads, "Otherwise" after a flip; frees about 11 cards (Krark, the Thumbless, Fiery Gambit, Ral Zarek)
- [ ] (r1auha) Votes left: more than two choices (Council Guardian), names of two words (Travel Through Caradhras), "whenever players finish voting" (Erestor, Model of Unity, Grudge Keeper), votes for players and objects with counts (Círdan, Mob Verdict, Trap the Trespassers); frees about 15 cards
- [ ] (r1auhb) Villainous choices left: an additional choice, "Then" before the subject, "that opponent" and "each opponent who" subjects, a target's owner; frees about 10 cards (The Valeyard, This Is How It Ends)
- [ ] (r1auj) Piles that the controller separates or of permanents and targets (Steam Augury, Boneyard Parley, Make an Example, Whims of the Fates)

## Model-assisted reading

- [ ] (t17e) Slips the proposal repair still leaves: an ability whose printed line the abilities of its kind do not give one-to-one, a target used in an event, phrases the target reader does not read ("target player's library"), a group in a static (`ControllerOf` as `affected`), a player where an object is meant, an `X` standing for "the chosen number"
- [ ] (t19d) `mtg-cli compile` counts the cards with no rules text outside reminder text (they play as printed, nothing to store) apart from the cards it cannot read
- [ ] (t17f) Corpus cards and testkit scenarios for the attachment groups and counts of `t17b_attached.rs` (Kor Blademaster, Syr Armont, Lovisa Coldeyes, Kor Spiritdancer, Civic Saber, Thran Power Suit): fixtures from the Scryfall cache, rules files, a scenario with an attachment moved and removed
- [ ] (t17g) An amount over the object a group static acts on ("Each creature you control gets +1/+1 for each of its colors": Reaper King Avatar, Coat of Arms): the layers lower a modification once for the whole group, so the amount needs a `Ref` to the affected object
- [ ] (t17h) Writers for a quoted triggered ability granted by a spell to its target (Undying Malice, Supernatural Stamina), "Aura and Equipment attached to it" (Thran Power Suit), "for each Aura you control that's attached to a creature" (Sage's Reverie) and a target that becomes a color
- [ ] (t15b) Checker writers for four accepted-in-principle popular proposals: Boseiju, Who Endures (search for a basic-typed land), Takenuma (mill then return), Sink into Stupor ("target spell or nonland permanent an opponent controls") and Ragavan (exile the top card, cast it this turn)
- [ ] (t15c) A counter that exiles the spell instead of putting it into its owner's graveyard, and the free cast "if it's not your turn": Force of Negation
- [ ] (t15d) Mana whose spell can't be countered ("Spend this mana only to cast a creature spell of the chosen type, and that spell can't be countered"): Cavern of Souls, Delighted Halfling
- [ ] (t15f) Choose new targets or change the target of a spell or ability: Deflecting Swat, Untimely Malfunction
- [ ] (t15h) The event "you create a token" and the tally "created a token this turn": Mirkwood Bats, Idol of Oblivion
- [ ] (t15i) Mana of the colors among permanents ("any color among legendary creatures and planeswalkers you control", Vivid, the colors of the exiled card): Mox Amber, Bloom Tender, Chrome Mox
- [ ] (t15j) Cards drawn this turn as a selector (Sylvan Library), the number of colors in the commander's identity as an amount (War Room), and an alternative cost of life equal to the mana value paid from the top of the library (Bolas's Citadel)
- [ ] (t15k) A replacement of entering with a choice the engine asks (`ReplaceBy::Instead` with a `Pay`: Mox Diamond), and a draw-step exception on a draw trigger (Orcish Bowmasters)
- [ ] (t15l) Cards of the sweep not yet tried with the existing nodes: The One Ring, Malakir Rebirth // Malakir Mire, Urza's Saga, Nykthos, Shrine to Nyx (the color chosen before the devotion), Arcane Denial (a draw of up to two cards), Animate Dead, Gemstone Caverns (opening hand)
- [ ] (t15o) Three Tree City's `{2}, {T}` ability (a mana ability with a mana cost and a counted amount of one color) is offered among the mana abilities (`t15a_validate::t15a_v_three_tree_city_*`, ignored until then); demoting the card exposes seed 7626 of `house_commander::five_seats_*`
- [ ] (t20b-a) Replacements the checker cannot read because the proposals are wrong, so new proposals first: Laboratory Maniac (`Count[In(Library)]` needs `OwnedBy(You)`, then a writer for "while your library has no cards in it, you win the game instead"), Sulfuric Vortex, Barad-dûr, Loading Zone, Bloodletter of Aclazotz, Phyrexian Vindicator, The Mindskinner
- [ ] (t20b-b) One replacement for several token kinds applied to each token of a run ("instead create one of each": Academy Manufactor, `TokenCreation.what` as an `AnyOf` of subtypes) and the engine's `Instead` run once for every token
- [ ] (t20b-c) A replacement of the card itself from every zone ("reveal ~ and shuffle it into its owner's library instead": Blightsteel Colossus, Darksteel Colossus, Nexus of Fate; `MoveTo(Library)` runs as a move to the graveyard today, the rules are `Instead` of a reveal and a shuffled move)
- [ ] (t20b-d) Replacement events the DSL lacks: mana a permanent produces (Nyxbloom Ancient), cards milled (Bruvac the Grandiloquent), a creature's counters as a choice of the replaced event (Underrealm Lich, Library of Leng)
- [ ] (t20d) Triggered shapes whose stored rules are right but lack a primitive: Edgar Markov (works from the command zone and the battlefield), Bennie Bracks and Mirkwood Bats (created a token this turn), Y'shtola ("a player lost 4 or more life this turn"), Valgavoth (`Nth` of `LifeLost` per opponent), Lunar Convocation, Bloodchief Ascension
- [ ] (t20e) The other `Triggered:` rows of the gap ledger (effects If, May, Move, CreateToken, RepeatEach, BecomeCopy, DealDamage, Untap; about 40 cards): most stored rules of "the combination or its shape" are the model's mistakes (Sylvan Library, Vanquisher's Banner, Enduring Tenacity) and need a better proposal or a repair
- [ ] (t20c-a) A card named like the source as an amount ("for each card named ~ in each graveyard") and the mana sentence "Add {R}{R}, then add {R} for each ...": Rite of Flame
- [ ] (t20c-b) Writers for "unless that creature's controller pays {3}" after "if you're the defending player" (Kazuul, Tyrant of the Cliffs) and "Its controller can't cast spells this turn" after a counter (Render Silent)
- [ ] (t20c-c) Esper Sentinel: `Nth` of a `SpellCast` with a filter counts every spell the player cast this turn (`triggers/detect.rs`, `Tally::Cast`) where "their first noncreature spell each turn" counts the matching ones
- [ ] (t19c-a) Return the Favor (Spree), Season of Weaving and Season of Gathering (pawprint modes), Collective Resistance (Escalate and "choose one or more") read back once the cost for each mode chosen of r2dgc-a exists: the Spree and Escalate readers and the header writers
- [ ] (t19c-b) "Choose a creature type" while a spell resolves (Kindred Dominance, Crippling Fear, Distant Melody, Harmonized Crescendo): an effect that keeps the choice for the rest of the resolving spell and reads `ChosenSubtype` there, in the engine, the DSL, the compiler and the writers
- [ ] (t19c-c) A repair step that gives a `Choose` or `Search` the count the text prints when the proposal leaves the maximum out (`{"min":0}` is up to one: Ghalta, Kodama's Reach, Unwind, Intuition, Gifts Ungiven) and the five-color protection list for "protection from each color" (Akroma's Will)
- [ ] (t9a-c) The attack triggers that backup gives on Bright-Palm, Soul Awakener, Emergent Woodwurm and Mirror-Style Master (backup itself plays)
- [ ] (t9d) The bodies of the combat-only spells (Cauldron Dance, Mandate of Peace, Spinal Embrace, Take the Bait, Wake the Dead), and `CastOnly` read by a cast an effect makes
- [ ] (t9e) "Discard up to N cards, then draw that many cards": a discard of up to N whose count the draw reads (Daretti, Scrap Savant, Jaya Ballard, Cathartic Pyre)
- [ ] (t9f) Imprint artifacts: "you may exile an artifact card from your hand" as it enters and "{X}, {T}: Create a token that's a copy of the exiled card. X is the mana value of that card." (Prototype Portal, Soul Foundry)
- [ ] (t9g) Council's dilemma spells (Fateful Tempest, Selvala's Stampede, Travel Through Caradhras) and the Will of the Planeswalkers sentence beside other unread spells (Path of the Pyromancer, Path of the Schemer)
- [ ] (t9h) Chosen and counted statics the report names: "This creature is the chosen type in addition to its other types" (Metallic Mimic), enters with +1/+1 counters equal to the mana spent (Marath, Kurbis, Gyrus), "+N/+N for each Aura and Equipment attached to it" (Strong Back)
- [ ] (t9i) Hidden choices: "As this creature enters, secretly choose an opponent" and the "Reveal the player you chose" cost (Guardian Archon, Stalking Leonin, Emissary of Grudges); "While voting, you may vote an additional time" (Tivit, The Valeyard) after the voting spells of (t9c)
- [ ] (t12b1) "If you do" after a step that is no "may", sacrifice or counter removal needs the engine to report it as performed: Cartographer's Hawk (return it to hand), Astral Drift (exile target, also the cycle event), Next of Kin, Flickerform (class C)
- [ ] (t12b2) The reflexive follow-up after reveal-until and look-at-top steps, once those read (t12a): Aspiring Champion (and "if that creature is a Demon"), Fireflux Squad, Industrial Advancement, Cream of the Crop, Barrowgoyf (from among the milled cards)
- [ ] (t12b3) Amounts a follow-up names: Bebop (X counters of any kind on ~, the X of the second sentence), Neheb and Loamcrafter Faun ("that many" of the discard), Mortarion (X capped by the life lost), Grishnákh (the amassed Army's power)
- [ ] (t12b4) Costs and choices before the link: Kitt Kanto (tap two untapped creatures, "that player"), Myr Battlesphere (tap X Myr), Canoptek Wraith (pay and sacrifice, choose a land), Chorus of the Conclave (any amount of mana as an additional cost), Crosis (choose a color, discard all of it)
- [ ] (t12b5) Follow-ups with a new shape: Forum Filibuster (return an Aura attached to the token), Craig Boone, Human Torch (a delayed damage copy), Braids, Lavabrink Floodgates, Insatiable Frugivore (repeat), N'Yami-Class Mother Ship ("if you don't"), Kozilek's Return
- [ ] (t12b6) A reflexive trigger node (CR 603.12) that puts the follow-up on the stack with targets chosen then: Dream Eater ("surveil 4. When you do, ... target"), the targeted follow-ups after a sacrifice, and the payments read as `Pay.paid` whose target is chosen with the ability
- [ ] (t12a1) Takes from among the top cards, the cards left unread: a conjunction inside a list of kinds (Acclaimed Contender), one take per kind (Harper Recruiter), put "tapped and attacking" (Arthur), a delayed trigger on what was put (Aethermage's Touch), proliferate by the shortfall (Expand the Sphere), one card per keyword (Selective Adaptation)
- [ ] (t12a2) Choose a creature type or a card type as a spell resolves and take the cards of that type from the top (For the Ancestors, Winding Way)
- [ ] (t12a3) Reveal until a card matches, the follow-ups left unread: all matches to one place and the rest to the bottom or shuffled (Old Stickfingers, Mass Polymorph), another player reveals (Proteus Staff, Collision of Realms), shares a type (Reality Scramble), Oath of Druids, Abundance
- [ ] (t13a) The one-off noun-phrase readers (the chosen type in `statics.rs` and `spend.rs`, cost subjects, `Targeting` in `cost_conditions.rs`) call the shared reader, output byte for byte the same
- [ ] (t13f) Sentence readers around the atoms that exist: free casts of a spell with mana value up to an amount, a count before a zone ("lands you control from your graveyard"), commander phrases (color identity, "a commander you own"), and adjectives in trigger subjects
- [ ] (t13g) Atoms that still lack an engine fact: "targets only", a type or mode a spell chooses as it resolves, "was activated this turn", paired
- [ ] (t12c-a) A modification that adds colors ("it's a black Zombie in addition to its other colors and types"; `SetColors` replaces), then the reanimation sentences of Rise from the Grave, Grimoire of the Dead and Necromantic Selection
- [ ] (t12c-b) A token replacement that keeps the original tokens ("those tokens plus that many 1/1 Squirrel tokens are created instead"; `ReplaceBy::Instead` drops them): Chatterfang, Donatello, Stridehangar Automaton
- [ ] (t12c-c) Athreos: "dies or is put into exile" and a "return that card" ref that finds a card in exile as well as in a graveyard
- [ ] (t12c-d) Control sentences that each need one more piece: a target player other than the owner (Crown of Doom), a random target opponent (Witch Hunt), "when you lose control" (Ray of Command), a text change (New Blood), the group exchange of Reins of Power, and the look-and-put forms of Zara, Oblivion Sower and Lonis
- [ ] (t12c-e) "Unless" forms that each need one more piece: a player becoming a target (Unsettled Mariner), a source's controller losing life unless they sacrifice (Archfiend of Spite), "sacrifices a creature or pays 3 life" (Indulgent Tormentor), abilities that can't be activated (Faith's Fetters), the tax on abilities (Tithe Taker)
- [ ] (t12c-f) In a "becomes the target" trigger `ControllerOf(Triggering)` names the controller of the targeted object as well as of the spell or ability: Bonecrusher Giant deals its 2 damage to both players; the cause names the targeting controller alone for it
- [ ] (t13b) A general amount language: `Count(Selector)` over every zone, a generic this-turn tally in place of the separate `TurnTally` variants, amounts from the triggering or chosen object, one "the number of ..." reader (about 150 tail cards)
- [ ] (t13c) A generic lasting-effect object with typed statics and triggers, bound objects and an end condition ("for as long as", "until your next turn"), the old duration nodes kept (about 200 tail cards, touches layers and lifetimes)
- [ ] (t13d) Optional parameters on the big node families instead of new nodes: a library position and several origin zones on `Move`, division among targets on `PutCounters`, an `in` zone on `Continuous`, copy exceptions, "different names" (3 to 15 cards each)
- [ ] (t13e) A cost item that wraps an effect (`CostItem::Do`) and one rule node for "X can't happen" (can't be cast, can't win or lose, triggers don't fire, untap in another player's untap step); about 65 tail cards
- [ ] (t21a) The cards whose stored proposal differs from the printed text are proposed again after each writer round: the gap ledger (`check-rules --gaps`) names them and `propose --resume` retries them
- [ ] (r4d-h) `Ability.ManaAbility.Other`: a mana ability with a sacrifice of X permanents whose X is the mana (Springjack Pasture, Hazel of the Rootbloom)
- [ ] (r4d-e) A count of transformed permanents (Mutagen Connoisseur)
- [ ] (r4d-f) Alternative costs past a "you control" condition: "If the {1}{B} cost was paid" (Baleful Mastery), "If a white creature is attacking" (Nemesis Trap), the other Traps and Invigorate
- [ ] (r4d-g) A blocker of several attackers: its controller divides its damage, "Whenever ~ blocks" triggers once, and the web names a blocker for several attackers
- [ ] (r4d-i) Writers left for valid r4c proposals: "Enchant red or green creature" in the keyword reader (Controlled Instincts), prevention of two groups by creatures (Eerie Interference), "creatures that are still lands" (Living Lands), "any other target" after an `AnyTarget` (Arc Trail, Explosive Welcome); Elite Inquisitor and Oversoul of Dusk need new proposals
- [ ] (r4b-e) Writers left for valid r4a proposals: a granted untap in each other player's untap step (Urban Burgeoning), a chosen creature as the source (Crush Underfoot), "the difference" (Balance of Power), "chooses three permanents, then sacrifices the rest" (Razia's Purification)
- [ ] (r4b-f) `mtg-cli promote` takes a card whose rules name the card itself (Pack Mastiff, accepted `Verified`, is skipped as naming a card outside the corpus)
- [ ] (r4e-a) Writers left for valid r4e proposals: protection from three colors or subtypes and "hexproof from artifacts and enchantments" in the keyword reader, "until end of combat" (Murk Dwellers), "you've drawn two or more cards this turn" (Spinehorn Minotaur), "twice X" (Sanguine Sacrament), a counter on "it" after a return (Miraculous Recovery), a chosen creature put on top of its library (Nulltread Gargantuan)
- [ ] (r4b-c) Engine and DSL pieces the accepted-Partial r4a proposals need: a land that doesn't untap next untap step (Cinder Marsh), +N/+N for each Aura attached (Auramancer's Guise), a card revealed at random from the hand (Goblin Secret Agent), each player gains control of what they own (Homeward Path), a life total that becomes N (Magister Sphinx), `Effect.Pay.DiscardSelf` (Hag of Mage's Doom), `Cost.OneOf` (Heartwood Shard), perpetual (Begin Anew)
- [ ] (r3w8b) Trigger writers left: a spell cast with mana value greater than a count (Mizzix of the Izmagnus), "if it isn't that player's turn" (Tataru Taru), a chapter whose target becomes an artifact (Phyrexian Scriptures)
- [ ] (r3w9a) Spell shapes left: divided damage among any number of targets with more sentences (Magma Opus), "any number of target players each" (Priest of Forgotten Gods), "a creature of their choice" with its toughness (Consuming Vapors), an intervening land count (Archaeomancer's Map), "Then if" (Éowyn, Shieldmaiden), costs per a count (Mizzix, Myth Unbound)
- [ ] (r3w2a-a) A writer for a group that becomes a creature "and gains flying" (Sarkhan the Masterless)
- [ ] (r3w2b) Writers for granted abilities in quotes whose trigger or effect is unwritten (The Reaver Cleaver, Unquenchable Fury), Clues that become Equipment (Armed with Proof), "creatures you control with toxic" (Skrelv's Hive) and an artifact type added without end (Phyrexian Scriptures)
- [ ] (r3w3b) Steps under "you may" left: a token copy with exceptions, "reveal that many cards", "the sacrificed permanent", "that player may ... if" and an exile of the card from the graveyard as a cost (Nightmare Shepherd, Glint Raker, Shadow, Ludevic, Kozilek's Return)
- [ ] (r3w1a) Conditional steps left: a card chosen at random (Deadbridge Chant), the top card revealed (Druidic Satchel), "remove all of them" with a named token (Replicating Ring), "choose target" before the condition (Meren of Clan Nel Toth), phasing with a rule until the next turn (Perch Protection)
- [ ] (r3w4b) Delayed triggers and repeated steps left: a loyalty ability's delayed return (Nahiri, the Harbinger), steps repeated for each object ("each Dragon you control", "each of those creatures"), a delayed trigger on its own line (Forth Eorlingas!)
- [ ] (r3w6a) The A to L mismatches left: "you may have it fight", "you and target opponent each create", "another target player", a token "that's all colors", one of two counter removals as a cost; Commodore Guff, Kazuul and Gyome need a rewrite
- [ ] (r3w5a) Writers for the rules of the r3 samples: a draw replaced after the first in a draw step (Notion Thief), a type and a granted ability during your turn (Gourmand's Talent)
- [ ] (r2ec) A specialist model: `mtg-cli export-pairs` writes card text and rules pairs from the Verified cards with a held-out split, decoding is limited to the DSL grammar (`dsl-schema`), and a LoRA fine-tune with a short prompt replaces the 22K-token reference
- [ ] (r2cf) A card's name in its token or a name that starts with a creature type reads as `~` in both the checker and the corpus calibration (Kher Keep, Tuktuk the Explorer, Sliver Hivelord)

## Layouts

- [ ] (c11) The initiative, dungeons and the remaining long-tail mechanics found by the report, one task each
- [ ] (c11c) The other 24 transforming Sagas, the two precon flip cards and the `prepare` layout's back face; frees about 37 cards (Budoka Gardener, Nezumi Graverobber)
