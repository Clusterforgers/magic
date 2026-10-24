# Reference engines

Survey of the open-source Magic engines Forge, XMage and Magarena: licence, card format, scale, and what in them can cross-check or feed the compiler's readings of Oracle text (`cards-dsl.md`, Compiler).

Measured on shallow clones of the default branches at the commits below; paths are repository-relative.

## Overview

| | Forge | XMage | Magarena |
|---|---|---|---|
| Repository | `github.com/Card-Forge/forge` | `github.com/magefree/mage` | `github.com/magarena/magarena` |
| Licence file | `LICENSE`: GNU General Public License, Version 3, 29 June 2007 | `LICENSE.txt`: MIT License, Copyright (c) 2010 betasteward@gmail.com | `LICENSE.txt`: GNU General Public License, Version 3, 29 June 2007 |
| Language | Java | Java | Java, Groovy for code scripts |
| Last commit | 2026-10-23 (`b13b6c82`) | 2026-10-22 (`c74ac92c`) | 2023-04-24 (`efa0aba8`), not archived |
| Card definitions | 34,100 files `forge-gui/res/cardsfolder/<letter>/<name>.txt`; 857 token scripts in `forge-gui/res/tokenscripts` | 32,546 classes `Mage.Sets/src/mage/cards/<letter>/<Name>.java` | 12,928 files `release/Magarena/scripts/<Name>.txt`, 143 token scripts, about 1,900 `.groovy` files; 5,293 unimplemented cards in `release/Magarena/scripts_missing` |
| Pool | `docs/Missing-Cards-in-Forge.md`: 32,479 unique cards, 46 missing for Vintage, Legacy and Commander (Stickers), 8 Alchemy missing for Historic | One class per oracle card; double-faced, split and adventure cards are one class | 12,928 of 18,221 known cards (71%) implemented |
| Level | Data: one text file per card, the effect verb is a name, the rest are parameters | Code: a Java constructor composes library classes; a card with special behaviour adds classes | Data: English rule sentences matched by regular expressions at load; code only where no pattern fits |

## Card definitions

| Engine | What is data, what is code | How a rule is parameterised |
|---|---|---|
| Forge | A line per part: `Name`, `ManaCost`, `Types`, `PT`, `K:` keywords, `A:` abilities, `T:` triggers, `S:` statics, `R:` replacements, `SVar:` named sub-parts, `Oracle:` text. Faces are joined in one file by an `ALTERNATE` line. All behaviour is in the engine: 205 effect APIs (`forge-game/src/main/java/forge/game/ability/ApiType.java`), 149 trigger types, 66 static-ability classes, 46 replacement classes, and keywords expanded into triggers and replacements in `forge-game/src/main/java/forge/game/card/CardFactoryUtil.java` | An ability is `Key$ Value` pairs separated by pipes; the first key names the API (`SP$` spell, `AB$` activated, `DB$` sub-ability chained by `SubAbility$`). Filters are a mini-language (`Creature.YouCtrl+powerLE2`); a number is a literal or an `X` defined by an `SVar` with a `Count$` expression; every ability line carries its clause text in a description key (`docs/Card-scripting-API/*.md`) |
| XMage | Java: abilities, costs, targets and effects are objects built in the constructor (`SimpleStaticAbility`, `EntersBattlefieldTriggeredAbility`, `GainAbilityControlledEffect`). The library is 796 `*Effect` classes under `Mage/src/main/java/mage/abilities`; 7,049 of 32,546 cards (22%) declare their own effect, trigger or replacement class beside the card | Constructor arguments: amounts, `Duration`, filter objects, a boolean per variant (one `ManifestEffect` also cloaks); each effect builds its rules text in a text field, and the card keeps the Oracle line as a comment |
| Magarena | A properties file: `name`, `cost`, `type`, `subtype`, `pt`, `ability=` lines (statics, triggers, activated, keywords), `effect=` (the spell, sentences joined by `~`), `timing`, `oracle`. 164 effect patterns (`src/magic/model/event/MagicRuleEventAction.java`), 270 ability patterns (`src/magic/model/MagicAbility.java`) and 107 trigger files (`src/magic/model/trigger`) read the sentences; 2,057 scripts name a `.groovy` file for the rest | The sentence is the parameter list: capture groups for amounts, players and filters (`ARG.*` in `MagicRuleEventAction.java`); the card name is `SN` |

## Frequency in Forge

