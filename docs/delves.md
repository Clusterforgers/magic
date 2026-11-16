# Delves

Design of Delves, the co-op campaign: a party of 1 to 6 heroes fights a ladder of bosses with their own Commander decks.

## Shape

- A run is a boss rush. Its ladder (`mtg-campaign`, `map.rs`) holds the theme's lieutenants in an order drawn from the seed, then the final boss; between two bosses the party votes for one of two camps: Treasure, Shop, Event or Campfire.
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
- Measured with two Hard heroes at Heat 0 (`fights`): the fallen hero spends its Spirit turns on 3 haunt actions a fight and never reaches Rise Again, so haunting helps at the margin and does not decide a fight.

## Bosses

- A boss (`content/<theme>.json`) has a 40-card deck of verified real cards, a brain (Hard by default), a base life and life per hero, an opening hand, one passive (`rules`), an Enrage (`enrage`), two phases, a relic of its own (`relic`) and an art card: a real card (`art.card`, `art.oracle_id`) whose art crop pictures it on the descent and the fight panel.
- Its advantages are open: its passive, Enrage, modifier and relic are emblems in its command zone from the start; each phase is an emblem that arrives when its life falls to a share of its starting life (66 % and 33 %). The table shows the boss's phase and the next one on the seat's chips.
- Enrage: each boss's own, from the heroes' seventh turn (`turn` on the card, read by the boss bar and the intro). It ends a stalled fight in the boss's manner: a growing swarm, growing counters, a growing toll or a growing attack.
- Modifier: one buff drawn from the theme's pool (`modifiers`) for each fight, from the seed and the room, so a retried fight keeps it.
- Relic: from the second boss of the ladder on (`Run::boss_rank`), the boss brings its own risky relic, which helps the boss and may help the heroes too.
- Every boss has Escalation (each of its end steps, every hero loses 1 life for every four turns it has taken).
- Drain rule: a boss has at most one life loss with no way out, Escalation. Every other life loss of a boss card, phase, scheme, modifier or relic happens only unless the hero pays (mana or a discard), and its text says so; a content test reads each card's abilities and enforces it.

