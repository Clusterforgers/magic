# Delves

Design of Delves, the co-op campaign: a party of 1 to 6 heroes fights a ladder of bosses with their own Commander decks.

## Shape

- A run is a boss rush. Its ladder (`mtg-campaign`, `map.rs`) holds three of the theme's lieutenants drawn from the seed, then a final boss drawn from the theme's final bosses and the upgraded forms of the lieutenants left off the ladder (a run of a content generation before 4 meets every lieutenant of its generation, then the theme's final boss); between two bosses the party votes for one of two camps: Treasure, Shop, Event or Campfire.
- Every boss fight is a fresh, normally recorded Commander game: board, hand and library reset. The party is one team of seats from seat 0, the boss one seat after them and its minions after it (`FightPlan`).
- Each side takes one shared team turn (`TeamMode::SharedTurns`, CR 805): every hero untaps, draws, plays a land and casts sorceries in the party's turn, a side's turn is led by its first seat (`turn_lead`: the first living hero, the boss) and priority goes round the table from it. In the declare-attackers step each player of the side chooses the attacks of their own creatures in turn order, seeing what the teammates before chose; the side declares every choice at once, so limits, requirements, costs and "attacks alone" read the whole declaration (`party_attacks`). A choice illegal only together is refused with its reason and chosen again; after two refusals the engine declares the plain legal attack. Life totals stay per hero. The table names the party's turn ("Your party's turn").
- Life carries over between fights. After a won fight each hero keeps their end life and heals 15; a hero who ended it as a Spirit comes back at 10 first. A Campfire rest heals 20, the shop's draught 15; nothing heals above 40 plus relic bonuses.
- The party has 3 hearts. A lost fight costs one and the fight is retried: against the first boss every hero regroups at full life, against a later one at half life; with no hearts left the run ends in defeat.
- A fight counts as lost, at the same price, when its table closes or expires without a result (the host closed the lobby, or the janitor dropped an unstarted lobby or a game nobody sat at; a bot ally is nobody) and when the host retreats, which ends the table. A game an admin ended or that failed to load was never played: the fight is made again at no cost.
- Timers: the host picks Relaxed (default), Quick or Off when starting a run (`CreateDelve.timers`, stored as the run's first settings event, `Run.timers`); every fight's game plays under it, so the clocks of [server.md](server.md) (Response clocks) are the same in each fight, and Off never passes for a hero by time. A run stored without the setting is Relaxed. The boss and its minions are bots and have no clock.
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

- A boss (`content/<theme>.json`, its custom cards in `content/boss_cards.json` or `cards.json`) has its colours, a thematic deck written in brackets (`brackets`, Difficulty), a commander card, a brain (Hard by default), a base life and life per hero, an opening hand, one passive (`rules`), an Enrage (`enrage`), two phases, a relic of its own (`relic`) and an art card: a real card (`art.card`, `art.oracle_id`) whose art crop pictures it on the descent and the fight panel. `since` is the content generation it joined in; `deck` is the 40-card deck runs of a generation before 4 play. A boss of a later generation is drawn only for runs of that generation or later, so the ladder of an older run never changes (a test pins generation-4 ladders).
- Bracket decks: Standard-style decks of real cards that play automatically (Verified or Read), in the boss's colours, at most four copies of a card, 40 to 45 % basic lands. `base` is the bracket 1 deck of 40 cards; each of `steps` makes the next bracket from the one below by the cards it adds and cuts: 50 cards at bracket 2, 60 from bracket 3, swaps for brackets 4 and 5. Bracket 1 plays cards of mana value 5 or less, bracket 2 of 6 or less, bracket 3 of 7 or less. Theme comes before strength: each deck plays the boss's creature type and supports its passive.
- Commander card: a real card of any type (`commander`) that waits in the boss's command zone, where nothing casts it (`PlayerSetup::held`). The boss's last own phase carries a state trigger: while the card is there, the boss casts it without paying its mana cost; a card that can't be cast then goes to the graveyard. The intro and the boss bar list it as "Phase III: <card>" until then, and the cast is logged as any spell. The plan names it (`BossSeat.commander`), the deck list sent for the boss's seat ends with it and `FightPlan::apply` moves it from the library to the command zone.
- Upgraded boss: a boss with an `upgrade` also stands as a final boss under a new name with one extra boss card (`<id>_ascendant`, `Boss::upgraded`): the same deck, commander, phases, Enrage and relic, at the final boss's bracket; `upgrade.since` is the generation the form joins in (Vex, Seraphine and Ysolde from 5). It is drawn only for a ladder its own lieutenant form is not on, and it never fields its own minion.
- No loops: no boss deck holds a known infinite combo (a content test vets card pairs and plays every deck at every bracket under the loop detector). The bots of the boss's side answer any loop offer with at most 5 iterations or Stop and never a draw (`loop_answer_in`, `BOSS_LOOP_CAP`).
- Its advantages are open: its passive, Enrage, modifier and relic are emblems in its command zone from the start; each phase is an emblem that arrives when its life falls to a share of its starting life (66 % and 33 %). The table shows the boss's phase and the next one on the seat's chips.
- Enrage: each boss's own, from the heroes' seventh turn at Heat 1 (ninth at Heat 0, sixth at Heat 2; `turn` on the card, read by the boss bar and the intro). It ends a stalled fight in the boss's manner: a growing swarm, growing counters, a growing toll or a growing attack.
- Modifier: one buff drawn from the theme's pool (`modifiers`) for each fight, from the seed and the room, so a retried fight keeps it; the first boss at Heat 0 draws none.
- Relic: as the Difficulty table says (from the second boss at Heat 1 and 2, the third at Heat 0), the boss brings its own risky relic, which helps the boss and may help the heroes too.
- Every boss has Escalation (each of its end steps, every hero loses 1 life for every four turns it has taken).
- Drain rule: a boss has at most one life loss with no way out, Escalation. Every other life loss of a boss card, phase, scheme, modifier or relic happens only unless the hero pays (mana or a discard), and its text says so; a content test reads each card's abilities and enforces it.

| Boss | Colours, deck | Commander card | Passive | Enrage (from the heroes' turn 7 at Heat 1) | Relic (Difficulty) |
|---|---|---|---|---|---|
| Skrix | B, Rats | Ashcoat of the Shadow Swarm | a Rat each upkeep | Rats each upkeep: the heroes' turns beyond six | Rat King's Crown: Rats get +1/+1 |
| Mossjaw | G, Trolls and beasts | Thrun, Breaker of Silence | its creatures +1/+1 and trample | a +1/+1 counter on each of its creatures each upkeep | Heartwood Bark: all creatures +0/+2 |
| Vex | BR, Zombies | Gisa, the Hellraiser | a Zombie when a hero's nontoken creature dies | each hero loses 3 unless they pay {2}, each upkeep | Bone Reliquary: creature spells cost {1} less |
| Grimgut (final) | R, Goblins | Krenko, Tin Street Kingpin | a hasty Goblin each upkeep | its creatures +1/+0 each combat per heroes' turn beyond six | Crown of Cinders: all creatures have haste |
| Seraphine | W, Angels and lifegain | Akroma, Angel of Wrath | its creatures have lifelink | a creature card at random from its graveyard to the battlefield each upkeep per heroes' turn beyond six | Halo of Wings: creatures with flying get +1/+1 |
| Ysolde | U, Merfolk and krakens | Lorthos, the Tidemaker | taps a hero's creature each upkeep | each hero mills two per heroes' turn beyond six, each upkeep | Tidal Codex: instants and sorceries cost {1} less |
| Hollowmere | WU, Spirits | Geist of Saint Traft | a 1/1 flying Spirit when its nontoken creature dies | 1/1 flying Spirits: the heroes' turns beyond six, each upkeep | Lantern of Lost Souls: creatures with flying have vigilance |
| Morwenna | UB, Faeries | Alela, Cunning Conqueror | the hero in the spotlight discards a card unless they pay {1}, each upkeep | creature cards at random from the heroes' graveyards onto her battlefield, one per heroes' turn beyond six | Moonlit Veil: creatures with flying get +1/+0 |
| Korrath | RG, Dinosaurs | Ghalta, Primal Hunger | a +1/+1 counter on its creature whenever it is dealt damage | 3/3 trampling Dinosaurs: the heroes' turns beyond six, each upkeep | Amber of the Ancients: creature spells with power 4 or more cost {1} less |
| Sahrra | GW, Cats | Jazal Goldmane | a +1/+1 counter on each of its Cats when one of its Cats dies | 2/2 Cats: the heroes' turns beyond six, each upkeep | Sunmane Pelt: Cats get +1/+1 |
| Valdric | WB, Vampires | The Haunt of Hightower | a +1/+1 counter on each of its Vampires when a hero's creature dies | 2/2 lifelink Vampires: the heroes' turns beyond six, each upkeep | Chalice of the Crimson Host: creatures with lifelink get +1/+0 |
| The Unmaker (uber) | colourless, Eldrazi on Wastes | Ulamog's Dreadsire | the heroes' cost colours drawn anew each round (Uber boss) | 4/4 Eldrazi: the heroes' turns beyond six, each upkeep | Prism of Unmaking: colourless spells cost {1} less |

- Phases of the new bosses: Seraphine's creatures get +1/+1 and vigilance, then a 4/4 flying Angel each upkeep; Ysolde's foes' creatures get −1/−0, then her creatures have hexproof and she draws at each end step. Hollowmere's Spirits get +1/+1, then all its creatures fly; Morwenna's creatures have deathtouch, then get +1/+1 and each hero mills two at her end step; Korrath's creatures have trample, then get +2/+0 and haste; Sahrra's Cats have first strike, then her creatures get +1/+1 and trample; Valdric's Vampires have lifelink, then his creatures get +1/+0 and fly; the Unmaker's colourless creatures get +1/+1, then all its creatures have annihilator 1.
- Upgraded forms: Skrix, Empress of the Undercity (a menace Rat at each end step) and Mossjaw the Undying (a 3/3 Troll at each end step); from generation 5 Vex, the Deathless (a 2/2 menace Zombie), Seraphine, Choir Eternal (a 2/2 flying Angel) and Ysolde, Empress of the Abyss (a 3/3 Kraken), each at its end step.
- Roster: the target is the five mono-colour bosses (complete: W Seraphine, U Ysolde, B Skrix, R Grimgut, G Mossjaw) and ten two-colour bosses, one per colour pair (BR Vex; WU Hollowmere, UB Morwenna, RG Korrath, GW Sahrra and WB Valdric from generation 5); the open pairs (UR, BG, UG, WR) are roadmap items.
- Uber boss (`uber`): never a lieutenant or a final boss of its own; from generation 5 at Heat 2 (`UBER_HEAT`), one ladder in four (`UBER_ODDS`, a random stream of its own, so the lieutenants and camps stay as drawn) meets it in place of the final boss, at the final boss's bracket. The Unmaker's passive is the engine rule `CostColorsDrawn` ([engine.md](engine.md), Rules of the game): each round every hero's coloured mana symbols stand for other colours of their commander's identity, so costs keep their mana value and stay castable with the deck's lands; the card, its spoken name and the preview show the cost now. Its deck is colourless Eldrazi on Wastes, which the bracket rules count as basic lands.

- Schemes: the theme's scheme deck of 14, shuffled from the seed for each fight, sets one scheme in motion every other boss turn from its third (`scheme_every`, CR 314 in the manner of Archenemy); a scheme stays in motion until the boss's next turn begins, so the turns between have none. The next one is public as the Omen with the boss turns until it comes. At that cadence a fight of 30 boss turns meets each scheme at most once. Their effects count the heroes (a Rat per hero, a sacrifice and a discard from each hero, a toll on each hero) or name the hero in the spotlight; Hunting Party spares commanders.
- Minions: a lieutenant seat on the boss's side with another boss's deck, its own life and bot brain, sharing the boss's turn; it falls with the boss (`LossReason::LeaderFell`), so the party wins when the boss falls.

## Spotlight

The boss's side spreads its pressure over the party (`mtg-engine`, `spotlight.rs`; bots in `mtg-testkit`, `brain/spotlight.rs`).

- Attention: damage a boss-side source deals a hero, life a hero loses to a boss-side spell or ability, and each permanent of a hero that one removes (3, a token 1) count as pressure on that hero. The window is the last three rounds of the table, or one round per hero for a bigger party.
- Cap: a hero who took more than 40 % of the window's pressure (60 % with two heroes standing, from 6 pressure on) is capped. The boss's side neither attacks a capped hero nor aims removal or player effects at them while another hero is a fair target, unless the capped hero is an overwhelming threat: a presence at least twice every other hero's (and 15 or more), or creatures whose power reaches the boss's life.
- Focus: the boss eyes the uncapped hero with the most presence (creature power, 2 per other nonland permanent, cards in hand) less attention, ties going round the table with the boss's turns; each minion eyes the next hero of that order, so the boss's side splits its targets. The table shows the eyed hero on each boss-side seat ("Eyes Ann").
- Binding: each boss-side seat's attacks and its targeted spells and abilities go at the hero it eyes (`spotlight_aim.rs`). A seat leaves that hero only when (a) the eyed hero can't be attacked or targeted (no legal way, protection, hexproof), (b) another hero is an overwhelming threat or their creatures' power reaches the boss's life while the eyed hero is neither, (c) every attacker that could go at the eyed hero would be blocked and killed with no damage through (a blocker that kills it and lives, or strikes first; flyers need a flyer or reach, menace two blockers) while an attack at another hero is not futile, or (d) the attack kills another hero (not in the grace; damage past each blocker, the biggest attackers blocked first): a kill is always taken. Blocks are free. Outside these cases nothing moves a seat off its eyed hero; inside them the aim follows the order the boss eyes the heroes. Within the aimed hero's board the bots weigh a target by its worth less attention (1 per point) plus 6 for the eyed hero, never by low life alone.
- The log names the reason: "Skrix attacks Chrisser instead of Mattias: Mattias can't be attacked" (`Event::FocusShifted`: the seat, the eyed hero, the hero attacked instead, and one of Unreachable, Overwhelming, Deadly, Futile, Lethal). Targeted spells and abilities follow the same rule without a log line.
- "The hero in the spotlight" (`Ref::Spotlit`) is the controller's focus: a scheme that names it (Marked Prey) goes round the party as its own drain adds attention.
- Grace: until every standing hero has finished three turns, damage and life loss from the boss's side leave a hero at 1 life at least. A hero's own cards are not held back.
- Politics-free: the boss's side reads only the board, the spotlight and the life totals; no deals, no kingmaking.

## Ramp-up

- Lull: the boss's side's creatures can't attack and its schemes wait until the boss's third turn, so the heroes have three turns before the first attack. The Omen shows the first scheme from the start.
- The boss bots hold back sweepers (wipes, and damage or sacrifice to each player or creature without a target) through the boss's first three turns.
- Reserves and Reinforcements begin on the boss's second turn and Warpath on its fourth; Escalation grows by 1 every four boss turns and Enrage begins after the heroes' seventh turn at Heat 1 (Difficulty).

## Commander rules in a fight

- Heroes play their Commander decks under the Commander rules: 40 life at the start of a run (carried after), the commander tax, 21 combat damage from one commander eliminates a hero.
- The boss and its minions have no commander. They take commander damage as normal damage but never lose to 21 of it, so a big commander shortens the fight without ending it in one hit.

## Scaling

One table (`scale.rs`) sets the boss for the party's size, so every size meets the same challenge:

| Heroes | Life | Hand | Reserves | Reinforcements | Warpath | Minions | Last Bulwark |
|---|---|---|---|---|---|---|---|
| 1 | 100 % | base − 5 | | | | | |
| 2 | 100 % | base − 4 | 1 | | | | |
| 3 | 100 % | base − 3 | | 1 | | | yes |
| 4 | 85 % | base − 2 | | 1 | | 1 | yes |
| 5 | 100 % | base − 1 | | 2 | | 1 | yes |
| 6 | 90 % | base | | 2 | 1 | 2 | yes |

- Life is a share of `life + life_per_hero × (heroes − 1)`. Reserves is a 2/2 Zombie each upkeep from the boss's second turn; Reinforcements a card and a 2/2 Zombie from its second; Warpath, from its fourth turn, untaps the boss's creatures after its first combat and adds a combat and a main phase; the Last Bulwark begins with the last phase: no more than three creatures can attack the boss each combat.
- The party-size table is the base; the boss's place on the ladder and the Heat move it (Difficulty).

## Difficulty

The host picks the Heat when creating a run; the creation form and the run header word it as a tier:

| Heat | Tier | In plain words |
|---|---|---|
| 0 | Casual | any deck can win with some luck |
| 1 | Seasoned | needs a deck that suits the format |
| 2 | Heroic | needs a team of decks that work together and suit the format |

Within a run the bosses ramp with the party's relics, loot and upgrades: the first boss (`Run::boss_rank` 0) is the gentlest and each later one harder. The deck's bracket carries most of the step (`scale.rs`, `BRACKETS`): a bracket is abstract deck power, and a lower bracket also plays a smaller deck (40 cards at bracket 1, 50 at bracket 2, 60 from bracket 3):

| Heat | Fight 1 | Fights 2 and 3 | Fight 4 |
|---|---|---|---|
| 0 Casual | 1 | 2 | 3 |
| 1 Seasoned | 2 | 3 | 4 |
| 2 Heroic | 3 | 4 | 5 |

Runs of content generation 4 and later (`BRACKETS_FROM`, `Scale::of_content`) play the bracket decks and the commander card under the bracket tier table (`BRACKET_TIERS`), whose life, hand and brain move less than before:

| Heat | Rank | Bracket | Life | Hand | Brain | Minions | Relic | Modifier |
|---|---|---|---|---|---|---|---|---|
| 0 | 0 | 1 | 75 % | −1 | −1 | −1 | | |
| 0 | 1 | 2 | 80 % | −1 | −1 | −1 | | yes |
| 0 | 2 | 2 | 85 % | | | | yes | yes |
| 0 | 3+ | 3 | 90 % | | | | yes | yes |
| 1 | 0 | 2 | 85 % | −1 | −1 | −1 | | yes |
| 1 | 1 | 3 | 90 % | | | | yes | yes |
| 1 | 2 | 3 | 95 % | | | | yes | yes |
| 1 | 3+ | 4 | 100 % | | | | yes | yes |
| 2 | 0 | 3 | 95 % | | | | | yes |
| 2 | 1 | 4 | 105 % | | | | yes | yes |
| 2 | 2 | 4 | 110 % | | | | yes | yes |
| 2 | 3+ | 5 | 120 % | +1 | +1 | | yes | yes |

A run of an earlier generation keeps the decks as written and the rank × Heat table below (`TIERS`). That table by Heat and rank moves the party-size table, so every party size meets the same ramp; a ladder longer than four bosses plays the last row for the rest. Life is a share of the party-size life (the minions' life too), hand and minions are added to the party-size numbers, brain moves the boss's and its minions' bot level (Hard by default), relic brings the boss's own relic, modifier draws one from the theme's pool:

| Heat | Rank | Life | Hand | Brain | Minions | Relic | Modifier |
|---|---|---|---|---|---|---|---|
| 0 | 0 | 60 % | −2 | −1 | −1 | | |
| 0 | 1 | 70 % | −2 | −1 | −1 | | yes |
| 0 | 2 | 80 % | −1 | | | yes | yes |
| 0 | 3+ | 90 % | | | | yes | yes |
| 1 | 0 | 75 % | −1 | −1 | −1 | | yes |
| 1 | 1 | 90 % | | | | yes | yes |
| 1 | 2 | 100 % | | | | yes | yes |
| 1 | 3+ | 110 % | +1 | | | yes | yes |
| 2 | 0 | 100 % | | | | | yes |
| 2 | 1 | 115 % | +1 | +1 | | yes | yes |
| 2 | 2 | 125 % | +1 | +1 | | yes | yes |
| 2 | 3+ | 140 % | +2 | +1 | | yes | yes |

- Enrage begins after the heroes' ninth turn at Heat 0, their seventh at Heat 1 (as each Enrage card is written) and their sixth at Heat 2 (`ENRAGE_TURN`). The content makes each Enrage card's variant for the other turns on loading (`<id>_at_<turn>`: its turn, its rules text and the turn counts its abilities read), so the boss bar and the intro read the turn the fight plays.
- Heat 1 at rank 2 is the boss as it was balanced before the tiers (Measured); Heat 0 sits below it at every rank and Heat 2 above.
- The drain rule holds at every Heat: no tier adds a life loss, so Escalation stays each boss's one drain with no way out.
- A content test holds both tables monotonic: at every party size a later rank or a higher Heat never gives a boss a lower bracket, less life, hand, brain or minions, never takes away its relic or modifier and never delays its Enrage.
- The run view lists each boss of the ladder with the life it starts with against the party at its rank (`DelveView.boss_lives`), which the ladder and the intro show.

## Loot

- After each lieutenant every hero gets 50 gold, a pick of one of three relics and a pick of one of three cards in their colours (skipping a card gives 10 gold).
- The Satchel: every card a hero earns (loot, shop, an event, the Mirror Pool) goes to their Satchel (`Hero.satchel`), never into the deck, so the deck's land count never shifts. Each fight starts with the whole Satchel in the hero's command zone (`PlayerSetup::satchel`, [engine.md](engine.md), Campaign seats); the hero casts its cards there like cards in hand, and a card cast stays out of it for the rest of that fight. The server lays it over the chosen deck (`ChosenDeck::with_satchel`), and the recipe keeps it. A run stored before the Satchel reads its earned cards as Satchel cards, and its decks play whole.
- Relics (`content/cards.json`) are emblems the hero keeps for the run, each with a role that shapes the party: Healer, Removal, Damage, Support, and Risky relics whose effect is symmetrical and helps the boss too (all red spells cost {2} less, every player draws an extra card, all creatures get +2/+0). Risky relics come only from loot and treasure, never the shop. A boss's own relic (Bosses) is risky in the same way and stays with the boss.
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

- An event (`content/<theme>.json`) is a story and options; each hero answers for themselves and only their own gold, life, relics, cards and commander change. An option is a list of outcomes. Rewards: `Gold`, `Life`, `RelicOf` (a pick of three relics of that rarity), `CardPick` (a pick of one of three cards), `Duplicate` (the Mirror Pool: a copy of a nonland card of the deck or Satchel joins the Satchel; the server lists the cards in `DelveView.mirror` and refuses any other). Costs: negative `Gold` and `Life`, `MaxLife` (the life cap moves for the rest of the run, `Hero.life_cap`; a cut lowers life to the new cap), `LoseCard` (the hero names a card of the deck or Satchel, `Offer::Discard`: a Satchel card leaves it, else one deck copy leaves for the run, `Hero.lost`, which the server takes out of every fight's deck, `ChosenDeck::without`), `LoseRelic` (the hero gives up a relic of their choice, `Offer::Surrender`) and `Bane` (the first commander costs {N} more or gets −N/−N for the run: an emblem in the command zone at every fight, like an upgrade). Neither follow-up can be skipped.
- Events are known trade-offs from generation 3: no outcome is random. Each event of generation 2 has a trade version that replaces it (`since` 3; the older one has `until` 2), and two more join: the Bleeding Altar and the Forge of Kings. A reward grows with its cost: a Common relic costs a little life or a card, a Rare one 12 to 15 life, a 5 cut to the cap or a relic, a Mythic one a 10 cut to the cap, or a 5 cut with a bane or a relic. Every event keeps a free option with a small reward. Older runs keep their events, gambles (`Gamble`, odds and the outcomes of a win and a loss) and one-relic picks (`Relic`).
- The words come from the data, never from the label: `Outcome::words`, `EventOption::summary` ("−8 life, +40 gold"), and its split into `cost` and `reward` ("−10 max life" and "a Mythic relic"; "nothing" for a free option). A content test checks that every option has a summary and that no label states an amount.
- A sure cost is a need (`EventOption::needs`): that much gold, more than that life (an answer never takes a hero below 1), a life cap of at least 20 after a cut (`MIN_LIFE_CAP`), a relic to give up, a commander (a creature commander for −N/−N) to weaken. The server refuses an answer whose need is unmet (`Run::accepts`, `Hero::lacks`); a stored log replays as before. Every event keeps an option with no need.
- Bots: in an older run they take the first option they can afford; from generation 3 they weigh each option they can take (a Common relic 8, a Rare 15, a Mythic 28, a card 6, gold by fifths; life lost and cap cuts weigh double when hurt or the cap is low, a relic given up 12, a bane 8 to 10) and take the best, never naming a deck card they cannot see (a card loss with an empty Satchel). They give up their least rare relic and their newest Satchel card.
- Each answer is recorded as a `Resolved` in `Run.results` (what the hero did, the outcomes as gold and life really moved, the gambles rolled and how they fell) and as a chronicle line. A camp's results stay through the next boss fight and clear when that fight is won or the next camp opens.

## Run state

- A run is the fold of its events (`run.rs`): `Content`, `Joined`, `Left`, `DeckChosen`, `Timers`, `Started`, `Voted`, `FightCreated`, `FightEnded`, `Picked`, `Abandoned`. The ladder is regenerated from the seed, never stored.
- Decks are chosen once, in the lobby; afterwards only rewards change them. A fight is made from those decks (`FightPlan.run` names the run, so the game links back); its game lobby refuses a deck change (409 `deck_fixed`). A hero whose deck no longer plays (deleted, nothing playable) holds the fight: the run view lists it in `deck_problems`, and only that hero may then choose another deck (`DeckChosen` while the fight waits for its game).
- The party's list leaves abandoned runs out; the rows stay stored. A run unchanged for 7 days (`STALE_DAYS`) with no live fight is abandoned when the list is read.
- Every random draw comes from the seed keyed by room, hero and purpose (`rng.rs`), so a reload never rerolls loot.
- Content generations: a new run stores the generation it is made with (`RunEvent::Content`, `CONTENT`); an event joins at a generation (`since`) and only runs of that generation or later draw it, so a stored run replays the same events. Generation 2 adds the larger relic pool (a relic's `since`) and the relic filter (Loot); generation 3 (`RARITY_FROM`) adds rarity-driven offers and prices, the Rare and Mythic relics of its own, and the trade events, while an event an older generation met names its last one (`until`). Wares appended to the shop go after the draught for the same reason.
- Votes: only a choice of two camps is voted on; a camp's single way on, the next boss, is entered once every hero answered the camp (stored votes for it fold as nothing). When every hero voted, the room with the most votes wins and a tie goes to the torch bearer, a role that moves one hero on with every cleared room. Bot allies vote once the players did, with the majority, and answer offers with the first choice (rest when hurt, the first affordable relic at the shop, the newest Satchel card at the Mirror Pool).

## Interface

- The run page reads as a descent: a header with the party's life, hearts, Heat and seed; the ladder as a path from the first boss to the final one, every room cleared, current, next or locked; votes show who chose which camp.
- Each camp is a scene of its own (loot, shop, campfire with Temper, event, the Mirror Pool, a card or relic to give up); a risky relic is always marked Dangerous, and every relic chip and card shows its rarity by colour and word. The run header lists each hero's Satchel, commander upgrades and banes, and a cut life cap. An event shows each option's cost and reward before the choice (relic rewards in their rarity's colour, odds in an older run, why an option is shut) and a "What happened" panel with the chronicle after it (Events). Implementation in [web.md](web.md), Delves.
- A fight opens with the boss intro (the run view's `fight`: the plan's boss, life, custom cards, phase thresholds and minions), in place of the game's lobby: the boss's abilities in full, the party's decks and carried life, and Ready to fight; the host also has Retreat, confirmed first. When the game ends the final table stays under the fight's result (`FightEnd`: won or lost, each seat's end, the log's last turns) until the player presses "Continue to the run", or after a 60 s countdown that "Stay" stops; the run then greets the loot or the lost heart.
- The run keeps each fight whose game ended or lost its table (`Run.fights`: the game, the room, won or lost; a voided game and a retreat before a game leave none). The run page lists them newest first as the Fight log, each linking to its recorded game (`/replay/$gameId`), where the whole log of the fight reads back.
- At the table the boss's side sits on top and the party together at the bottom (boss mode); the boss's passive, Enrage (with the heroes' turn it begins on), modifier, relic, phases to come, scheme and Omen are a row of chips with their rules, and the intro lists the same. Their words come from the content records the engine plays, matched to the emblems in the boss's command zone and its `BossView`.

## Content

- Custom cards are not on Scryfall: id, name, type line, rules text, art placeholder key, role and price, and their abilities in the card DSL. The engine plays each as an emblem; the web draws them from the content endpoint.
- A content test checks that every boss, commander and reward card plays automatically (Verified or Read) in the corpus, every deck as written has 40 cards, every bracket deck keeps the rules of Bosses at every bracket and every custom card keeps all its abilities.
- Content generations of bosses: 4 brings the bracket decks, the commander cards, Seraphine, Ysolde and the upgraded forms; 5 brings Hollowmere, Morwenna, Korrath, Sahrra, Valdric, the upgraded forms of Vex, Seraphine and Ysolde and the Unmaker (`ROSTER_FROM`, `RunEvent::Content`; the map is drawn again for the generation the run stores).

## Tuning

- `cargo run --release -p mtg-campaign --example delve_sim -- fights <games> <heroes> <level> [first seed] [threads] [heat]` plays each boss in turn against a fresh party of bot heroes on the four fixture Commander decks; `runs` plays whole runs with life carried and loot taken. Games are untimed and seeded, so each result replays. `DELVE_SIM_RANK` scales each boss as the one at that rank of the ladder; `DELVE_SIM_DROP` (boss cards, phases or schemes by id), `DELVE_SIM_LIFE` (percent), `DELVE_SIM_HAND` and `DELVE_SIM_MINIONS` change the plan to measure what each part is worth; `DELVE_SIM_IDLE_SPIRITS` makes every Spirit pass, to measure what haunt actions are worth. Each line also counts falls, rises and haunt actions. A run plays the newest content generation (`DELVE_SIM_CONTENT` sets another); `DELVE_SIM_BOSSES` (ids, an upgraded form included) plays only those bosses, a boss off the seed's ladder taking the place of the boss of its rank.
- Minutes are estimated at 0.6 per hero turn and 0.5 per turn of the boss's side.
- Each fight prints the pressure each hero took, the round a hero first fell to half life and the round a hero first fell.
- The Heat 1 target for a party of Hard bots, at every size: each fight from the second boss won in 50 to 60 % of fights, a whole run in 20 to 35 %, a fight in 25 to 60 minutes for four heroes; no hero takes more than 40 % of the boss's pressure with three or more heroes (60 % with two).

Measured with Hard heroes against the boss at Heat 1, rank 2 strength (life 100 %, its own brain and hand), on the four fixture Commander decks (three precons and a tuned player deck), `fights 1 <heroes> hard <seed>`, one boss per seed (seeds 1 to 4 meet Skrix, Mossjaw, Vex and Grimgut, seeds 5 to 8 again), with the spotlight and the ramp-up:

| Heroes | Seeds | Fights won | Turns (boss side) | Minutes | Largest hero share (worst) | First at half life | First fall |
|---|---|---|---|---|---|---|---|
| 1 | 1 to 8 | 6/7 | 15.0 (7.1) | 8 | 100 % | round 6.6 | round 6.0 |
| 2 | 1 to 8 | 5/8 | 22.4 (7.5) | 13 | 61 % (74 %) | round 6.0 | round 7.2 |
| 3 | 1, 4 | 2/2 | 23.5 (5.5) | 14 | 49 % (54 %) | round 5.0 | none |

- Won per boss (Skrix, Mossjaw, Vex, Grimgut): one hero 1/1, 2/2, 2/2, 1/2; two heroes 2/2, 1/2, 1/2, 1/2; three heroes 1/1, unfinished, unfinished, 1/1.
- The grace keeps every hero above 0 life through their first three turns.
- Solo seed 5 (Skrix) is not counted: the boss bot repeats one paid ability until the runaway watchdog stops the game in turn 13.
- Three heroes on seeds 2 and 3 and four heroes on seeds 1 to 4 had not ended after 110 minutes of machine time per fight; five and six heroes are slower still and are not in this table.

### Tier targets and measures

Targets by Heat and rank, for a party of Normal bots (a deck of any kind played plainly) and of Hard bots (a deck played well); one fight at a time, each a fresh party at full life:

| Heat | First boss (rank 0) | Middle bosses (ranks 1, 2) | Final boss (rank 3) |
|---|---|---|---|
| 0 Casual | Normal bots win about 80 % | Normal bots win 60 to 70 % | Normal bots win about 50 % |
| 1 Seasoned | Normal bots win about 70 % | Hard bots win 50 to 60 % | Hard bots win about 45 % |
| 2 Heroic | Hard bots win about 50 % | Hard bots win 35 to 45 % | Hard bots win about 30 % |

Measured with two Normal heroes, `DELVE_SIM_RANK=<rank> fights <games> 2 normal 1 6 <heat>` (seeds from 1, the boss following the seed):

| Heat | Rank | Fights won | Skrix, Mossjaw, Vex, Grimgut | Turns (boss side) | Machine time a fight |
|---|---|---|---|---|---|
| 0 | 0 | 38/40 (95 %) | 10/10, 10/10, 10/10, 8/10 | 12.2 (5.8) | about 1 s |
| 0 | 1 | 39/40 (98 %) | 10/10, 10/10, 10/10, 9/10 | 12.7 (6.0) | about 1 s |
| 1 | 0 | 37/40 (92 %) | 10/10, 10/10, 10/10, 7/10 | 13.4 (6.5) | about 1 s |
| 1 | 2 | 1/3 | Skrix 0/1, Vex 1/1, Grimgut 0/1 | 15.0 (7.7) | 7 to 9 min |

- The brain is the largest step: a boss one level below Hard (Normal) loses to two Normal heroes in more than 90 % of fights, so the first boss at Heat 1 and the first two at Heat 0 sit above their targets; Grimgut, the final boss of the theme, is the hardest of the four at every rank.
- The Heat 1 rank 2 sample is three fights (a Hard boss takes 7 to 9 minutes of machine time a fight with six threads) and only shows that the Hard boss beats two Normal heroes more often than not; the Hard-boss rows (Heat 0 ranks 2 and 3, Heat 1 ranks 1 to 3, all of Heat 2) and parties of three or more are not yet measured against their targets.

First-boss tuning of the bracket generation: two Normal heroes against each boss as the first boss at Heat 1 (bracket 2, a Normal brain), `DELVE_SIM_RANK=0 DELVE_SIM_BOSSES=<ids> fights <games> 2 normal <first seed> 6 1`, about 1 to 8 s of machine time a fight with six threads; the target is about 70 % won by the heroes:

| Boss | Life (per hero), hand | Fights won by the heroes | Turns (boss side) |
|---|---|---|---|
| Seraphine | 20 (24), 8 | 28/40 (70 %) | 17.3 (8.4) |
| Ysolde | 38 (38), 10 | 30/40 (75 %) | 15.5 (7.5) |
| Hollowmere | 27 (30), 9 | 22/23 (96 %) | 13.3 (6.2) |
| Morwenna | 17 (21), 8 | 17/31 (55 %) | 18.0 (8.9) |
| Korrath | 31 (30), 10 | 7/8 | 13.1 (6.4) |
| Sahrra | 27 (30), 9 | 6/8 | 13.4 (6.6) |
| Valdric | 27 (30), 9 | 7/8 | 14.5 (7.0) |

- Before tuning the heroes won 40 of 75 fights against Seraphine at 27 life and 9 cards, 69 of 75 against Ysolde at 27 life and 9 cards, and 1 of 8 against Morwenna at 26 life with a toll of {2}.
- The samples are small: 40 fights for Seraphine and Ysolde, 31 and 23 for Morwenna and Hollowmere, 8 for Korrath, Sahrra and Valdric, so a rate can move by 15 points or more between series. Hollowmere, Korrath and Valdric sit above the target and Morwenna below it (dl15l).
- Hollowmere plays no attack tax: Windborn Muse made the party's shared attack cost more than a hero could pay and stopped the fight (`AttackUnaffordable`, dl2c). One Morwenna fight in 32 ran into the watchdog: a hero deck's Archangel of Tithes taxes the boss's attack, and the Normal boss bot declared an attack it could not pay and cancelled the payment again and again (dl15p).