| Measure | Value |
|---|---|
| Ability uses (`SP$`, `AB$`, `DB$`) in all scripts | 59,356 over 194 distinct APIs |
| Share of uses by the 10, 30 and 60 most used APIs | 58.6%, 84.6%, 94.4% (ChangeZone, Pump, Draw, Token, PutCounter, Cleanup, DealDamage, Mana, Effect, GainLife lead) |
| Trigger lines (`T:`) | 17,265 over 139 modes; the 10 most used modes cover 86% (ChangesZone 7,672, Phase 2,394, Attacks 1,620, SpellCast 1,422) |
| Scripts with an `Oracle:` line | 34,098 of 34,100 |

## Our blockers

| Wording | Forge | XMage | Magarena |
|---|---|---|---|
| Each player discards their hand, then draws N cards | `cardsfolder/w/wheel_of_fortune.txt`: `Discard` in hand mode over every player, then a `Draw` sub-ability for every player; data | `cards/w/WheelOfFortune.java`: `DiscardHandAllEffect` then `DrawCardAllEffect(7)` with its text overridden | `scripts/Wheel_of_Fortune.txt`: two sentences ("discards his or her hand", "draws seven cards") matched by the `DiscardHand` and draw patterns |
| "When you cast this spell" (storm copies) | `T:` mode `SpellCast` on `Card.Self` (117 scripts) with `CopySpellAbility` of the triggered spell; `Storm` is a keyword expanded in `CardFactoryUtil.java`; `fury_storm.txt` and `echo_storm.txt` count the commander casts with a `Count$` expression | `Mage/src/main/java/mage/abilities/effects/common/CastSourceTriggeredAbility.java` (89 cards), keywords `StormAbility` and `CommanderStormAbility` in `abilities/keyword`; `FuryStorm.java` | Not supported: no implemented script has the wording (8 in `scripts_missing`, `Fury_Storm.txt` text only); Storm is an `ability=` keyword (`Grapeshot.txt`) |
| Remove X storage counters from this land: add X mana in any combination of {R} and/or {G} | `f/fungal_reaches.txt`: `Mana` ability with a `SubCounter<X/STORAGE>` cost part, `Produced$ Combo R G`, `Amount$ X` | `FungalReaches.java`: `AddManaInAnyCombinationEffect` (`abilities/effects/mana`) with `RemoveVariableCountersSourceCost` | `scripts_missing/Fungal_Reaches.txt`, status `not supported: add-multiple-mana` (289 cards share it) |
| Copy exceptions "isn't legendary", "loses all other card types" | `CopyPermanent` key `NonLegendary$` (43 scripts); `Clone` keys `AddTypes$`, `RemoveCardTypes$`, `RemoveSubTypes$` (`i/imposter_mech.txt`); statics carry `RemoveCardTypes$` too (103 scripts, `imprisoned_in_the_moon.txt`); all data | `CreateTokenCopyTargetEffect` with an isn't-legendary setter (20 cards, `HelmOfTheHost.java`); the type removal is code in a per-card copy applier (`ImposterMech.java`) or a layer-4 effect class (`ImprisonedInTheMoon.java`) | Neither wording is implemented; `Helm_of_the_Host.txt` and `Imprisoned_in_the_Moon.txt` (status `needs groovy`) are in `scripts_missing`; 22 cards have status `becomes-copy` |
| "Becomes crewed" events | Trigger mode `BecomesCrewed` (`forge-game/src/main/java/forge/game/trigger/TriggerBecomesCrewed.java`; keys for first time each turn and the crew count) and mode `Crewed` for the crew side (`TriggerCrewedSaddled.java`); 12 scripts | Game event `CREWED_VEHICLE` used by the cards' own trigger classes (`MobilizerMech.java`, `MightyServantOfLeukO.java`); `CrewsVehicleSourceTriggeredAbility` for the crew side | No card has the wording; `MagicCrewActivation` is the Crew ability only |
| Umbra armor | Keyword `Umbra armor` (17 scripts) expanded in `CardFactoryUtil.java` into a destroy replacement that heals the damage and destroys the Aura | `UmbraArmorAbility` in `abilities/keyword` (17 cards) | The pool uses the old name Totem armor as an ability line (11 implemented, 4 missing) |
| Manifest and cloak | APIs `Manifest` and `ManifestDread` (70 scripts; `ManifestEffect.java`, `ManifestDreadEffect.java`) and `Cloak` (11 scripts; `CloakEffect.java`) | One `ManifestEffect` with a cloak flag, `ManifestDreadEffect`, `ManifestTargetPlayerEffect` (`abilities/effects/keyword`); 79 card files name manifest | Manifest: 8 implemented, 14 missing; the pool ends before cloak |