| Boss | Passive | Enrage (from the heroes' turn 7) | Relic (from the second boss) |
|---|---|---|---|
| Skrix | a Rat each upkeep | Rats each upkeep: the heroes' turns beyond six | Rat King's Crown: Rats get +1/+1 |
| Mossjaw | its creatures +1/+1 and trample | a +1/+1 counter on each of its creatures each upkeep | Heartwood Bark: all creatures +0/+2 |
| Vex | a Zombie when a hero's nontoken creature dies | each hero loses 3 unless they pay {2}, each upkeep | Bone Reliquary: creature spells cost {1} less |
| Grimgut | a hasty Goblin each upkeep | its creatures +1/+0 each combat per heroes' turn beyond six | Crown of Cinders: all creatures have haste |

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
- Reserves and Reinforcements begin on the boss's second turn and Warpath on its fourth; Escalation grows by 1 every four boss turns and Enrage begins after the heroes' seventh turn.

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
- Heat raises the difficulty: each Heat gives the boss a card more in its opening hand and the boss and its minions a fifth more life; Heat 2 gives them a stronger brain.

## Loot

- After each lieutenant every hero gets 50 gold, a pick of one of three relics and a pick of one of three cards in their colours (skipping a card gives 10 gold).
- The Satchel: every card a hero earns (loot, shop, an event, the Mirror Pool) goes to their Satchel (`Hero.satchel`), never into the deck, so the deck's land count never shifts. Each fight starts with the whole Satchel in the hero's command zone (`PlayerSetup::satchel`, [engine.md](engine.md), Campaign seats); the hero casts its cards there like cards in hand, and a card cast stays out of it for the rest of that fight. The server lays it over the chosen deck (`ChosenDeck::with_satchel`), and the recipe keeps it. A run stored before the Satchel reads its earned cards as Satchel cards, and its decks play whole.
- Relics (`content/cards.json`) are emblems the hero keeps for the run, each with a role that shapes the party: Healer, Removal, Damage, Support, and Risky relics whose effect is symmetrical and helps the boss too (all red spells cost {2} less, every player draws an extra card, all creatures get +2/+0). Risky relics come only from loot and treasure, never the shop. A boss's own relic (Bosses) is risky in the same way and stays with the boss.
- Camps: Treasure offers a relic pick; the Shop sells three cards (35 gold), two relics, a draught and a commander upgrade (50 gold); an Event offers each hero a choice with gold, life, relic or card outcomes and seeded gambles; a Campfire offers rest, a card pick or Temper, a pick of one of three commander upgrades.
- Commander upgrades (`upgrade.rs`): a cost {1} or {2} lower (generic mana only, never below its coloured pips), a keyword (flying, trample, vigilance, lifelink, haste, menace, first strike, deathtouch, hexproof), +1/+1 or +2/+2, entering with an additional +1/+1 counter, or with an additional loyalty counter. `fitting` offers only what does something for that commander: P/T, counters and keywords for a creature, loyalty for a planeswalker, a reduction while generic mana is left, no keyword it already has, no combat keyword for one with no power; each commander of a pair on its own, each upgrade once. The deck's commander facts (`HeroDeck.commanders`: types, power, generic mana, keywords) are read when it is chosen; a deck chosen before upgrades gets none. Each upgrade is an emblem in the hero's command zone at every fight (a static, cost or replacement ability on "your commander named X"), so the card itself never changes.

## Events

- An event (`content/<theme>.json`) is a story and options; each hero answers for themselves and only their own gold, life, relics and cards change. An option is a list of outcomes: `Gold`, `Life`, `Relic` (a pick of one relic), `CardPick` (a pick of one of three cards), `Duplicate` (the Mirror Pool: a copy of a nonland card of the deck or Satchel joins the Satchel; the server lists the cards in `DelveView.mirror` and refuses any other) and `Gamble` (odds in percent, then the outcomes of a win and of a loss).
- The words come from the data, never from the label: `Outcome::words` and `EventOption::summary` ("−8 life, +40 gold", "50%: a relic · 50%: −8 life"). A content test checks that every option has a summary and that no label states an amount.
- A sure cost, a negative gold or life outcome outside a gamble, is a need (`EventOption::needs`): the hero needs that much gold and more than that life, since an answer never takes a hero below 1. The server refuses an answer whose need is unmet (`Run::accepts`); a stored log replays as before. Bots take the first option they can afford, and every event keeps an option with no need.
- Each answer is recorded as a `Resolved` in `Run.results` (what the hero did, the outcomes as gold and life really moved, the gambles rolled and how they fell) and as a chronicle line. A camp's results stay through the next boss fight and clear when that fight is won or the next camp opens.

## Run state

- A run is the fold of its events (`run.rs`): `Content`, `Joined`, `Left`, `DeckChosen`, `Timers`, `Started`, `Voted`, `FightCreated`, `FightEnded`, `Picked`, `Abandoned`. The ladder is regenerated from the seed, never stored.
- Decks are chosen once, in the lobby; afterwards only rewards change them. A fight is made from those decks (`FightPlan.run` names the run, so the game links back); its game lobby refuses a deck change (409 `deck_fixed`). A hero whose deck no longer plays (deleted, nothing playable) holds the fight: the run view lists it in `deck_problems`, and only that hero may then choose another deck (`DeckChosen` while the fight waits for its game).
- The party's list leaves abandoned runs out; the rows stay stored. A run unchanged for 7 days (`STALE_DAYS`) with no live fight is abandoned when the list is read.
- Every random draw comes from the seed keyed by room, hero and purpose (`rng.rs`), so a reload never rerolls loot.
- Content generations: a new run stores the generation it is made with (`RunEvent::Content`, `CONTENT`); an event joins at a generation (`since`) and only runs of that generation or later draw it, so a stored run replays the same events. Wares appended to the shop go after the draught for the same reason.
- Votes: only a choice of two camps is voted on; a camp's single way on, the next boss, is entered once every hero answered the camp (stored votes for it fold as nothing). When every hero voted, the room with the most votes wins and a tie goes to the torch bearer, a role that moves one hero on with every cleared room. Bot allies vote once the players did, with the majority, and answer offers with the first choice (rest when hurt, the first affordable relic at the shop, the newest Satchel card at the Mirror Pool).

## Interface

- The run page reads as a descent: a header with the party's life, hearts, Heat and seed; the ladder as a path from the first boss to the final one, every room cleared, current, next or locked; votes show who chose which camp.
- Each camp is a scene of its own (loot, shop, campfire with Temper, event, the Mirror Pool); a risky relic is always marked Dangerous. The run header lists each hero's Satchel and commander upgrades. An event shows each option's effects before the choice (gains and costs, odds, why an option is shut) and a "What happened" panel with the chronicle after it (Events). Implementation in [web.md](web.md), Delves.
- A fight opens with the boss intro (the run view's `fight`: the plan's boss, life, custom cards, phase thresholds and minions), in place of the game's lobby: the boss's abilities in full, the party's decks and carried life, and Ready to fight; the host also has Retreat, confirmed first. When the game ends the final table stays under the fight's result (`FightEnd`: won or lost, each seat's end, the log's last turns) until the player presses "Continue to the run", or after a 60 s countdown that "Stay" stops; the run then greets the loot or the lost heart.
- The run keeps each fight whose game ended or lost its table (`Run.fights`: the game, the room, won or lost; a voided game and a retreat before a game leave none). The run page lists them newest first as the Fight log, each linking to its recorded game (`/replay/$gameId`), where the whole log of the fight reads back.
- At the table the boss's side sits on top and the party together at the bottom (boss mode); the boss's passive, Enrage (with the heroes' turn it begins on), modifier, relic, phases to come, scheme and Omen are a row of chips with their rules, and the intro lists the same. Their words come from the content records the engine plays, matched to the emblems in the boss's command zone and its `BossView`.

## Content

- Custom cards are not on Scryfall: id, name, type line, rules text, art placeholder key, role and price, and their abilities in the card DSL. The engine plays each as an emblem; the web draws them from the content endpoint.
- A content test checks that every boss and reward card is Verified in the corpus, every boss deck has 40 cards and every custom card keeps all its abilities.

## Tuning

- `cargo run --release -p mtg-campaign --example delve_sim -- fights <games> <heroes> <level> [first seed] [threads] [heat]` plays each boss in turn against a fresh party of bot heroes on the four fixture Commander decks; `runs` plays whole runs with life carried and loot taken. Games are untimed and seeded, so each result replays. `DELVE_SIM_DROP` (boss cards, phases or schemes by id), `DELVE_SIM_LIFE` (percent), `DELVE_SIM_HAND` and `DELVE_SIM_MINIONS` change the plan to measure what each part is worth; `DELVE_SIM_IDLE_SPIRITS` makes every Spirit pass, to measure what haunt actions are worth. Each line also counts falls, rises and haunt actions.
- Minutes are estimated at 0.6 per hero turn and 0.5 per turn of the boss's side.
- Each fight prints the pressure each hero took, the round a hero first fell to half life and the round a hero first fell.
- The target at Heat 0 for a party of Hard bots, at every size: each fight won in 50 to 60 % of fights, a whole run in 20 to 35 %, a fight in 25 to 60 minutes for four heroes; no hero takes more than 40 % of the boss's pressure with three or more heroes (60 % with two).

Measured with Hard heroes at Heat 0 on the four fixture Commander decks (three precons and a tuned player deck), `fights 1 <heroes> hard <seed>`, one boss per seed (seeds 1 to 4 meet Skrix, Mossjaw, Vex and Grimgut, seeds 5 to 8 again), with the spotlight and the ramp-up:

| Heroes | Seeds | Fights won | Turns (boss side) | Minutes | Largest hero share (worst) | First at half life | First fall |
|---|---|---|---|---|---|---|---|
| 1 | 1 to 8 | 6/7 | 15.0 (7.1) | 8 | 100 % | round 6.6 | round 6.0 |
| 2 | 1 to 8 | 5/8 | 22.4 (7.5) | 13 | 61 % (74 %) | round 6.0 | round 7.2 |
| 3 | 1, 4 | 2/2 | 23.5 (5.5) | 14 | 49 % (54 %) | round 5.0 | none |

- Won per boss (Skrix, Mossjaw, Vex, Grimgut): one hero 1/1, 2/2, 2/2, 1/2; two heroes 2/2, 1/2, 1/2, 1/2; three heroes 1/1, unfinished, unfinished, 1/1.
- The grace keeps every hero above 0 life through their first three turns.
- Solo seed 5 (Skrix) is not counted: the boss bot repeats one paid ability until the runaway watchdog stops the game in turn 13.
- Three heroes on seeds 2 and 3 and four heroes on seeds 1 to 4 had not ended after 110 minutes of machine time per fight; five and six heroes are slower still and are not in this table.
