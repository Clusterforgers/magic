# Cards backlog

Open card, compiler and wording items by code area; the rules of `docs/ROADMAP.md` apply (one line per item, claims, ids). The gap ledger (`mtg-cli check-rules --gaps`, `coverage --all`) ranks them by the popularity of the cards they hold back. Design: `docs/cards-dsl.md`.

## Generality

- [ ] (gn1a) Blame names the unwritten part of a `Replacement`, `Rule` and `CostModifier` ability instead of "the combination"; splits 54 ledger cards into node gaps (Academy Manufactor, Harmonic Prodigy, Urza's Incubator); `mtg-cards/src/proposal/blame.rs`
- [ ] (gn1b2) Reader atoms and writers for the general amounts (`BasePower`, `Distinct` colors, `CommanderColors`, `TriggeringTargets`, `Tally`, `TurnNumber`, `StartingPlayer`) and the amount of damage prevented this way; War Room, Faeburrow Elder, Kutzil, Idol of Oblivion, Ashling, Flame Dancer; `compile/read/amounts.rs`, `facts.rs`, `resolve/mod.rs` (Resolving record of prevented damage)
- [ ] (gn1e2) Mana abilities still `Ability.ManaAbility.Other`: an amount that depends on the chosen colour (Nykthos, Shrine to Nyx: devotion to that colour), effects before the mana beyond a counter on the source, a granted mana ability (Urza's Saga), and a mana-free cost without {T}; `mana_shape` in `mtg-cards/src/assemble.rs`, `mana_ability.rs`
- [ ] (gn1i) Zone statics: `Static.affected` on the stack, hand, graveyard or exile for cast keywords, `uses` for "the next spell" and `Shield.uses` for "the next time"; frees about 11 ledger cards (Imoti, Lier, Archway of Innovation, Words of Worship); `ability.rs`, `effect.rs`, engine casting
- [ ] (gn1j) Writer composition: a frame writes with parts that write alone, for the combinations gn1a names (after gn1a); frees about 27 ledger cards (Sylvan Library, Vanquisher's Banner, Emry, Lurker of the Loch); `compile/read/render`
- [ ] (gn1k) Activated-ability modifiers: a `CostModifier` with `change: Instead(cost)` and loyalty abilities activated twice (`activating` already names the ability kind); frees about 10 ledger cards; `ability.rs`, engine announcement
- [ ] (gn1la) A permanent that returns to the battlefield with its card types set ("it's an enchantment", Enduring Vitality); `Destination::Battlefield`
- [ ] (un34s) Sticker sheet lines no reader knows (16 sheets `Partial`, `PARTIAL` in `tests/it/un34_sheets.rs`): protection from even or odd mana values and from creatures with two or more creature types, Heroic, Metalcraft and Threshold statics, cast from the graveyard, switch power and toughness, a flicker under its owner's control, "mills twice that many", divided damage by its power, "can't block this creature", "blocks this creature", Morbid at each end step, a dice-roll replacement, generic mana of the cast spell, a static and a trigger on one sticker; and Space sculptor (Space Beleren: sectors); `compile/un34_sheets.rs`
- [ ] (r378b) A tap logged with the cost it paid ("becomes tapped to pay a teamwork cost": Agent Maria Hill); `mana_ability.rs`, `detect/did.rs`
- [ ] (r378c) Text changes: every color word changed to the chosen one (Swirl the Mists); `text_change.rs`, layers
- [ ] (r378d) Turn history: damage by sources of a name (Blazing Effigy); `turn_log.rs`, `turn_summary.rs`
- [ ] (p158b) Engine pieces of pack158-other still open: "unattach an Equipment from ~" as a cost whose mana value is the divided damage (Captain America, First Avenger), a play of a card exiled with the source as a trigger (Prowl), beam me up (Scotty, Miracle Worker)
- [ ] (cx8b) Myra the Magnificent: a midway counter on a chosen Attraction and a visit of that Attraction copying the exiled card while it stays; `memory.rs`, `un33_attractions.rs`
- [ ] (p162a) A look at face-down exiled cards that a player keeps once granted, after the source leaves or changes control (Grimoire Thief rulings): `LookAtExiled` grants it only to the current controller

## Forge coverage

Ordered by scripts freed per piece (`mtg-cli translate-forge`, `forge-check`). `gn1a` and `gn1j` are writer items beside this list; a new decision comes with its web prompt in the same piece.

- [ ] (ep21b) A variable number of flips with the wins counted after them (Yusri); repeats whose loop state is a stored count (`StoreSVar`: Delina, Mana Clash, Fiery Gambit), another player deciding to go on (Game of Chaos), a choice before the first pass (Dance with Calamity) and Tainted Pact
- [ ] (r5513a) An uncalled flip ("if it comes up heads") has no winner or loser: `CoinFlipped` marks it, so `CoinFlipWon` and `Did LostCoinFlip` skip it
- [ ] (ep21c) Forge `RaiseCost` of mana abilities (`ValidSpell$ Activated.ManaAbility | Cost$ PayLife<1>`: Thran Portal) onto `CostModifier.activating` Mana; `translate/statics.rs`
- [ ] (fg5eb) Token halving (Halving Season: `ReplaceBy::Half` over `TokenCreation`); `translate/replacements_token.rs`, `replace.rs`
- [ ] (fg5ec) `FirstTime$` on batches of a card kind: `DiscardedAll` with `ValidCard` (Veronica, Dissident Scribe) and `ChangesZoneAll` (Crawling Sensation); `translate/triggers_batch.rs`, `triggers_more.rs`
- [ ] (gn1p) `RemoveCountersAmong` lets the payer distribute the removed counters instead of taking them in play order (a new decision with web support); `mtg-engine/src/parts/among.rs`
- [ ] (fe1s) Actions of `Did` that need an event first (`Voted`, `Sought`, `AbilityResolved`, `Stationed`, `Trained`, `GameStarted`) with web support, and the reader and writer wordings of `Did`; `detect/did.rs`
- [ ] (ep26a) Changes of targets with a condition on the old target (Meddle, Quicksilver Dragon: "only one target and that target is a creature"); `replace.rs`, `resolve/prim/retarget.rs`
- [ ] (ep27a) Engine families of the API notes (`ep27-families.tsv`) still open: boast, exhaust and ninjutsu as mana spending restrictions (an exhaust marker distinct from "Activate only once"); a random look at one card of a hand (Urza's Bauble); conditional flash that checks the spell (its X, targets or teamwork); `replace.rs`, `parts/`, `activation_kind.rs`
- [ ] (ep28a) History and duration families still open: Forge `Effect | EffectOwner$` with a turn duration ("until that player's next turn": Sphinx's Decree, Willie Lumpkin) and previous-turn object tallies; target players of other facts (dealt damage by this source, more X than the active player for the Oaths, other than the card's owner or the enchanted player); Forge's `UntilYourNextUpkeep` still maps to `UntilYourNextTurn` (a g13h test pins it); entering with a variable or several counter kinds; `effects.rs`, `turn_summary.rs`, `translate/valid.rs`, `translate/effects_obj.rs`
- [ ] (ep30a) Open families: a damage shield over two recipients whose prevented damage is dealt elsewhere (Channel Harm, Comeuppance, Refraction Trap: `DamageDone-DealDamage`, `ReplaceDamage`) or gains life from a source of a colour (Shadowbane), a remembered player and their permanents (Lightning, Army of One); two repeatable costs counted apart (Primitive Justice); "any number of" counters between two targets with the same controller (Bioshift); `replacements_damage.rs`, `ep30_paid.rs`, `effects_verbs.rs`
- [ ] (ep31a) Copy and predicate leftovers: an ability copy from a colorless or emblem source (Abstruse Archaic), a named token attached on entering (Domesticated Mammoth), copies of a spell with `ChooseOnlyOne`/`CantCopy` (Gogo), "has an activated ability" of other shapes (`hasAbility Activated`), `firstTurnControlled`, `attackedLastTurn`, `SharesCMCWith`; `modify.rs`, `rules.rs`, `translate/effects_effect.rs`, `translate/effects_verbs3.rs`, `translate/filter.rs`
- [ ] (ep26b) A mana ability whose amount reads the exiled creature (Food Chain: one way per creature the cost may take); `parts/`, `mana_ability.rs`
- [ ] (gn1ha) Combat and lock leftovers: Forge `CantHaveKeyword$` without a matching `RemoveKeyword$` and of `AnimateAll`, `RemoveKeywords$` of `AnimateAll` onto `LockKeyword`, "must be blocked by exactly one / two or more creatures" (`MaxBlockers`, `MinBlockers` with `MustBeBlocked`), `IgnoreShroud`, statics carried by an `Effect` SVar (`StaticAbilities$ CanAttack`), ward that does not trigger under `IgnoreHexproof`; `translate/statics_rules.rs`, `compile/read`
- [ ] (fe1q) Keyword macro that waits for another node: Double team (a conjured duplicate: `fe1j`); `macros.rs`, `compile/keywords.rs`
- [ ] (fe1h) `Duration::Perpetual`: a modification that follows the card across zones (Alchemy), and clearing it; frees 158 scripts (Begin Anew, Cottontail Caretaker, Ethereal Grasp); `modify.rs`, engine card identity and `layers.rs`
- [ ] (fe1j) `Heist` and intensity (`Starting intensity`, `Intensify`, `Amount::Intensity`) beside the existing `Effect::Conjure`; frees scripts with spellbook cards (Argothian Uprooting, Bladehold Cleaver); `effect.rs`, `resolve/prim/`, web spellbook prompt
- [ ] (fe1u) The manual panel (`manual.rs`) obeying `Cant` beyond drawing and life changes (the other manual actions bypass prohibitions); `prohibit.rs`, `manual.rs`
- [ ] (fe1v) `Effect::RemoveDamage` with an amount (`DamageRemoved` carries the amount); frees the remaining `fe1n` scripts; `resolve/prim/fe1n_damage.rs`
- [ ] (fe1mg) Craft: materials beyond one type with a count (several kinds, "four or more") and the back faces that read the cards exiled with the card (`ExiledWith`, shared keywords); `keywords/craft.rs`, `parts/exile.rs`
- [ ] (fe1mh) Assist: an assisted payment can be backed out of with the helper's mana returned; `announce/assist.rs`
- [ ] (fe1mf) `AltCast.then`: the abilities of a way carried on the node and gated by `CastWith` by the engine (a new `AbilitySource`), so a table row holds its riders; and `Effect::CastFromZone { way }` for the table ways
- [ ] (fe1ka) Costs in the announcement that are left: flash at a price and the compiler readers and renderers for Spree and Tiered mode costs (Requisition Raid, Great Train Heist); `keyword.rs`, `effect.rs`, `announce/price.rs`
- [ ] (fe1oa) `Waterbend` in an optional additional cost and in ward of a spell: the helpers are offered for activated abilities and required additional costs only; `cost.rs`, engine `price.rs`, `announce/total.rs`
- [ ] (fe2) Split into pieces the rest of the matrix: `MayLook`, `GainAbilitiesOf`, `AssignAsUnblocked` (184 scripts), planar deck and die (176), Vanguard (106), Archenemy (102), stickers (88), contraptions (66), the Ring (54), mana replacement and conversion (52), conspiracies (50), speed (49), durations (48), attractions (45), `ReplaceWhen::Did` (41), `SetStatus` with Case (40), library position N (36), outside the game (26), combat changes (19), specialize (19), Mindslaver (15), meld (14), text changes (14), turn face down (11), ante (9), ownership and turn order (8), one-offs (63); `orch/forge-matrix/matrix.tsv`

## Costs and casting

- [ ] (g2e) The mode prompt offers a mode with X in its target filter only when some payable X has its targets, so choosing a mode never backs the cast out
- [ ] (g5c) Convoke and delve picks are public to every seat in the protocol projection, with a hidden-information test
- [ ] (g6d) Cost parts other than mana for multikicker, suspend, overload and foretell; frees about 5 cards (Arrogant Poet, Owlin Shieldmage, Skullmane Baku)
- [ ] (r6003c) Names beyond one chosen as the permanent enters: a land name chosen by a trigger (Petrified Hamlet)

## Faces and copies

- [ ] (n1d6) The day and night wordings left; frees about 10 corpus cards (Celestus Sanctifier, Angel of Eternal Dawn)
- [ ] (n2b) Face-down remaining: face-up and face-down predicates, turn-up conditions, morph costs beyond mana; frees about 14 cards (Write into Being, Whisperwood Elemental, Fathom Seer)
- [ ] (ep33a) Open families: a secretly chosen player, "you choose how each player votes", face-down permanents with set characteristics (a Forest land), "whenever you give a gift", spend on face-down creature spells, morphing up, equip, kicked spells and cumulative upkeep, and writers for the exile-memory drafts (Cemetery Protector, Wormfang Crab)
- [ ] (o2fa) "When you next cast" left: inside "If you do," a chapter with an ability word or a mana ability, "an additional time", a spell or ability, and effects other than a copy (counters, sunburst, rebound) (Magus Lucea Kane)
- [ ] (o2fb) Copy exceptions that set a name or depend on the copy (Sakashima, Lazav); frees about 9 cards
- [ ] (o2fc) Token copies that attack a player ("for each opponent, create a token copy that attacks that opponent") and storm riders (Skull Storm's "each opponent who can't loses half their life"); frees about 10 cards (Skull Storm, The Scarab God)
- [ ] (o2fd) Populate X times and populate for each (Full Flowering)
- [ ] (o1d) A copy-granted static reads its trigger from the copy that left in the same batch

## Attachments

- [ ] (m4) A Fortification that is also a creature (CR 301.6): the attachment check and a test

## Mana and costs

- [ ] (c1j) A mana ability that takes permanents and is used more than once in a payment may take its own source with its last use (Skirk Prospector)
- [ ] (c1k) The triggered mana abilities of a nonland permanent and with a condition (Kinnan, Bonder Prodigy)
- [ ] (c1l) Mana abilities still unread: Domain counts, X mana by a where clause, counted sacrifices, mana not for generic costs (Springjack Pasture)
- [ ] (r2d) Costs in the engine: `PayLife.Counted`, `PayEnergy.Counted`, `Discard.Counted`, `Untap`, `OneOf`, `ReturnToHand`, `Pay.ExileSelf`; frees about 12 cards (G'raha Tia, Confiscation Coup, Order of Whiteclay)
- [ ] (r1va) Reductions of other activated abilities: "abilities of Foods", "the first activated ability each turn"; frees about 12 cards (Sam, Loyal Attendant, Tezzeret, Betrayer of Flesh)
- [ ] (r1vb) Reductions granted for later spells: counts of opponents attacked or energy paid, the first face-down or X spell; frees about 12 cards (Kaza, Roil Chaser, Kadena)
- [ ] (c14a) The rest of the top coverage blockers: Multiclass Baldric, six spell lines and rules for the 61 other prepare cards; frees about 70 cards (Ponder)

## Keywords and cast ways

- [ ] (c2g) Keywords given to spells beyond convoke, improvise and delve, and keywords with a cost given to cards in a hand or graveyard; frees about 20 cards (Ghost Ark, Past in Flames)
- [ ] (c2d) The cards that print scavenge, transmute, awaken, gravestorm or sunburst and wait on another wording; frees about 15 cards (Brainspoil, Dodgy Jalopy)
- [ ] (r2da-c-k) A delayed trigger reads the targets of the resolution that set it up by last known information: "at the beginning of your next main phase, you may add an amount of {C} equal to that spell's mana value" after a won clash (Scattering Stroke)
- [ ] (r2da-c-e4) Mad Wizard's Lair: "draw three cards and reveal them. You may cast one of them without paying its mana cost" with the drawn cards bound, so Dungeon of the Mad Mage is offered
- [ ] (r2da-c-e5) The initiative cards: "add {C}{C} instead" if you have the initiative (Sarevok's Tome), Seasoned Dungeoneer's attack trigger, From the Catacombs
- [ ] (r2da-c-e3) The player view of the dungeon and room
- [ ] (c3d) The monarch cards still held back: "can't attack you", "the monarch controls", stun counters after a group's damage, a delayed attack on the monarch (Regal Sliver)
- [ ] (c3b) Time counters on suspended cards and permanents, in the views of exiled cards too, and the blessing badge on the table
- [ ] (c4e) Cards that read the crew record and crew events and wait on a wording (Smogbelcher Chariot)
- [ ] (c5e) The rest of the cipher and hideaway cards; frees about 6 cards (Shelldock Isle)
- [ ] (c6d-c) Devour X, devoured kinds and conditions: Hellkite Hatchling, Kresh; none of them is in a precon
- [ ] (c6d-g) A phase-out and an exile from the battlefield in one batch trigger The War Doctor once instead of once each
- [ ] (c6d-d) Gift "instead" spells left (Wildfire Howl, Dewdrop Cure, Into the Flood Maw) and entered-under-your-control-this-turn conditions (Zimone)
- [ ] (r2de-c) Cast permissions left: spells chosen from a graveyard and cast free (Dire Fleet Daredevil)
- [ ] (r2r) Alternative costs left: energy instead of mana, a paid madness cost as a condition (Xander's Pact)
- [ ] (r1aug) Extra turns and phases: its restrictions, attacks alone, combats with chosen attackers, extra-turn costs; frees about 20 cards (Medomai, Moraug)

## Statics, attachments and mutate

- [ ] (r6013c) Keyword actions done again: assembling more Contraptions (Steamflogger Boss)
- [ ] (c7l) The attacking player who chooses the target and the mode of a trigger the Curse controls (Curse of Inertia)
- [ ] (c7m) Quoted abilities that are rules or make tokens (Nerd Rage)
- [ ] (c7n) "they gain decayed" after a choice of a card in each player's graveyard (Ghouls' Night Out)
- [ ] (c7s) "If the creature an opponent controls would die this turn, exile it instead" after a fight of two targets (Unnatural Aggression)
- [ ] (c7e) The rest of mutate: a merged permanent that returns, a token or face-down top
- [ ] (c8jb) Prevention for "you and" a group, quoted static abilities given under a condition, and the Slivers' quoted abilities left (flip, becomes a type); frees about 20 cards (Rune of Flight, The Wanderer)
- [ ] (r1ab) The rest of the `Continuous` statics: types for spells and cards, every land type, a color added, group unions, "modified", attachment conditions; frees about 25 cards (Arcane Adaptation, Death Baron, Song of the Dryads)
- [ ] (r2az-c) Domain of another player and the effects around it: "lands they control", "lands that player controls" in an intervening "if", "unless its controller pays {1} for each", "prevent the next X damage"; frees about 8 corpus cards (Allied Strategies)
- [ ] (r2u) Attachments made by effects (any number, onto tokens, onto returned cards) (Goldwardens' Gambit)
- [ ] (r2db-a) Several statics, or a player rule and statics, in one sentence, and hexproof from the Aura controller's opponents; frees about 6 cards (Tenza, Godo's Maul, Nighthowler, Heron's Grace)

## Compiler wordings

- [ ] (lx1a) The enter exiles linked to "return the exiled card" still left by hand: a pick from a revealed hand (Tidehollow Sculler, Mesmeric Fiend), cards from a single graveyard (Gravegouger), the same-name exile (Detention Sphere), permanents and graveyard cards together (Angel of Serenity) and the name rule of Ashiok's Erasure; `compile/linked.rs`
- [ ] (dl1) The four Delves boss deck cards on the content test's pending list play automatically: Leonin Relic-Warder (its compiled linked triggers read back as printed: a corpus card), Wayta, Trainer Prodigy (an activation's reduction counts its own targets), Ripples of Potential (proliferate binds the permanents it put counters on), Plumb the Forbidden (its draft lacks "copy this spell for each creature sacrificed this way", and the checker files that sentence under the additional cost).
- [ ] (dl2) The 146 boss deck cards that play from Forge or read drafts reach the test corpus, so the simulator plays them: a corpus that loads approved drafts, or drafts the checker accepts.
- [ ] (t16c) The follow-up of a destroy read and written by the compiler ("If an artifact is destroyed this way, its controller draws a card" as an `If` of `Performed`; the checker writes it, the compiler does not read it); frees Acolyte Hybrid
- [ ] (t16d) The single wording left on Warhammer 40,000 cards: "the next spell you cast this turn has cascade", "life lost this turn" as an amount, "a card exiled with it", "choose ... at random"
- [ ] (c10) Unread spell, trigger and activated wordings ranked by `mtg-cli coverage`: one piece per family left (quoted tokens, goad, counters, pump, manlands, graveyard, flicker, damage, draw and discard)
- [ ] (c10s-a) Mana spent to cast a spell as an amount ("the amount of mana spent to cast this spell", "that spell"): Wurmquake, Manaform Hellkite, Prossh, Gyrus, Kurbis, Marath
- [ ] (c10s-b) Differences and "whichever is greater" as amounts: Sandstone Oracle, Slithermuse, Hit the Mother Lode, Tales of the Ancestors, Prophet of the Scarab, Willowdusk
- [ ] (c10s-c) Counts of another player ("the number of Islands target opponent controls", "creatures target player controls")
- [ ] (c10s-e) Coven and names left: a conditional cast permission (Augur of Autumn), differently named tokens and different names (Gimbal, Neriv, Liliana's Contract)
- [ ] (c10d) Top-of-library wordings left: reveals with a follow-up, each player's top cards, a land put from a look (Risen Reef)
- [ ] (c10e) Subtype and type nouns, legendary names without a comma as `~`, and the events "enters from your graveyard", "is turned face up" and "modified creature"
- [ ] (c10r) The flicker with a delayed return, the exile that returns when the source leaves for an Aura with several hosts, and "until your next turn" durations
- [ ] (c13h) A token when N or more cards leave your graveyard
- [ ] (r2jb) Trigger events left: "first time" variants, a token copied, several events in one trigger (Valgavoth)
- [ ] (r2ad) Delayed triggers left: on every block, repeating this turn
- [ ] (r2dgc-b) Modal header left: "Each mode must target a different player" (Vindictive Lich, Shadrix Silverquill)
- [ ] (r2aa-a) Control changes left: each player taking back what they own (Homeward Path, Alicia Masters), "when you lose control" (Ray of Command), the group exchange of Reins of Power, and the look-and-put forms of Zara, Oblivion Sower
- [ ] (r2cxe-b) A part of a payment in an effect that counts X ("you may pay X life", `Effect.Pay.X.Parts`)
- [ ] (r2ab) A color, creature type or card type chosen as a spell resolves or by each player; frees about 10 cards (Sudden Demise, Wash Out, Kindred Charge)
- [ ] (r2aca) Selectors left: what was exiled this way (Declaration in Stone)
- [ ] (r2bgd) Damage left: redirected and to battles (Heroic Sacrifice)
- [ ] (r1ap) Zone moves left: "instead" branches that name targets or counts, returned permanents with counters, haste or attachments, and cards of a chosen type; frees about 15 cards (Tear Asunder)
- [ ] (r2q) Picks at random from a hand or library, with a per-seat projection for hidden zones
- [ ] (r1ac) Token wordings left: tokens for other players and counts of mana spent or damage dealt (Death by Dragons)
- [ ] (r1aufb) Targets for each opponent or player left: without "up to one", a chosen creature each, the pass's player named again; frees about 15 cards (Hideous Taskmaster, Windgrace's Judgment, Guff Rewrites History)
- [ ] (r1auec) Die results left: "if the result is" conditions
- [ ] (r1aued) Endeavors left: a search for a number of lands, power at least an amount, a return with that many counters, a free cast up to a mana value, "have ~ deal damage to itself"; frees 5 cards (Wild, Valiant, Grave and Arcane Endeavor, Vrondiss)
- [ ] (r1aui) Coins left: lose triggers, stop flipping, "Otherwise" after a flip; frees about 11 cards (Krark, the Thumbless, Fiery Gambit)
- [ ] (r1auha) Votes left: more than two choices (Council Guardian), names of two words (Travel Through Caradhras), votes for players and objects with counts (Círdan); frees about 15 cards
- [ ] (r1auhb) Villainous choices left: "Then" before the subject, "that opponent" and "each opponent who" subjects, a target's owner; frees about 10 cards (This Is How It Ends)
- [ ] (r1auj) Piles that the controller separates or of permanents and targets (Steam Augury, Boneyard Parley, Make an Example)

## Checker and proposals

- [ ] (t17e) Slips the proposal repair still leaves: an ability whose printed line the abilities of its kind do not give one-to-one, a target used in an event, phrases the target reader does not read ("target player's library"), a group in a static (`ControllerOf` as `affected`), a player where an object is meant, an `X` standing for "the chosen number"
- [ ] (t19d) `mtg-cli compile` counts the cards with no rules text outside reminder text (they play as printed, nothing to store) apart from the cards it cannot read
- [ ] (t17f) Corpus cards and testkit scenarios for the attachment groups and counts of `t17b_attached.rs` (Kor Blademaster, Syr Armont, Kor Spiritdancer, Civic Saber, Thran Power Suit): fixtures from the Scryfall cache, rules files, a scenario with an attachment moved and removed
- [ ] (t17h) Writers for a quoted triggered ability granted by a spell to its target (Undying Malice, Supernatural Stamina), "for each Aura you control that's attached to a creature" (Sage's Reverie) and a target that becomes a color
- [ ] (t15b) Checker writers for two accepted-in-principle popular proposals: Boseiju, Who Endures (search for a basic-typed land) and Sink into Stupor ("target spell or nonland permanent an opponent controls")
- [ ] (t15d) Mana whose spell can't be countered ("Spend this mana only to cast a creature spell of the chosen type, and that spell can't be countered"): Cavern of Souls, Delighted Halfling
- [ ] (t15h) The event "you create a token" and the tally "created a token this turn": Mirkwood Bats, Bennie Bracks, Idol of Oblivion
- [ ] (t15j) An alternative cost of life equal to the mana value paid from the top of the library (Bolas's Citadel)
- [ ] (t15l) Cards to try with the existing nodes: The One Ring, Malakir Rebirth // Malakir Mire, Arcane Denial (a draw of up to two cards)
- [ ] (dk27a) Bids for a target or against one player (Illicit Auction, Mages' Contest: `BidLife` with targets and `OtherBidder$`); the payer's choice between a Food and three graveyard cards when a mana ability forages (the planner sacrifices a Food while there is one)
- [ ] (t20b-a) Replacements the checker cannot read because the proposals are wrong, so new proposals first: Sulfuric Vortex, Loading Zone
- [ ] (t20b-d) A writer for the replacement of the mana a permanent produces as it is tapped (`GameRule::ManaProduced`, Nyxbloom Ancient)
- [ ] (t20d) Triggered shapes whose stored rules are right but lack a primitive: Lunar Convocation, Bloodchief Ascension
- [ ] (t20e) The other `Triggered:` rows of the gap ledger (effects If, May, Move, CreateToken, RepeatEach, BecomeCopy, DealDamage, Untap; about 40 cards): most stored rules of "the combination or its shape" are the model's mistakes (Enduring Tenacity) and need a better proposal or a repair
- [ ] (t20c-a) A card named like the source as an amount ("for each card named ~ in each graveyard") and the mana sentence "Add {R}{R}, then add {R} for each ...": Rite of Flame
- [ ] (t20c-b) A writer for "Its controller can't cast spells this turn" after a counter (Render Silent)
- [ ] (t19c-a) Collective Resistance (Escalate and "choose one or more") reads back once the cost for each mode chosen of fe1ka exists: the Spree and Escalate readers and the header writers
- [ ] (t19c-b) "Choose a creature type" while a spell resolves (Kindred Dominance, Crippling Fear, Distant Melody, Harmonized Crescendo): readers and writers for the choice the resolving spell keeps (`ChooseValue` as `chosen`, `ChosenAs`)
- [ ] (t9a-c) The attack triggers that backup gives on Bright-Palm, Soul Awakener (backup itself plays)
- [ ] (t9d) The bodies of the combat-only spells (Cauldron Dance, Mandate of Peace, Spinal Embrace), and `CastOnly` read by a cast an effect makes
- [ ] (t9e) "Discard up to N cards, then draw that many cards": a discard of up to N whose count the draw reads (Daretti, Scrap Savant, Jaya Ballard, Cathartic Pyre)
- [ ] (t9f) Imprint artifacts: "you may exile an artifact card from your hand" as it enters and "{X}, {T}: Create a token that's a copy of the exiled card. X is the mana value of that card." (Prototype Portal, Soul Foundry)
- [ ] (t9g) Council's dilemma spells (Fateful Tempest, Selvala's Stampede, Travel Through Caradhras) and the Will of the Planeswalkers sentence beside other unread spells (Path of the Pyromancer, Path of the Schemer)
- [ ] (t9h) Chosen and counted statics the report names: "+N/+N for each Aura and Equipment attached to it" (Strong Back)
- [ ] (t12b1) "If you do" after a step that is no "may", sacrifice or counter removal needs the engine to report it as performed: Astral Drift (exile target, also the cycle event), Next of Kin, Flickerform (class C)
- [ ] (t12b2) The reflexive follow-up after reveal-until and look-at-top steps, once those read (t12a): Aspiring Champion (and "if that creature is a Demon"), Fireflux Squad, Cream of the Crop, Barrowgoyf (from among the milled cards)
- [ ] (t12b3) Amounts a follow-up names: Bebop (X counters of any kind on ~, the X of the second sentence), Neheb and Loamcrafter Faun ("that many" of the discard), Grishnákh (the amassed Army's power)
- [ ] (t12b4) Costs and choices before the link: Kitt Kanto (tap two untapped creatures, "that player"), Myr Battlesphere (tap X Myr), Canoptek Wraith (pay and sacrifice, choose a land), Crosis (choose a color, discard all of it)
- [ ] (t12b5) Follow-ups with a new shape: Forum Filibuster (return an Aura attached to the token), Human Torch (a delayed damage copy), Lavabrink Floodgates, Insatiable Frugivore (repeat), N'Yami-Class Mother Ship ("if you don't"), Kozilek's Return
- [ ] (t12a1) Takes from among the top cards, the cards left unread: a conjunction inside a list of kinds (Acclaimed Contender), one take per kind (Harper Recruiter), put "tapped and attacking" (Arthur), a delayed trigger on what was put (Aethermage's Touch), proliferate by the shortfall (Expand the Sphere), one card per keyword (Selective Adaptation)
- [ ] (t12a2) Choose a creature type or a card type as a spell resolves and take the cards of that type from the top (For the Ancestors, Winding Way)
- [ ] (t12a3) Reveal until a card matches, the follow-ups left unread: all matches to one place and the rest to the bottom or shuffled (Mass Polymorph), another player reveals (Proteus Staff, Collision of Realms), shares a type (Reality Scramble), Oath of Druids, Abundance
- [ ] (t13a) The one-off noun-phrase readers (the chosen type in `statics.rs` and `spend.rs`, cost subjects, `Targeting` in `cost_conditions.rs`) call the shared reader, output byte for byte the same
- [ ] (t13f) Sentence readers around the atoms that exist: free casts of a spell with mana value up to an amount, a count before a zone ("lands you control from your graveyard"), commander phrases (color identity, "a commander you own"), and adjectives in trigger subjects
- [ ] (t13g) Atoms that still lack an engine fact: a type or mode a spell chooses as it resolves, "was activated this turn"
- [ ] (t12c-a) The reanimation sentences of Grimoire of the Dead and Necromantic Selection ("it's a black Zombie in addition to its other colors and types")
- [ ] (t12c-b) A token replacement that keeps the original tokens ("those tokens plus that many 1/1 Squirrel tokens are created instead"): Donatello, Stridehangar Automaton
- [ ] (t12c-c) Athreos: "dies or is put into exile" and a "return that card" ref that finds a card in exile as well as in a graveyard
- [ ] (t12c-e) "Unless" forms that each need one more piece: a source's controller losing life unless they sacrifice (Archfiend of Spite), the tax on abilities (Tithe Taker)
- [ ] (t12c-f) In a "becomes the target" trigger `ControllerOf(Triggering)` names the controller of the targeted object as well as of the spell or ability: Bonecrusher Giant deals its 2 damage to both players; the cause names the targeting controller alone for it
- [ ] (t13b) A general amount language: `Count(Selector)` over every zone, a generic this-turn tally in place of the separate `TurnTally` variants, amounts from the triggering or chosen object, one "the number of ..." reader (about 150 tail cards)
- [ ] (t13c) A generic lasting-effect object with typed statics and triggers, bound objects and an end condition ("for as long as", "until your next turn"), the old duration nodes kept (about 200 tail cards, touches layers and lifetimes)
- [ ] (t13d) Optional parameters on the big node families instead of new nodes: a library position and several origin zones on `Move`, division among targets on `PutCounters`, an `in` zone on `Continuous`, copy exceptions, "different names" (3 to 15 cards each)
- [ ] (t13e) A cost item that wraps an effect (`CostItem::Do`) and one rule node for "X can't happen" (can't be cast, can't win or lose, triggers don't fire, untap in another player's untap step); about 65 tail cards
- [ ] (r4d-e) A count of transformed permanents (Mutagen Connoisseur)
- [ ] (r4d-f) Alternative costs past a "you control" condition: "If the {1}{B} cost was paid" (Baleful Mastery), the other Traps
- [ ] (r4d-g) A blocker of several attackers: its controller divides its damage and "Whenever ~ blocks" triggers once
- [ ] (r4d-i) Writers left for valid r4c proposals: "Enchant red or green creature" in the keyword reader (Controlled Instincts), "creatures that are still lands" (Living Lands); Elite Inquisitor and Oversoul of Dusk need new proposals
- [ ] (r4b-e) Writers left for valid r4a proposals: "the difference" (Balance of Power), "chooses three permanents, then sacrifices the rest" (Razia's Purification)
- [ ] (r4e-a) Writers left for valid r4e proposals: protection from three colors or subtypes in the keyword reader, "twice X" (Sanguine Sacrament), a counter on "it" after a return (Miraculous Recovery), a chosen creature put on top of its library (Nulltread Gargantuan)
- [ ] (r4b-c) Engine and DSL pieces the accepted-Partial r4a proposals need: +N/+N for each Aura attached (Auramancer's Guise), a card revealed at random from the hand (Goblin Secret Agent), each player gains control of what they own (Homeward Path), a life total that becomes N (Magister Sphinx), perpetual (Begin Anew)
- [ ] (r3w8b) Trigger writers left: a spell cast with mana value greater than a count (Mizzix of the Izmagnus), "if it isn't that player's turn" (Tataru Taru), a chapter whose target becomes an artifact (Phyrexian Scriptures)
- [ ] (r3w9a) Spell shapes left: "any number of target players each" (Priest of Forgotten Gods), costs per a count (Mizzix, Myth Unbound)
- [ ] (r3w2a-a) A writer for a group that becomes a creature "and gains flying" (Sarkhan the Masterless)
- [ ] (r3w2b) Writers for granted abilities in quotes whose trigger or effect is unwritten (The Reaver Cleaver, Unquenchable Fury), Clues that become Equipment (Armed with Proof) and an artifact type added without end (Phyrexian Scriptures)
- [ ] (r3w3b) Steps under "you may" left: a token copy with exceptions, "reveal that many cards", "the sacrificed permanent", "that player may ... if" and an exile of the card from the graveyard as a cost (Nightmare Shepherd, Glint Raker, Shadow, Ludevic, Kozilek's Return)
- [ ] (r3w1a) Conditional steps left: a card chosen at random (Deadbridge Chant), the top card revealed (Druidic Satchel), "remove all of them" with a named token (Replicating Ring), "choose target" before the condition (Meren of Clan Nel Toth)
- [ ] (r3w4b) Delayed triggers and repeated steps left: a loyalty ability's delayed return (Nahiri, the Harbinger), steps repeated for each object ("each Dragon you control", "each of those creatures"), a delayed trigger on its own line (Forth Eorlingas!)
- [ ] (r3w6a) The A to L mismatches left: "you may have it fight", "you and target opponent each create", "another target player", a token "that's all colors", one of two counter removals as a cost; Commodore Guff, Kazuul and Gyome need a rewrite
- [ ] (r3w5a) Writers for the rules of the r3 samples: a type and a granted ability during your turn (Gourmand's Talent)

## Layouts

- [ ] (c11) The initiative, dungeons and the remaining long-tail mechanics found by the report, one task each
- [ ] (c11c) The other 24 transforming Sagas, the two precon flip cards and the `prepare` layout's back face; frees about 37 cards (Budoka Gardener, Nezumi Graverobber)