## Reading Oracle text

| Engine | Tooling | Coverage |
|---|---|---|
| Forge | No converter or parser. `forge-gui/tools/oracleScript.py` fills the `Oracle:` line of each script from Scryfall's oracle bulk file and renames wrongly named files; the other tools scrape, translate or convert decks | Every script is a hand-written pair of Oracle text and rules |
| XMage | `Utils/gen-card.pl` writes a class skeleton from `Utils/mtg-cards-data.txt` (66,076 printing lines, written by `Utils/mtg-cards-data-scryfall.py`): it recognises the 162 keywords of `Utils/keywords.txt` with their parameter kind (number, cost, type, colour) and leaves every other ability line as a comment. `Mage.Verify` (`VerifyCardDataTest.java`) compares each class with MTGJSON: types, colours, cost, power and toughness, loyalty, missing keyword and trigger phrases, and the full rules text for the sets named in `FULL_ABILITIES_CHECK_SET_CODES` | Keywords only; the rules text of a card is checked against Oracle, never derived from it |
| Magarena | The engine itself reads English: `effect=` and `ability=` sentences go through the pattern enums at load. `scripts/analyze_cards.scala` with `scripts/effects.txt` (268 known sentences, card name as a placeholder) marks a card scriptable when every rule line equals a known sentence after normalising, and prints a skeleton with name, type, cost and power. `Makefile` targets list the Oracle lines of unimplemented cards (`cards/unknown_oracle.txt`) | 10,871 of 12,928 implemented cards (84%) need no Groovy. 4,431 of 5,293 unimplemented cards carry one of 155 blocker statuses (`needs groovy` 905, `multiple-targets` 590, `add-multiple-mana` 289, `blocking-restriction` 205, `no-info` 159, `split-card` 143, `any-number-of` 117, `reveal-hand` 108) |

## The 20 cards

Forge column: script under `forge-gui/res/cardsfolder`; XMage: class under `Mage.Sets/src/mage/cards`; Magarena: file under `release/Magarena/scripts` (impl) or `scripts_missing`. "Devouring Monster" is an ability word of Haruspex.

