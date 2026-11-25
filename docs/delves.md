# Delves

Design of Delves, the co-op campaign: a party of 1 to 6 heroes fights a ladder of bosses with their own Commander decks.

## Shape

- A run is a boss rush. Its ladder (`mtg-campaign`, `map.rs`) holds three of the theme's lieutenants drawn from the seed (fewer when the theme has fewer), then one of the theme's final bosses drawn by the seed (the Warrens: Grimgut or the Unmaker; the Drowned Spire: the Leviathan or the Drowned Admiral; the Ember Foundry: Kaldrek or Ignivar); a stored run of a generation before 4 folds by its older ladder of every lieutenant, then the final boss; between two bosses the party votes for one of two camps: Treasure, Shop, Event or Campfire.
- Themes: the Warrens, the Drowned Spire (`content/spire.json`, Bosses) and, from generation 11 (`FOUNDRY_FROM`) and Heat 3 (`FOUNDRY_HEAT`), the Ember Foundry (`content/foundry.json`, The Ember Foundry). A run of generation 8 or 9 climbs the Spire's first ladder: the Siren Queen and the Drowned Archivist, then the Leviathan. A new run draws its theme by its seed among the themes whose final boss its generation meets and whose least Heat (`Theme.min_heat`) its Heat reaches (`Content::theme_at`); from generation 9 each descent leads into another of those themes (Endless).
- Every boss fight is a fresh, normally recorded Commander game: board, hand and library reset. The party is one team of seats from seat 0, the boss one seat after them (`FightPlan`).
- Each side takes one shared team turn (`TeamMode::SharedTurns`, CR 805): every hero untaps, draws, plays a land and casts sorceries in the party's turn, a side's turn is led by its first seat (`turn_lead`: the first living hero, the boss) and priority goes round the table from it. In the declare-attackers step each player of the side chooses the attacks of their own creatures in turn order, seeing what the teammates before chose; the side declares every choice at once, so limits, requirements and "attacks alone" read the whole declaration (`party_attacks`), and each hero pays the attack costs of their own creatures. A choice illegal only together is refused with its reason and chosen again; after two refusals the engine declares the plain legal attack. Life totals stay per hero. The table names the party's turn ("Your party's turn").
- Life carries over between fights. After a won fight each hero keeps their end life and heals 15; a hero who ended it as a Spirit comes back at 10 first. A Campfire rest heals 20, the shop's draught 15; nothing heals above 40 plus relic bonuses.
- The party has 3 hearts. A lost fight costs one and the fight is retried: against the first boss every hero regroups at full life, against a later one at half life; with no hearts left the run ends in defeat.
- From content generation 5 a run is endless: when a floor's final boss falls the party votes to bank the win or descend to the next floor (Endless).
- A fight counts as lost, at the same price, when its table closes or expires without a result (the host closed the lobby, or the janitor dropped an unstarted lobby or a game nobody sat at; a bot ally is nobody) and when the host retreats, which ends the table. A game an admin ended or that failed to load was never played: the fight is made again at no cost.
- Timers: the host picks Relaxed (default), Quick or Off when starting a run (`CreateDelve.timers`, stored as the run's first settings event, `Run.timers`); every fight's game plays under it, so the clocks of [server.md](server.md) (Response clocks) are the same in each fight, and Off never passes for a hero by time. A run stored without the setting is Relaxed. The boss is a bot and has no clock.
- Within a fight a hero at 0 life is a Restless Spirit while a teammate lives (`docs/engine.md`, Campaign seats); the party loses when every hero has fallen. See Spirits.

## Spirits

A fallen hero keeps playing: it haunts the fight with small, clear choices and can come back.

- A Spirit's cards leave the game; it keeps its turns and its library. Opponents cannot attack it or target it.
- Ectoplasm is its resource: 1 on falling and 1 more at each of its upkeeps. The Restless Spirit emblem (`content/cards.json`) offers haunt actions at instant speed, each paying its ectoplasm as a cost (`SpendCounters`) as it goes on the stack, so an action is a legal play only while the Spirit holds its price:

| Action | Ectoplasm | Effect |
|---|---|---|
| Wail | 1 | Target opponent loses 2 life; its creatures get -1/-0 until end of turn |
| Ward | 1 | Prevent the next 3 damage to each teammate this turn |
| Whisper | 1 | Each teammate scries 2 |
| Hinder | 1 | Tap a creature an opponent controls; it does not untap during its next untap step |
| Howl | 3 | Tap up to two creatures opponents control; each opponent loses 2 life |
| Rise Again | 5 | As a sorcery: return to life with 10 life and draw five cards |

- Ways back: Rise Again; the Phoenix Feather relic (pay 6 life once each fight: every fallen teammate returns with 8 life and four cards); a won fight brings every Spirit back at 10.
- A Spirit is asked at priority only where its haunts matter (`docs/server.md`, Windows): the party's upkeep and main phases with an empty stack (Rise Again, spending ectoplasm), the boss side's beginning of combat (Hinder, Howl) and its declare attackers step when it attacks the party (Ward), and whenever an object of the boss side is on the stack. Spells and abilities of its teammates never ask it and every other step is passed for; a stop of its own asks anywhere, and Full Control asks at every window. The Spirit panel says so.
- Bot Spirits (`brain/haunt.rs`) rise as soon as they can, Ward when 4 or more power attacks the party, and before an opponent's combat Hinder its biggest creature (power 4 or more, 2 ectoplasm held) or Howl (4 held). A bot holding the Phoenix Feather uses it while it has 14 life or more.
- Measured with two Hard heroes against the boss at Heat 1, rank 2 (`fights`): the fallen hero spends its Spirit turns on 3 haunt actions a fight and never reaches Rise Again, so haunting helps at the margin and does not decide a fight.

## Bosses

Every boss (`boss_plan.rs`) is: one innate, one encounter modifier, two phases by round, a satchel, a precon deck and relics by depth. The boss has nothing else; its brain is Hard at every Heat and depth.

- Record (`content/<theme>.json`): `deck_file` names its stored Commander precon (`<code>-<name>.json` in `content/decks`, with `-foundry` where the Foundry swaps a precon the Spire also plays; the face commander shuffled in as an ordinary card), `innate` its innate card, `round_phases` its Phase 1 and Phase 2 cards, `satchel` its four satchel spells (`reusable`, or once per fight; `cue`, when its bot casts it, `docs/bots.md`), `art` the real card whose art crop pictures it. The deck file holds the precon's cards with its by-hand cards swapped for basics or automatic cards (in the Foundry of the same type within its colours; `swaps`) and its `makeup`, counted from the corpus by the `boss_decks` test (`MTG_UPDATE_MAKEUPS=1` rewrites it). Every card name of a deck file is the full name of the card it plays, never only one face of another card (`boss_decks`). The server loads the deck as a live game loads a deck: each Read or Unverified card plays its draft, pinned in the recipe (`cards_read`); the satchel cards come last in the deck (`FightPlan::boss_deck`) and the plan moves them into the seat's satchel.
- Depth: the bosses the party defeated in the run over all floors (`Run::defeated`); a lost and retried fight adds none.
- Innate: written once with a number N over five levels (`"levels": [n1, …, n5]`, `{N}` in its text, `"$N"` and a cost's `{N}` in its abilities, `"$L"` for the level where the shape changes); loading makes the card of each level (`<id>_i` to `<id>_v`, `innate.rs`) and refuses levels that fall. A level may differ in more than N: `"params": {"NAME": [v1, …, v5]}` puts any value where `"$NAME"` stands (an ability that becomes `null` is left out at that level), and `"texts"` gives each level its own words. A phase may be written with levels too; the fight plays it at the innate's level (`boss_plan.rs`). The level is `1 + (depth points + Heat) / 2`, at most V (`Scale::innate_level`); depth points are the depth for the first four bosses, then one more per two bosses.

| Depth | 0 | 1 | 2 | 3 | 4–5 | 6–7 | 8–9 | 10–11 | 12+ |
|---|---|---|---|---|---|---|---|---|---|
| Heat 0 Casual | I | I | II | II | III | III | IV | IV | V |
| Heat 1 Seasoned | I | II | II | III | III | IV | IV | V | V |
| Heat 2 Heroic | II | II | III | III | IV | IV | V | V | V |
| Heat 3 Legendary | II | III | III | IV | IV | V | V | V | V |

- Encounter modifier: one game-warping rule for the whole fight, drawn from the pool both themes share (`content/modifiers.json`, its cards in `content/encounters.json`) by the seed keyed by floor and room, so a retried fight keeps it. A modifier never meets a boss its `not_with` names (`modifiers.rs`).
- Phases: Phase 1 begins at the beginning of the end step of the last turn of round 8, Phase 2 of round 14 (`PHASE_ROUNDS`, `PhaseStart::AtRound`). A round is one turn of each seat or team (`Turn::round`); an extra turn does not advance it. A phase is an emblem for the rest of the fight; its "when this phase begins" ability (`TriggerPattern::PhaseBegins`) uses the stack once.
- Satchel: four spells in the boss's command zone, cast like cards in hand and public; a reusable one returns as it leaves the stack and costs {2} more for each earlier cast this fight, a once card does not return.
- Relics: `(depth + Heat + 1) / 2` relics and never fewer than `depth − offset`, the offset 4, 3, 2 and 1 at Heat 0 to 3, at most 6 (`boss_relics.rs`); a run of a generation before 12 (`BOSS_SCALE_FROM`) counts only `depth − offset`. From generation 15 (`THEME_RELICS_FROM`) a theme may cap its bosses' relics (`boss_relics_most`): the Warrens at three, which binds from depth 4 at Heat 3 and depth 6 at Heat 1; the Reinforcements fade (below) follows the uncapped count.

| Depth | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10+ |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Heat 0 | 0 | 1 | 1 | 2 | 2 | 3 | 3 | 4 | 4 | 5 | 6 |
| Heat 1 | 1 | 1 | 2 | 2 | 3 | 3 | 4 | 4 | 5 | 6 | 6 |
| Heat 2 | 1 | 2 | 2 | 3 | 3 | 4 | 4 | 5 | 6 | 6 | 6 |
| Heat 3 | 2 | 2 | 3 | 3 | 4 | 4 | 5 | 6 | 6 | 6 | 6 |

- They come from the heroes' whole relic pool of the run's generation, risky relics included, never one that names teammates (`team`), and only those the deck's makeup can use. Each is Rare by chance (10, 15, 20 or 25 % at Heat 0 to 3 with one relic, 10 points more for each further relic, at most 60 %) or, from four relics at Heat 1 and above, Mythic (5 % at four, 10 at five, 15 at six); the rest are Common. Copies can repeat and stack. The draw is keyed by floor and room.
- Life: 40 and 15 for each hero after the first (`Scale::boss_life`); Heat and depth do not change it.
- Reinforcements: from generation 12 the boss brings one Reinforcements emblem for each hero after the first (`Scale::reinforcements`, `content/cards.json`): it draws an additional card in its draw step and may play an additional land each turn, so it plays about one more deck's cards for each deck the party adds. A solo hero meets none. From generation 13 (`THEME_SUPPLIES_FROM`) a theme may add Supplies (an additional card in its draw step, without the land) to every boss of it against two or more heroes (`boss_supplies`, `Scale::theme_reinforcements`): the Ember Foundry one. From generation 14 (`REINFORCEMENTS_FADE_FROM`) a boss that brings six relics (`FADE_RELICS`, the most) brings no Reinforcements; the theme's Supplies stay.
- Its advantages are open: the innate, modifier, relics and reinforcements are emblems in its command zone from the start, named with their rules on the seat's sheet (`boss_sheet.rs`, `BossSeat::sheet`). The intro (`DelveFightView`) lists the two effects, the phases with their rounds ("Phase 1 at round 8"), the satchel with each card's mark and the relics; at the table the bar shows the innate with its level, the modifier and relics (reinforcements among them) as chips, and a pill counting down ("Phase 1 in 3 rounds") that names the phase in force once it begins, with the next one's countdown. A reusable satchel card reads "returns · +{N}" with its tax now, a once card "once" (`BossView`: `innate`, `modifier`, `round_phases`, `phase_in`, `relics`, `satchel`).
- Opening hand: the boss draws 7 with 2 to 4 lands and takes no mulligan (`OpeningLands::BOSS`).
- No loops: the boss's bot answer any loop offer with at most 5 iterations or Stop and never a draw (`loop_answer_in`, `BOSS_LOOP_CAP`).
- Drain rule: a boss's innate, modifier, phase or satchel card takes life from a hero only as damage or unless the hero pays.
- Relics for heroes: from generation 8 (`RELICS_AGAIN_FROM`) a relic can drop again; owning it never lowers its chance, and copies stack.

### The Drowned Spire

Sixteen lieutenants, one for each colour and colour pair (two blue-black), and two final bosses. Each innate scales its number N with the level.

| Boss | Colours | Precon | Innate | Phase 1; Phase 2 |
|---|---|---|---|---|
| The Lamplighter | W | C14 Forged in Stone | beginning of combat: taps up to N hero creatures | taps every hero creature, +1/+1 and vigilance; hero creatures -2/-0, first strike |
| The Tidecaller | U | C14 Peer Through Time | end step: returns a hero permanent of mana value N or less | returns hero permanents of mana value 3 or less; hero spells cost {1} more, it draws an extra card |
| The Drowned Abbess | B | C14 Sworn to Darkness | upkeep: the eyed hero sacrifices a creature unless they pay {N} | hero creatures -2/-2; a hero creature dying deals 1 damage to its controller |
| The Wreck-Forger | R | C14 Built From Scratch | upkeep: a Treasure; its artifacts dying deal N damage to a hero creature | destroys hero artifacts; +2/+0 and haste |
| The Coral Mother | G | C14 Guided by Nature | end step: a +1/+1 counter on each of up to N creatures | a 4/4 reach Coral per hero; creatures with counters +1/+1, trample and hexproof |
| The Siren Queen | UB | ZNC Sneak Attack | takes an attacker of mana value N or less | takes each hero's strongest creature; heroes must attack into her |
| The Drowned Archivist | UB | MKC Revenant Recon | casts a hero's instant or sorcery of mana value N or less | heroes mill ten, it casts their creatures; copies what it casts from them |
| The Fog Bell | WU | KHC Phantom Premonition | attacking hero creatures get -N/-0 | two flying Spirits per hero; blockable only by flying or reach |
| The Salt Magistrate | WB | C21 Silverquill Statement | a hero's noncreature spell makes an Inkling unless they pay {N} | each hero sacrifices two creatures; flyers +1/+1 and lifelink |
| The Storm Herald | WR | MKC Blame Game | beginning of combat: N damage to a hero creature | 2 damage to each hero and their creatures; +1/+0, flying and haste |
| The Kelp Shepherd | WG | MIC Coven Counters | end step: N 1/3 Crabs | two +1/+1 counters on each of its creatures; +1/+1 and trample |
| The Maelstrom | UR | C21 Prismari Performance | its instants and sorceries return a hero creature of mana value N or less | 3 damage to each hero creature; its instants and sorceries deal 1 to each hero |
| The Spawning Shoal | UG | C21 Quantum Quandrix | upkeep: N 1/1 Fish | a Fish per creature token it has; Fish +2/+1 and trample |
| The Brine Hag | BR | AFC Planar Portal | upkeep: N damage to the eyed hero | 2 damage to each creature; a hero creature dying makes a Treasure, menace |
| The Silt Crawler | BG | DSC Death Toll | upkeep: each hero mills N | a creature from each hero's graveyard joins it; +1/+1 and deathtouch |
| The Magma Vent | RG | NEC Upgrades Unleashed | landfall: N damage to a hero creature | 3 damage to each hero creature and 1 to each hero; landfall makes a 3/3 haste Elemental |
| The Leviathan (final) | UG | CMR Reap the Tides | upkeep: Tentacles up to N, each can stop an attacker | returns all but sea creatures, lands are Islands; +2/+2 and islandwalk |
| The Drowned Admiral (final) | UBR | LCC Ahoy Mateys | upkeep: N 2/2 menace Zombie Pirates; a Pirate's combat damage makes a Treasure | its Pirates return from its graveyard, +1/+0; Pirates +1/+1 and haste, end step 2 damage to each hero |
### The Ember Foundry

A burning foundry under a mountain, drawn only at Heat 3 (`min_heat`): fifteen lieutenants, one of every colour and colour pair, then Kaldrek or Ignivar. Its art is the commanders, artificers and dragons of its precons; its events trade at forges, troughs and anvils (the Slag Heap, the Bellows Shrine, the Quench Trough, the Master Smith's Anvil, the Runaway Ore Cart), and its reward pool leans on mana rocks, artifacts and equipment. Cards: `content/rework_foundry.json`.

| Boss | Colours | Precon | Innate | Phases |
|---|---|---|---|---|
| Brannoc, the Anvil Warden | W | Forged in Stone | equipped creatures +N/+N and first strike | Soldiers, vigilance; +1/+1, equipped double strike |
| Ottoline the Clockmaker | U | Peer Through Time | heroes' spells cost {N} more | heroes' creatures tapped through an untap; a card each upkeep, spells {2} less |
| Morvane, the Slag Lich | B | Necron Dynasties | an artifact creature of value N or less returns each end step | every artifact creature returns; +1/+1 deathtouch |
| Grizzek the Scrapmaster | R | Built From Scratch | a Servo each upkeep; an artifact dying deals N to each hero | 3 to the heroes' creatures; +2/+0 menace |
| Veyla of the Rusted Grove | G | Guided by Nature | destroys a hero artifact of value N or less each upkeep | a counter each, trample; +2/+2 |
| Ilsabet, the Ghost in the Gears | WU | Phantom Premonition | flyers +N/+N | small hero creatures bounce; flying, a Spirit per hero each upkeep |
| Haldric, the Ashen Stoker | UB | Undead Unleashed | a hero creature card of value N or less rises for him each upkeep | heroes mill ten; heroes' creatures −1/−1 |
| Vashka, the Ember Warlock | BR | Planar Portal | N damage to each hero each end step | 2 to heroes and their creatures; 1 to each hero per spell |
| Kassa Bladebreaker | RG | Upgrades Unleashed | N counters on her biggest creature each upkeep | a counter each, modified trample; modified +2/+0 haste |
| Mother Ironbark | GW | Coven Counters | a counter on her smallest creature each upkeep; countered creatures +N/+0 vigilance | two counters each; countered trample and hexproof |
| Overseer Calvaine | WB | Silverquill Statement | each hero pays {N} or he gets a flying Inkling | each hero sacrifices a creature; +1/+1 lifelink |
| Ivra, the Brass Inventor | UR | Exquisite Invention | N Thopters per artifact spell | two Thopters per hero, artifacts {1} less; artifact creatures +1/+1 haste |
| Winnow, the Ash Gardener | BG | Death Toll | mills N, a creature card back to hand each upkeep | everyone mills five, a creature returns; +2/+1 trample |
| Dame Teodora, the Reforger | RW | Lorehold Legacies | N Servos when an artifact creature dies | a 4/4 Golem per hero; +1/+1 first strike |
| Quillon the Mold-Maker | GU | Jump Scare! | an Ooze with N counters each upkeep | counters doubled, trample; +1/+1 hexproof |
| Kaldrek the Ironmaster (final) | UBR | Mishra's Burnished Banner | N damage to a hero creature per artifact entering | takes each hero's greatest artifact, artifacts {1} less; 4 to the heroes' creatures, artifact creatures +2/+2 |
| Ignivar, the Forge-Wyrm (final) | RG | Draconic Rage | N to each hero when his creature is dealt damage | 3 to every creature and each hero; a 4/4 Dragon each upkeep |

### Encounter modifiers

Each modifier is an emblem on the boss's seat whose rule binds every player alike unless it names the heroes; most copy the rule a Verified card runs. The Spire bosses go by their ids in `not_with` (`siren_queen`, `fog_bell`, …).

| Modifier | Rule | Never with |
|---|---|---|
| Mirror Hall | No one controls more than five creatures of one name; the newest beyond five are sacrificed. | Skrix, Grimgut, Vex, Hollowmere, Sahrra, the Leviathan, the Fog Bell, the Kelp Shepherd, the Spawning Shoal, the Drowned Admiral |
| Living Stone | Artifacts have "{T}: Add {C}." | |
| Waking Earth | Lands are 1/1 creatures too (Kormus Bell). | Vex, Seraphine, Korrath, Ozrith, Ysolde, Hollowmere, Valdric, Vorthana, the Leviathan, the Lamplighter, the Drowned Abbess, the Salt Magistrate, the Storm Herald, the Maelstrom, the Brine Hag, the Magma Vent |
| Bloodlust | Creatures have double strike and attack each combat (Avatar of Slaughter). | the Siren Queen, Aurenne |
| Twin Flames | Damage is doubled (Furnace of Rath). | |
| Overflowing Mana | A land tapped for mana adds one more of a type it made. | |
| Deep Pockets | An extra card in each draw step and no maximum hand size (Howling Mine, Anvil of Bogardan). | |
| Quickening | Spells may be cast as though they had flash (Vedalken Orrery). | |
| One Voice | One spell per player each turn (Rule of Law). | Ozrith, Morwenna, the Drowned Archivist, the Maelstrom |
| Narrow Pass | At most two attackers and two blockers each combat (Caverns of Despair). | Skrix, Grimgut, Aurenne, Hollowmere, Sahrra, the Kelp Shepherd, the Spawning Shoal, the Drowned Admiral |
| Second Spring | Tokens and +1/+1 counters on creatures are doubled (Primal Vigor). | Sivanne, Sahrra, Skrix, the Coral Mother, the Kelp Shepherd, the Spawning Shoal |
| No Rest | Each card or token going to a graveyard flips a coin; on a loss it is exiled. | Vex, Vorthana, Hollowmere, the Siren Queen, the Drowned Archivist, the Leviathan, the Drowned Abbess, the Silt Crawler, the Drowned Admiral |
| Stone Sleep | One flip for the active player each untap step; on a loss their creatures with power 3 or more stay tapped. | Mossjaw, Korrath, the Unmaker, the Leviathan |
| Prismatic Chaos | Each round the colored symbols of the heroes' costs stand for other colors of their commander's identity. | |
| Endless Summer | One additional land each turn (Ghirapur Orrery). | |
| Open Book | Libraries are played with the top card revealed, and lands and spells may be played from the top (Future Sight). | |
| Thin Veil | Creatures entering cause no triggers (Torpor Orb). | |
| Shared Blood | Creatures are every creature type. | Vex |
| Giant's Table | Creature spells cost {2} less. | |
| Free Command | Heroes pay no commander tax (`GameRule::NoCommanderTax`). | |
| Echoing Spells | Each player's first instant or sorcery each turn is copied (Double Vision). | Ozrith, the Maelstrom |
| No Mercy | Each life gain flips a coin; on a loss no life is gained. | Seraphine, Valdric, Mossjaw |
| Haunted Ground | A nontoken creature dying gives its controller a 1/1 white flying Spirit. | Hollowmere |

## Spotlight

The boss's side spreads its pressure over the party (`mtg-engine`, `spotlight.rs`; bots in `mtg-testkit`, `brain/spotlight.rs`).

- Attention: damage a boss-side source deals a hero, life a hero loses to a boss-side spell or ability, and each permanent of a hero that one removes (3, a token 1) count as pressure on that hero. The window is the last three rounds of the table, or one round per hero for a bigger party.
- Cap: a hero who took more than 40 % of the window's pressure (60 % with two heroes standing, from 6 pressure on) is capped. The boss's side neither attacks a capped hero nor aims removal or player effects at them while another hero is a fair target, unless the capped hero is an overwhelming threat: a presence at least twice every other hero's (and 15 or more), or creatures whose power reaches the boss's life.
- Focus: the boss eyes the uncapped hero with the most presence (creature power, 2 per other nonland permanent, cards in hand) less attention, ties going round the table with the boss's turns. The table shows the eyed hero on each boss-side seat ("Eyes Ann").
- Binding: each boss-side seat's attacks and its targeted spells and abilities go at the hero it eyes (`spotlight_aim.rs`). A seat leaves that hero only when (a) the eyed hero can't be attacked or targeted (no legal way, protection, hexproof), (b) another hero is an overwhelming threat or their creatures' power reaches the boss's life while the eyed hero is neither, (c) every attacker that could go at the eyed hero would be blocked and killed with no damage through (a blocker that kills it and lives, or strikes first; flyers need a flyer or reach, menace two blockers) while an attack at another hero is not futile, or (d) the attack kills another hero (not in the grace; damage past each blocker, the biggest attackers blocked first): a kill is always taken. Blocks are free. Outside these cases nothing moves a seat off its eyed hero; inside them the aim follows the order the boss eyes the heroes. Within the aimed hero's board the bots weigh a target by its worth less attention (1 per point) plus 6 for the eyed hero, never by low life alone.
- The log names the reason: "Skrix attacks Chrisser instead of Mattias: Mattias can't be attacked" (`Event::FocusShifted`: the seat, the eyed hero, the hero attacked instead, and one of Unreachable, Overwhelming, Deadly, Futile, Lethal). Targeted spells and abilities follow the same rule without a log line.
- Grace: until every standing hero has finished three turns, damage and life loss from the boss's side leave a hero at 1 life at least. A hero's own cards are not held back.
- Politics-free: the boss's side reads only the board, the spotlight and the life totals; no deals, no kingmaking.

## Ramp-up

- The boss bots hold back sweepers (wipes, and damage or sacrifice to each player or creature without a target) through the boss's first three turns.

## Commander rules in a fight

- Heroes play their Commander decks under the Commander rules: 40 life at the start of a run (carried after), the commander tax, 21 combat damage from one commander eliminates a hero.
- The boss has no commander. They take commander damage as normal damage but never lose to 21 of it, so a big commander shortens the fight without ending it in one hit.

## Difficulty

The host picks the Heat when creating a run; the creation form and the run header word it as a tier:

| Heat | Tier | In plain words |
|---|---|---|
| 0 | Casual | any deck can win with some luck |
| 1 | Seasoned | needs a deck that suits the format |
| 2 | Heroic | needs a team of decks that work together and suit the format |
| 3 | Legendary | needs a tuned team, and opens the Ember Foundry |

Depth and Heat act on the boss through its innate level and relics (Bosses); the party size sets its life and reinforcements.

## Endless

From content generation 5 (`ENDLESS_FROM`) a run continues below its first floor (`Run.floor`, from 1).

- Floor cleared: when a floor's final boss falls the run waits on the cleared floor (`Status::FloorCleared`, `Run.floors_cleared` counts it). Each hero votes to bank the win or descend (`RunEvent::FloorVoted`, `DelveAction::FloorVote`); with every vote in, the most votes win and a tie goes to the torch bearer's vote, as the camp vote. Bank ends the run in victory.
- Descent: the next floor's ladder is drawn from the seed keyed by the floor (`map::generate_floor`): three lieutenants and a final boss as on the first floor from the roster of the run's generation, avoiding the bosses of the floor above while enough others are left. The heroes keep their life, relics, Satchel, upgrades, banes and gold; the party gets one heart back, up to 3 (`DESCENT_HEARTS`). The floor's first boss awaits at once.
- Descent loot: from content generation 7 (`DESCENT_LOOT_FROM`) a vote to descend first gives every hero the final boss's loot as after a lieutenant (Loot: 50 gold, a relic pick and a card pick, drawn for the floor), the party still in the cleared floor's last room (`Status::Room`, `Run.descending`); the descent follows once every hero answered. Bank takes no loot. A run of generation 5 or 6 descends at once, so its log replays unchanged.
- Themed floors: from content generation 9 (`THEMES_FROM`) each descent leads into another theme: the floor below draws its theme from the seed keyed by that floor among the themes open to the run's generation and Heat other than the current floor's (`Content::next_theme`; the Foundry only from Heat 3), so no theme plays two floors in a row; with one open theme the floor keeps it. The floor carries its theme (`Run.floor_theme`, the floor above's in `Run.theme_from` when it changed; both absent on the first floor and in older runs, whose stored events replay unchanged): its ladder, events, card pool and relic filter are that theme's, a Satchel counting each card by the first pool that names it. The chronicle and the descent's summary name the new theme.
- Bots: bot allies vote once every player voted; they descend while the party holds 2 hearts or more and 60 % of its life cap or more (`Run::bots_descend`), else bank.
- Only the final boss's fall asks the vote. A run of an earlier generation ends in victory at its final boss as before.

## Loot

- After each lieutenant, and from generation 7 after a final boss the party descends from (Endless), every hero gets 50 gold (`LOOT_GOLD`), a pick of one of three relics and a pick of one of three cards in their colours (skipping a card gives 10 gold).
- The Satchel: every card a hero earns (loot, shop, an event, the Mirror Pool) goes to their Satchel (`Hero.satchel`), never into the deck, so the deck's land count never shifts. Each fight starts with the whole Satchel in the hero's command zone (`PlayerSetup::satchel`, [engine.md](engine.md), Campaign seats); the hero casts its cards there like cards in hand, and a card cast stays out of it for the rest of that fight. The server lays it over the chosen deck (`ChosenDeck::with_satchel`), and the recipe keeps it. A run stored before the Satchel reads its earned cards as Satchel cards, and its decks play whole.
- Relics (`content/cards.json`) are emblems the hero keeps for the run, each with a role that shapes the party: Healer, Removal, Damage, Support, and Risky relics whose effect is symmetrical and helps the boss too (all red spells cost {2} less, every player draws an extra card, all creatures get +2/+0). Risky relics come only from loot and treasure, never the shop.
- The relic pool holds 70 relics: 20 of generation 0, 33 of generation 2 and 17 of generation 3 (`CustomCard::since`; a run of an older generation draws only the relics of its own). Each is written with abilities the engine runs (static, triggered, replacement, cost-modifier and rule abilities), and none takes life from the party: a relic's life loss hits only its holder's opponents, and a risky relic names no side:

| Role | Relics | What they do |
|---|---|---|
| Healer | 11 | life at an upkeep or when a spell, creature, artifact, enchantment, land or token comes, more life at the start of a fight, a trickle from a full graveyard; life gained drains the opponents |
| Removal | 9 | damage, tapping or -1/-1 on the opponents' creatures when the hero casts a colour, artifact or instant or sorcery, creatures that enter tapped, -1/-0 or -1/-1 for the opponents' creatures, a {1} tax on their spells |
| Damage | 15 | +1/+0, +1/+1 or keywords for creatures, tokens, green creatures and a creature commander, counters at the end step, doubled tokens or counters, cascade for big creatures, life loss for the opponents with a red spell, a land or a dying creature |
| Support | 29 | {1} or a coloured pip less for a colour, {1} less for an artifact, enchantment, instant or sorcery or commander spells, scry, a land drop, mana, cards, haste, more counters, cascade from instants or sorceries, a creature cast from the graveyard |
| Risky | 6 | symmetrical: red spells {2} less, every spell {1} less, an extra draw, creatures +2/+0, +1/+1 or menace |

- Rarity (`CustomCard::rarity`): every relic is Common (35), Rare (24) or Mythic (11); stronger effects sit higher, as tiers of one effect show (Ruby Focus, Common: red spells cost {1} less; Ruby Prism, Rare: {R} less; Ember Idol +1/+0, Ember Crown +1/+1). The risky relics are Rare. Mythic relics define a run: the Maelstrom Crown (an instant cascades, and the cards passed stay in exile instead of going to the bottom), cascade for sorceries or for creatures of mana value 5 or more, doubled creature tokens or +1/+1 counters, life gained drains the opponents, an extra draw, a creature cast from the graveyard each turn, +2/+2 double strike and trample for a creature commander, +1/+1 vigilance and trample for every creature, -1/-1 for the opponents' creatures.
- From generation 3 rarity decides where a relic comes from: each slot of a loot pick is Rare by chance (`RARE_ODDS`: 15, 25, 35 and 45 % after the first to the fourth boss), else Common; a Treasure adds 20 to that chance; the Shop sells one Common (55 gold) and one Rare (90 gold), never a risky relic; a Mythic comes only from an event's trade (Events) and the first slot of the loot after the last lieutenant. A slot whose rarity has nothing usable left takes the tier below (`relics::draw`). An older run keeps its own relic prices.
- Usefulness: from generation 2 a relic offer (loot, Treasure, the Shop, an event's relic) holds only relics the hero's deck can use, risky ones under the same rule. A relic lists what its rules care about (`CustomCard::wants`: spells of a colour, cards of a type or subtype, token makers, +1/+1 counters, the graveyard, a creature commander); a relic that wants nothing (life, cards, mana, the opponents' creatures and spells) fits every deck, and 21 of them do. A Mythic relic follows the same rule: the Maelstrom Crown needs 8 instants.
- The deck's makeup (`HeroDeck.makeup`, `relics.rs`) counts its commanders and library by colour, type, subtype and those marks when the deck is chosen, as the commander facts are; the Satchel adds the pool cards it holds (colours from `Theme.pool`, the type line from `Theme.pool_types`; a Mirror Pool copy of a deck card counts nothing more). A relic is usable when the deck holds at least 8 cards of everything it wants together (an instant and a sorcery count alike for a relic about both), or a creature commander for a relic that wants one.
- The pick is drawn from the seed as before, from the usable relics the hero lacks: the generic ones are always among them, so a pick fills from them; with fewer than three usable it is shorter and never padded with relics the deck cannot use, and with none usable it falls back to every relic left, so it is never empty while one is left. Bots take the same offers. A deck chosen before its makeup was stored, and a run of an older generation, draw from every relic they could before.
- Camps: Treasure offers a relic pick; the Shop sells three cards (35 gold), two relics, a draught and a commander upgrade (50 gold); an Event offers each hero a trade with its cost and reward up front (Events); a Campfire offers rest, a card pick or Temper, a pick of one of three commander upgrades.
- Commander upgrades (`upgrade.rs`): a cost {1} or {2} lower (generic mana only, never below its coloured pips), a keyword (flying, trample, vigilance, lifelink, haste, menace, first strike, deathtouch, hexproof), +1/+1 or +2/+2, entering with an additional +1/+1 counter, or with an additional loyalty counter. `fitting` offers only what does something for that commander: P/T, counters and keywords for a creature, loyalty for a planeswalker, a reduction while generic mana is left, no keyword it already has, no combat keyword for one with no power; each commander of a pair on its own, each upgrade once. The deck's commander facts (`HeroDeck.commanders`: types, power, generic mana, keywords) are read when it is chosen; a deck chosen before upgrades gets none. Each upgrade is an emblem in the hero's command zone at every fight (a static, cost or replacement ability on "your commander named X"), so the card itself never changes.

## Events

- An event (`content/<theme>.json`) is a story and options; each hero answers for themselves and only their own gold, life, relics, cards and commander change. An option is a list of outcomes. Rewards: `Gold`, `Life`, `RelicOf` (a pick of three relics of that rarity), `CardPick` (a pick of one of three cards), `Duplicate` (the Mirror Pool: a copy of a nonland card of the deck or Satchel joins the Satchel; the server lists the cards in `DelveView.mirror` and refuses any other). Costs: negative `Gold` and `Life`, `MaxLife` (the life cap moves for the rest of the run, `Hero.life_cap`; a cut lowers life to the new cap), `LoseCard` (the hero names a card of the deck or Satchel, `Offer::Discard`: a Satchel card leaves it, else one deck copy leaves for the run, `Hero.lost`, which the server takes out of every fight's deck, `ChosenDeck::without`; a card the deck no longer holds, by the library names of its `DeckMakeup`, is refused), `LoseRelic` (the hero gives up a relic of their choice, `Offer::Surrender`) and `Bane` (the first commander costs {N} more or gets −N/−N for the run: an emblem in the command zone at every fight, like an upgrade). Neither follow-up can be skipped.
- Events are known trade-offs from generation 3: no outcome is random. Each event of generation 2 has a trade version that replaces it (`since` 3; the older one has `until` 2), and two more join: the Bleeding Altar and the Forge of Kings. A reward grows with its cost: a Common relic costs a little life or a card, a Rare one 12 to 15 life, a 5 cut to the cap or a relic, a Mythic one a 10 cut to the cap, or a 5 cut with a bane or a relic. Every event keeps a free option with a small reward. Older runs keep their events, gambles (`Gamble`, odds and the outcomes of a win and a loss) and one-relic picks (`Relic`).
- The words come from the data, never from the label: `Outcome::words`, `EventOption::summary` ("−8 life, +40 gold"), and its split into `cost` and `reward` ("−10 max life" and "a Mythic relic"; "nothing" for a free option). A content test checks that every option has a summary and that no label states an amount.
- A sure cost is a need (`EventOption::needs`): that much gold, more than that life (an answer never takes a hero below 1), a life cap of at least 20 after a cut (`MIN_LIFE_CAP`), a relic to give up, a commander (a creature commander for −N/−N) to weaken. The server refuses an answer whose need is unmet (`Run::accepts`, `Hero::lacks`); a stored log replays as before. Every event keeps an option with no need.
- Bots: in an older run they take the first option they can afford; from generation 3 they weigh each option they can take (a Common relic 8, a Rare 15, a Mythic 28, a card 6, gold by fifths; life lost and cap cuts weigh double when hurt or the cap is low, a relic given up 12, a bane 8 to 10) and take the best, never naming a deck card they cannot see (a card loss with an empty Satchel). They give up their least rare relic and their newest Satchel card.
- Each answer is recorded as a `Resolved` in `Run.results` (what the hero did, the outcomes as gold and life really moved, the gambles rolled and how they fell) and as a chronicle line. A camp's results stay through the next boss fight and clear when that fight is won or the next camp opens.

## Run state

- A run is the fold of its events (`run.rs`): `Content`, `Joined`, `Left`, `DeckChosen`, `Timers`, `Started`, `Voted`, `FightCreated`, `FightEnded`, `Picked`, `FloorVoted`, `Abandoned`. The ladder is regenerated from the seed, never stored.
- Decks are chosen once, in the lobby; afterwards only rewards change them. A fight is made from those decks (`FightPlan.run` names the run, so the game links back); its game lobby refuses a deck change (409 `deck_fixed`). A hero whose deck no longer plays (deleted, nothing playable) holds the fight: the run view lists it in `deck_problems`, and only that hero may then choose another deck (`DeckChosen` while the fight waits for its game).
- The party's list leaves abandoned runs out; the rows stay stored. A run unchanged for 7 days (`STALE_DAYS`) with no live fight is abandoned when the list is read. A run that ends (the host ends it, a stale run is abandoned) ends its fight's table like a retreat, so no seat of it stays listed as anybody's lobby or game; once the stored games load, the server ends every fight whose run no longer waits for it (a run closed by a migration).
- Every random draw comes from the seed keyed by room, hero and purpose (`rng.rs`), so a reload never rerolls loot.
- Content generations: a new run stores the generation it is made with (`RunEvent::Content`, `CONTENT`); an event joins at a generation (`since`) and only runs of that generation or later draw it, so a stored run replays the same events. Generation 2 adds the larger relic pool (a relic's `since`) and the relic filter (Loot); generation 3 (`RARITY_FROM`) adds rarity-driven offers and prices, the Rare and Mythic relics of its own, and the trade events, while an event an older generation met names its last one (`until`). Wares appended to the shop go after the draught for the same reason.
- Votes: only a choice of two camps is voted on; a camp's single way on, the next boss, is entered once every hero answered the camp (stored votes for it fold as nothing). When every hero voted, the room with the most votes wins and a tie goes to the torch bearer, a role that moves one hero on with every cleared room. Bot allies vote once the players did, with the majority, and answer offers with the first choice (rest when hurt, the first affordable relic at the shop, the newest Satchel card at the Mirror Pool).

## Interface

- The run page reads as a descent: a header with the party's life, hearts, Heat, floor and seed; the ladder of the floor as a path from the first boss to the final one, every room cleared, current, next or locked, with its floor; votes show who chose which camp.
- A cleared floor shows the choice to bank the win or descend, each hero's ballot and what a descent changes (`DescendPanel`), and the loot before a descent names the floor it leads to; the run summary counts the floors cleared and the fight log names each fight's boss and, below the first floor, its floor.
- Each camp is a scene of its own (loot, shop, campfire with Temper, event, the Mirror Pool, a card or relic to give up); a risky relic is always marked Dangerous, and every relic chip and card shows its rarity by colour and word. The run header lists each hero's Satchel, commander upgrades and banes, and a cut life cap. An event shows each option's cost and reward before the choice (relic rewards in their rarity's colour, odds in an older run, why an option is shut) and a "What happened" panel with the chronicle after it (Events). Implementation in [web.md](web.md), Delves.
- The run page names the theme of the floor the party is on and its story (`DelveView.theme`, `story`) and takes its look: the page sets `data-delve-theme`, and a Drowned Spire floor lays its sea-green ember and stone (`--delve-spire`, `--delve-spire-stone`) and an Ember Foundry floor its molten ember and iron (`--delve-foundry`, `--delve-foundry-stone`) over the Warrens' ones. The ladder names the floor's theme beside its number. On a floor that a descent led into another theme, a notice under the header says "The party descends into the Drowned Spire" until the floor's first room is cleared (`DescentNotice`).
- A fight opens with the boss intro (the run view's `fight`: the boss, its life, innate at its level, modifier, relics, phases with their rounds and satchel), in place of the game's lobby: the boss's abilities in full, the party's decks and carried life, and Ready to fight; the host also has Retreat, confirmed first. When the game ends the final table stays under the fight's result (`FightEnd`: won or lost, each seat's end, the log's last turns) until the player presses "Continue to the run", or after a 60 s countdown that "Stay" stops; the run then greets the loot or the lost heart.
- The run keeps each fight whose game ended or lost its table (`Run.fights`: the game, the room, won or lost; a voided game and a retreat before a game leave none). The run page lists them newest first as the Fight log, each linking to its recorded game (`/replay/$gameId`), where the whole log of the fight reads back.
- At the table the boss sits on top and the party together at the bottom (boss mode); the boss bar shows the innate, modifier and relics as chips with their rules, the phase countdown and the satchel (Bosses). Their words come from the boss's sheet the engine plays (`BossView`).

## Leaderboard

- `GET /api/delves/leaderboard` ([server.md](server.md), Delves) folds every stored run past its lobby, abandoned ones included, into one board per Heat the run was started at (Legendary, Heroic, Seasoned, Casual), the greatest first.
- Depth (`Run::depth`): the floors whose final boss fell, then the bosses beaten on the floor below them (none while the party stands on a cleared floor). A deeper run ranks first by floors, then by bosses; a tie goes to the run that last changed earlier, then by name.
- Each signed-in player stands once per board with their deepest run: their name in it, its depth, their commanders (the deck name for a deck chosen before commanders were read), the party size with bot allies and the day the run last changed. A run with no boss beaten and a bot ally never stand on it; a board lists at most 20 (`BOARD_LIMIT`).

## Content

- Custom cards are not on Scryfall: id, name, type line, rules text, art placeholder key, role and price, and their abilities in the card DSL. The engine plays each as an emblem; the web draws them from the content endpoint. The kinds are relics, the Spirit's haunt, innates, phases, encounter modifiers and the boss's reinforcements.
- Files: `cards.json` (relics, the haunt), `encounters.json` and `modifiers.json` (encounter modifiers), `rework_*.json` (innates and phases), `<theme>.json` (bosses, events, reward pool), `decks/` (the bosses' precons).
- Content tests check that every satchel and reward card plays automatically (Verified or Read) in the corpus, every boss names a 100-card deck file, an innate with five levels, two phases and four satchel cards, every custom card keeps all its abilities, and each boss plays a real fight at level V under the loop detector.
- Content generations of bosses: a boss's `since` keeps it off the ladder of an older run (Ozrith, Aurenne, Vorthana and Sivanne 6, the Spire 8, its grown roster 10, the Foundry 11, its Supplies 13, Reinforcements fading at six relics 14, the Warrens' relics capped at three 15); every run plays the boss of Bosses whatever its generation; `RunEvent::Content` stores the generation and the map is drawn again for it.

## Tuning

- `cargo run --release -p mtg-campaign --example delve_sim -- fights <games> <heroes> <level> [first seed] [threads] [heat] [depth] [modifier]` plays each boss in turn against a fresh party of bot heroes on the four fixture Commander decks; `runs` plays whole runs with life carried and loot taken. Games are seeded and played by paced bots ([bots.md](bots.md), Approach), so each result replays and a fight takes what its bots would think at a live table (about a minute for six rounds); `DELVE_SIM_UNPACED` searches the levels' whole untimed count, `DELVE_SIM_TIMED` plays on the clock and `DELVE_SIM_PROFILE` adds a line per seat and decision kind with its seconds, tree searches and rollouts. The depth (0 by default) is the bosses defeated before the fight; the modifier is `rolled` (the run's draw, the default), `none` or a modifier's id. The bosses' bots read the content's Satchel cues. `DELVE_SIM_DROP` (modifier, relics, reinforcements or phases by id), `DELVE_SIM_ADD` (reinforcements by id), `DELVE_SIM_RELICS` (at most that many relics), `DELVE_SIM_LIFE` (percent), `DELVE_SIM_HAND` and `DELVE_SIM_LEVEL` (the innate and levelled phases at that level, relics kept) change the plan to measure what each part is worth; `DELVE_SIM_EVENTS` adds per fight the plan, the phases begun, the combats and each seat's lands, spells and attackers; `DELVE_SIM_IDLE_SPIRITS` makes every Spirit pass. `DELVE_SIM_CONTENT` sets the content generation, `DELVE_SIM_THEME` the theme (the Warrens by default) and `DELVE_SIM_BOSSES` (ids) plays only those bosses, a boss off the seed's ladder taking the place of the boss of its rank.
- The output is one `key=value` line per boss (`boss=<id>`, then `boss=all` and a spotlight line), and a `missing` line names the deck cards the corpus lacks, played as Wastes: wins, the median round the fights ended in, the share that reached Phase 1 (`p1`) and Phase 2 (`p2`), the share that ended in each round (`ended=<round>:<share>`), turns and minutes, estimated at 0.6 per hero turn and 0.5 per boss turn. `DELVE_SIM_SEEDS` adds a line per fight with the pressure each hero took, the round a hero first fell to half life and the round a hero first fell.
- Targets: the median fight ends in rounds 9 to 13, so most fights see Phase 1 and about a third see Phase 2; a party of Hard heroes wins each fight from the second boss in 50 to 60 % at Heat 1 and a whole run in 20 to 35 %; no hero takes more than 40 % of the boss's pressure with three or more heroes (60 % with two).
- Measured with Hard heroes at Heat 1, depth 1 (`fights 30 <heroes> hard 1 4 1 1`, about two fights a boss; an older generation by `DELVE_SIM_CONTENT`, generation 11's two-hero row with 60 fights from seed 10). Each cell reads win rate, median end round, Phase 1, Phase 2 and the most pressed hero's share; generation 13 changes only the Foundry against two or more heroes, and generations 14 and 15 change no cell here (a boss at depth 1 brings one relic; the Warrens at generation 15 measured again, alike):

| Theme | Heroes | Generation 11 | Generation 12 | Generation 13 |
|---|---|---|---|---|
| Drowned Spire | 1 | 60 %, 9, 53 %, 7 % | 60 %, 9, 57 %, 3 % | as 12 |
| Drowned Spire | 2 | 95 %, 8, 40 %, 8 %, 68 % | 63 %, 9, 62 %, 28 %, 64 % | as 12 |
| Drowned Spire | 3 | 100 %, 8, 37 %, 3 %, 59 % | 50 %, 10 (28 fights, wins and rounds only) | as 12 |
| Warrens | 1 | 57 %, 7, 37 %, 0 % | 47 %, 7, 33 %, 0 % | as 12 |
| Warrens | 2 | 87 %, 7, 25 %, 0 %, 67 % | 52 %, 8, 34 %, 10 %, 62 % (29 fights) | as 12 |
| Warrens | 3 | 93 %, 7, 10 %, 7 %, 63 % | 43 %, 8 (28 fights, wins and rounds only) | as 12 |
| Ember Foundry | 1 | 57 %, 8, 41 %, 0 % | 67 %, 8, 43 %, 7 % | as 12 |
| Ember Foundry | 2 | 98 %, 7, 33 %, 3 %, 70 % | 73 %, 9, 58 %, 7 %, 60 % (60 fights) | 50 %, 10, 70 %, 3 %, 63 % (seeds 31 to 60, 77 % at 12) |
| Ember Foundry | 3 | not measured | 69 %, 8 (29 fights, wins and rounds only) | 53 %, 10, 80 %, 10 %, 51 % |

- The reinforcement brings two heroes from 87 to 98 % down to 52 to 73 % and three heroes to 43 to 69 %, with longer fights that reach Phase 1 in most and Phase 2 in up to a quarter; the one early relic leaves a solo hero within the noise of 30 fights (47 to 67 %). One Heat higher (two heroes on the Spire at Heat 2, depth 1, two relics) the party wins 53 %, round 9, Phase 1 67 %, Phase 2 7 %, 60 %, so the curve keeps rising without overshooting. A fight of a long tail (rounds 20 and more) takes over half an hour at a loaded machine, so a three-hero cell of 30 fights takes over two and a half hours on four threads.
- The Foundry's knob against two heroes: one more relic for its bosses leaves them at 70 % (30 fights); one more Reinforcements overshoots to 23 % (13 fights; a solo hero 23 % in 30); Supplies, its draw without the land, lands at 50 %.
- Deep: two Hard heroes at Heat 3, depth 7 (level V, six relics; `fights 20 2 hard 1 3 3 7`), each cell as above. Generation 14 drops the Reinforcements there, the Foundry keeping its Supplies; generation 15 caps the Warrens' relics at three, so its bosses bring no Mythic there either:

| Theme | Generation 11 | Generation 13 | Generation 14 | Generation 15 |
|---|---|---|---|---|
| Drowned Spire | 54 %, 8 | 15 %, 7, 35 %, 0 %, 61 % | 45 %, 8, 45 %, 10 %, 62 % | as 14 |
| Warrens | 20 %, 7, 40 %, 0 % | 25 %, 6, 15 %, 0 %, 61 % | 20 %, 7, 40 %, 0 %, 62 % | 50 %, 8, 45 %, 10 %, 67 % |
| Ember Foundry | not measured | 5 %, 8, 45 %, 0 %, 57 % | 40 %, 9, 60 %, 5 %, 56 % | as 14 |

- The deep Warrens at generation 14 is hard by its relics: without them the party wins 80 %, without the cost relics (Spendthrift's Purse, the prisms and foci, Crystal Geode, Spellbinder's Ring) 50 %, without the board relics (the anthems, Battle Frenzy, Blight Censer, Frostbound Anchor, Banner of Multitudes and the like) 45 %, with the first three 55 %; generation 11 (no Reinforcements), Supplies in place of the Reinforcements and four relics win 20 %, 20 % and 15 %, the innate at level III 35 %. Its creature decks cast their hand by round 2 to 4 and a hero falls to half life in round 2 to 4 in most losses (Skrix, Mossjaw, Valdric, Aurenne), where the Spire's and the Foundry's come in rounds 4 to 8. Seeds 21 to 40 win 70 % at generation 14 and 65 % at 15, so all 40 fights win 45 % and 58 %; three of the 40 generation-14 wins are a boss's Maelstrom Crown exiling its library in round 2.

- What each part is worth at generation 11 (the Spire unless named, 24 to 60 fights): 150 % life ends in round 9 and reaches Phase 1 in 59 % and Phase 2 in 8 %, still won 95 %; the Foundry at 200 % life ends in round 8 and is won every time; a hand of 10 with 150 % life wins 97 %; the innate and phases at level V win 92 %, round 10, Phase 1 58 %, Phase 2 17 %. Without reinforcements the boss's side rarely attacks: it declares 0 or 1 attackers to the party's 15 to 25, because two heroes' blockers outclass its creatures.