| # | Card | Forge | XMage | Magarena | Meaning in a mappable form |
|---|---|---|---|---|---|
| 1 | Helm of the Host | Yes: `CopyPermanent` of the equipped creature, `NonLegendary$`, haste keyword | Yes: library trigger, own effect class (`HelmOfTheHost.java`) | Missing, text only | Forge: the verb and the exception are keys. XMage: in code |
| 2 | Living Laser | Yes: `CopyPermanent` with a count of cards discarded this turn, tapped and attacking, exile at end of turn | No class | Absent | Forge: keys for every part |
| 3 | Imprisoned in the Moon | Yes: static with colour, type, `RemoveCardTypes$`, `RemoveAllAbilities$` and a granted mana ability | Yes: own layer-4 effect class | Missing, `needs groovy` | Forge: keys. XMage: in code |
| 4 | Galvanic Iteration | Yes: `DelayedTrigger` on `SpellCast`, then `CopySpellAbility`; Flashback keyword | Yes: library `CopyNextSpellDelayedTriggeredAbility`, Flashback | Absent | Forge and XMage: named parts |
| 5 | Fury Storm | Yes: self `SpellCast` trigger, `CopySpellAbility` with a commander-cast count, and the spell's own `CopySpellAbility` of a targeted spell | Yes: `CommanderStormAbility` and `CopyTargetStackObjectEffect` | Missing, text only | Forge and XMage: named parts |
| 6 | Rhythm of the Wild | Yes: `Counter` replacement (cannot happen) and a static `AddKeyword` of Riot | Yes: library effects plus an own replacement class | Absent | Forge: keys. XMage: partly in code |
| 7 | Crystal Barricade | Yes: static hexproof for `You`; `DamageDone` replacement that prevents noncombat damage | Yes: library `GainAbilityControllerEffect`, `PreventAllNonCombatDamageToAllEffect` | Absent | Forge and XMage: named parts |
| 8 | Regal Sliver | Yes: static `AddTrigger` with monarch condition, `PumpAll`, `BecomeMonarch` | Yes: library effects only | Absent | Forge: keys (an `SVar` per sub-part). XMage: named parts |
| 9 | Prismatic Geoscope | Yes: enters-tapped replacement; `Mana` with `Combo Any` and `Count$Domain` | Yes: `DynamicManaAbility`, enters tapped | Missing, `add-multiple-mana` | Forge and XMage: named parts |
| 10 | Haruspex (Devouring Monster) | Yes: `Mana` with a `SubCounter<X/P1P1>` cost, any one colour | Yes: `DynamicManaAbility`, `RemoveVariableCountersSourceCost` | Absent | Forge and XMage: named parts |
| 11 | Terminus | Yes: `ChangeZoneAll` of creatures to library position -1, Miracle | Yes: own effect class, `MiracleAbility` | Implemented, effect in `Terminus.groovy` | Forge: keys. XMage and Magarena: in code |
| 12 | Banishing Stroke | Yes: `ChangeZone` to library position -1, Miracle | Yes: `PutOnLibraryTargetEffect`, `MiracleAbility` | Implemented, `effect=` sentence and Miracle line | All three |
| 13 | Hidden Strings | Yes: Cipher keyword, `TapOrUntap` twice with unique targets | Yes: `CipherEffect` and an own effect class | Missing, `multiple-targets` | Forge: keys. XMage: partly in code |
| 14 | Watcher for Tomorrow | Yes: Hideaway 4, enters tapped, leaves trigger `ChangeZone` of the exiled card | Yes: `HideawayAbility`, own effect class | Absent | Forge: keys. XMage: partly in code |
| 15 | Shelldock Isle | Yes: Hideaway 4, `Play` of the exiled card with a library-size condition | Yes: `HideawayAbility`, `HideawayPlayEffect`, `ConditionalOneShotEffect` | Missing, `hideaway` | Forge and XMage: named parts |
| 16 | Wiretapping | Yes: Hideaway 5, `Drawn` trigger for the first draw of the draw step, `Play` with a hand-size condition | Yes: `HideawayAbility`, own trigger class | Absent | Forge: keys. XMage: in code |
| 17 | Fealty to the Realm | Yes: `BecomeMonarch`, static `GainControl$` by the monarch, `CantAttack`, `MustAttack` | Yes: library effects plus an own continuous effect for the control | Absent | Forge: keys. XMage: partly in code |
| 18 | Queen Mother Ramonda | Yes: `BecomeMonarch`, static `CantAttack` with `Condition$ Monarch` and a power filter | Yes: library effects only | Absent | Forge and XMage: named parts |
| 19 | Tamiyo, Field Researcher | Yes: three loyalty abilities (an `Effect` that holds a floating trigger until your next turn, `Tap` with a hidden keyword, an emblem with a may-play static) | Yes: library effects, an own effect, an own delayed trigger and an emblem class | Missing, `multiple-targets` | Forge: keys. XMage: partly in code |
| 20 | Archon of Coronation | Yes: `BecomeMonarch`, `LifeReduced` replacement for damage while monarch | Yes: library effects plus an own replacement class | Absent | Forge: keys. XMage: partly in code |

| Engine | Cards found | Meaning stated in structured form |
|---|---|---|
| Forge | 20 of 20 | 20: every part is an API, a mode or a static or replacement key |
| XMage | 19 of 20 | 9 compose library classes only (4, 5, 7, 8, 9, 10, 12, 15, 18); 10 have their own effect class |
| Magarena | 9 of 20 (2 implemented, 7 in the missing list) | 1 (Banishing Stroke); Terminus has its effect in Groovy; the missing ones hold Oracle text only |

## Recommendation

| Use | Engine | Licence facts | Effort |
|---|---|---|---|
| Cross-check of our readings: a reader of Forge scripts (line split, API, keys, trigger mode), a map from the 60 most used APIs and the trigger and static modes to DSL verbs, and a per-card report of the verbs Forge names that our compiled card lacks | Forge; XMage as a second source for the verbs Forge states less directly | GPLv3 section 2: making, running and propagating a covered work without conveying it has no conditions. MIT grants use without conditions. The report keeps card names and verb counts, and the clones stay outside the repository | Medium: one reader task, one map task, one report task |
| Grouping the unread tail by Forge API and key instead of by wording, so the one-off wordings gather under the verbs and exceptions they share (for example `CopyPermanent` with `NonLegendary$`) | Forge | As above | Small after the reader exists |
| Examples in a model's prompt: pairs of a clause (the description key of a Forge line, the Oracle sentence) and its parts, picked by nearest wording for the reader pilot (`r2`) and written out as our DSL by the model | Forge for retrieval at run time; XMage for any example stored in the repository | GPLv3 section 5: a work based on the program that is conveyed is licensed as a whole under GPLv3 (section 0: conveying is a propagation that lets other parties make or receive copies; interaction over a network with no copy is not). MIT: the copyright and permission notice goes with copies or substantial portions | Small: the retrieval is a lookup over the reader's output |
| Bulk conversion of scripts into rules files and `card_rules` rows | Forge for the 60 most used APIs | Converted rules are copied or adapted from the script (GPLv3 section 0, "modify"); rules rows reach browsers (`cards-dsl.md`, D1), which is conveying; section 5 then applies to the converted work as a whole and section 6 to source. XMage under MIT needs the notice only | Large: `SVar` indirection, the `Count$` expression forms and the filter language each need a reader; every converted card still passes the structure checks and the oracle fingerprint |
| Blocker taxonomy | Magarena: the 155 status values and their counts as a second ranking of unsupported features | GPLv3, read only | Small, as part of the report; the pool ends in April 2023 |
| Behaviour checks | XMage: 1,007 single-card test files (`Mage.Tests/src/test/java/org/mage/test/cards/single`) among 2,150 test files, as a list of mechanics to scenario-test | MIT | Out of scope for the readings |

Order: the Forge reader and map serve the cross-check, the grouping and the examples; the prompt examples come second, and a conversion only for API families where the report shows the compiler has no reading.

## Cross-check report

`mtg-cli forge-check --forge <checkout> [--deck CODE_OR_NAME] [--top N] [--keys N] [--out FILE]` sets every stored precon card against the Forge script of its name (`crates/mtg-cli/src/forge`). The scripts are read at run time from the checkout; the repository holds card names, Forge's API, mode and key names, and counts.

| Part | Rule |
|---|---|
| Reader (`script.rs`) | A line is `Tag:body`; `A:`, `T:`, `S:`, `R:`, `K:` and `SVar:` lines are read, faces split at `ALTERNATE`. A body splits at pipes into `Key$ Value` pairs. The verb is the value of `SP$`, `AB$` or `DB$` (effect), `Mode$` (trigger, static) or `Event$` (replacement), the text before the first colon of a keyword, and each part of a `Cost$` of an ability (a spell's mana is skipped). A name in a value (`SubAbility$`, `Execute$`, `Choices$`, `Triggers$`) is followed once to its `SVar`; an ability, trigger, static or replacement part that no line names is read too; `Count$` expressions and hints for the computer player are not items. Keys that only carry text, targeting, references or remembered objects are left out, and the many keys of a condition or an "unless" payment are one key each. A keyword the DSL has whole (`Backup`, `Gift`) holds its ability: the parts it names give no item. The effect a replacement names carries the key `Replacing` with the replaced event, and a cost part on the source by its nickname (`Sac<1/NICKNAME>`) is the part on the source (`SacMe`) |
| Map (`map.txt`, `map.rs`) | A Forge item, with optional conditions on keys, against the verbs of ours that stand for it (any one is enough), named as `used_primitives` names them (`Effect.Move`, `Trigger.StepBegins`). The most specific rule that holds wins. `-` is an item with no DSL verb, `~` bookkeeping. `also` lines name a key that asks for a verb of its own (`CopyPermanent` with `NonLegendary` asks for `Mod.RemoveSupertype`; an optional trigger, `OptionalDecider`, is read by a `May`, a `Pay` or a cast, and an "unless" payment by a `Pay` or a `May`); conditions after the key make the line more specific and the lines with the most conditions that hold decide (`AffectedDefined=Self`: an ability a static grants to its own card is the ability of the card with a condition, as level and station brackets state it); `variant` lines name a key whose value tells uses apart (`Destination=Exile`). A keyword with no rule stands for the keyword of its name. The map has 153 effect, 79 trigger, 46 static, 22 replacement, 11 keyword and 28 cost rules, 63 key rules and 10 variants; it names the 60 most used APIs and every item of the cards whose clauses are all read, and a test checks that every verb of the map is a variant of the DSL |
| Comparison (`compare.rs`) | The verbs of a card's rules are the primitives of its modelled abilities (the hand-written file, else the compiler's reading). An item is read when one of its verbs is among them, missing when none is, `no verb` or `unmapped`. The comparison is by sets for the whole card, not by clause |
| Report (`report.rs`) | An unread card has a clause left to the table or unusable rules. The tail groups the unread cards by Forge API (a trigger, static, replacement, keyword by its family and name), with the cards that carry each key and the cards no other group holds; a key that 90% of a group's cards carry is a parameter of the verb and is left out; costs are read with their ability. The cover is the share of the grouped cards whose groups all lie among the top groups |

Run on the 6,429 distinct cards of the 198 stored precons and 34,100 scripts (commit `b13b6c82`):

| Measure | Value |
|---|---|
| Cards with a Forge script | 6,427 of 6,429 (no script: Arvinox, the Mind Flail; Baldin, Century Herdmaster) |
| State of those cards | 2,234 verified, 2,425 partial, 1,768 manual; rules: 616 hand-written, 5,802 compiled, 9 none |
| Items their scripts name | 26,834: 13,320 read (49.6%), 12,662 missing a mapped verb (47.2%), 752 with no DSL verb (2.8%), 100 not in the map (0.4%) |
| Verbs Forge names that the rules lack | 327 verbs on 4,120 cards. Most cards: `Trigger.ZoneChange` for `ChangesZone` 848, `Effect.CreateToken` for `Token` 591, `Effect.Move` or `Search` for `ChangeZone` 584, `Effect.Draw` 558, a static for `Continuous` 502, `Effect.PutCounters` for `PutCounter` 443, `Trigger.StepBegins` for `Phase` 430, `Effect.Continuous` for `Pump` 330 |
| The same in cards whose clauses are all read | 1 of the 2,336 (0 of the 670 hand-written): an optional trigger that the Forge script states with "you may" and the Oracle text states without (Timeless Witness: CR 603.5 makes a trigger optional only when its text says "may", so the mandatory trigger is the right reading). The map names a verb for every item of these cards. The optional triggers read in rules are `Pay` ("you may pay {N}. If you do": 13 cards) and a cast (Smuggler's Buggy), both asked of the controller on resolution (`optional_triggers.rs`); level and station brackets state a granted ability as an ability with a condition |
| Unread tail | 4,191 cards with a clause left to the table; 4,132 of them have Forge items we do not read, in 366 groups. 2,436 cards read some abilities (2,377 grouped, 291 groups); 1,755 read nothing (276 groups) |
| Largest groups of the tail (cards; cards with no other group) | `ChangeZone` 867 (107), trigger `ChangesZone` 851 (7), `Token` 590 (39), `Draw` 552 (26), static `Continuous` 524 (160), `PutCounter` 473 (19), trigger `Phase` 430 (4), `Pump` 329 (29), `Effect` 328 (0), trigger `Attacks` 306 (1) |
| Keys of a group (`CopyPermanent`, 113 cards) | `AtEOT` 27, `AddTypes` 20, `PumpKeywords` 20, `SetPower` and `SetToughness` 17, `TokenTapped` 17, `NumCopies` 16, `TokenAttacking` 13, `NonLegendary` 7, `Populate` 7 |
| No DSL verb | `RepeatEach` 111 cards, `DigUntil` 66, `RollDice` 27, `AddPhase` 23, `FlipCoin` 23, `MultiplyCounter` 23, `Vote` 22 |

Share of the grouped tail that one reader for each of the top groups covers:

| Top groups | 1 | 3 | 5 | 10 | 20 | 50 | 100 |
|---|---|---|---|---|---|---|---|
| All 4,132 grouped cards | 2.6% | 6.8% | 12.9% | 21.2% | 36.8% | 64.5% | 83.2% |
| The 2,377 that read some abilities | 0.3% | 10.5% | 15.7% | 24.7% | 42.1% | 67.5% | 84.8% |
| The 1,755 that read nothing | 3.1% | 5.4% | 8.6% | 16.0% | 31.9% | 61.7% | 82.7% |

The tail is long: ten readers free a fifth of the cards and fifty two thirds, because a card holds on average several groups. The groups that free cards alone (static `Continuous` 160, `ChangeZone` 107, `Token` 39, `ReduceCost` 38, `Mana` 34, `Pump` 29) are the pieces of `ROADMAP.md`, Model-assisted reading.
