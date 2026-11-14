# Delves

Design of Delves, the co-op campaign: a party of 1 to 6 heroes fights a ladder of bosses with their own Commander decks.

## Shape

- A run is a boss rush. Its ladder (`mtg-campaign`, `map.rs`) holds the theme's lieutenants in an order drawn from the seed, then the final boss; between two bosses the party votes for one of two camps: Treasure, Shop, Event or Campfire.
- Every boss fight is a fresh, normally recorded Commander game: board, hand and library reset. The party is one team of seats from seat 0, the boss one seat after them and its minions after it (`FightPlan`).
- Life carries over between fights. After a won fight each hero keeps their end life and heals 10; a hero who ended it as a Spirit comes back at 10 first. A Campfire rest heals 20, the shop's draught 15; nothing heals above 40 plus relic bonuses.
- The party has 3 hearts. A lost fight costs one, every hero regroups at half life and the fight is retried; with no hearts left the run ends in defeat.
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
- Bot Spirits (`brain/haunt.rs`) rise as soon as they can, Ward when 4 or more power attacks the party, and before an opponent's combat Hinder its biggest creature (power 4 or more, 2 ectoplasm held) or Howl (4 held). A bot holding the Phoenix Feather uses it while it has 14 life or more.
- Measured with two Hard heroes at Heat 0, seeds 1 to 4 (`fights`): 0/4 won with Spirits acting and 0/4 idle, 4 falls each; the fallen hero spends its Spirit turns on 3 haunt actions a fight and never reaches Rise Again, so haunting helps at the margin and does not decide a fight.

## Bosses

- A boss (`content/<theme>.json`) has a 40-card deck of verified real cards, a brain (Hard by default), a base life and life per hero, an opening hand, a boss card and two phases.
- Its advantages are open: its boss cards are emblems in its command zone from the start; each phase is an emblem that arrives when its life falls to a share of its starting life (66 % and 33 %). The table shows the boss's phase and the next one on the seat's chips.
- Every boss has Escalation (each of its end steps, every hero loses 1 life for every four turns it has taken) and Enrage (from its tenth turn, every hero loses its turns minus nine each upkeep), so a stalled fight ends.
- Schemes: the theme's scheme deck, shuffled from the seed for each fight, sets one scheme in motion as each boss turn begins from its third (CR 314 in the manner of Archenemy); the next one is public as the Omen. Their effects count the heroes (a Rat per hero, a sacrifice and a discard from each hero, a drain of each hero) or name the hero in the spotlight.
- Minions: a lieutenant seat on the boss's side with another boss's deck, its own life, turn and bot brain; it falls with the boss (`LossReason::LeaderFell`), so the party wins when the boss falls.

## Spotlight

The boss's side spreads its pressure over the party (`mtg-engine`, `spotlight.rs`; bots in `mtg-testkit`, `brain/spotlight.rs`).

- Attention: damage a boss-side source deals a hero, life a hero loses to a boss-side spell or ability, and each permanent of a hero that one removes (3, a token 1) count as pressure on that hero. The window is the last three rounds of the table, or one round per hero for a bigger party.
- Cap: a hero who took more than 40 % of the window's pressure (60 % with two heroes standing, from 6 pressure on) is capped. The boss's side neither attacks a capped hero nor aims removal or player effects at them while another hero is a fair target, unless the capped hero is an overwhelming threat: a presence at least twice every other hero's (and 15 or more), or creatures whose power reaches the boss's life.
- Focus: the boss eyes the uncapped hero with the most presence (creature power, 2 per other nonland permanent, cards in hand) less attention, ties going round the table with the boss's turns; each minion eyes the next hero of that order, so the boss's side splits its targets. The bots weigh a hero by threat less attention (1 per point) plus 6 for the eyed hero, never by low life alone. The table shows the eyed hero on each boss-side seat ("Eyes Ann").
- "The hero in the spotlight" (`Ref::Spotlit`) is the controller's focus: a scheme that names it (Marked Prey) goes round the party as its own drain adds attention.
- Grace: until every standing hero has finished three turns, damage and life loss from the boss's side leave a hero at 1 life at least. A hero's own cards are not held back.
- Politics-free: the boss's side reads only the board, the spotlight and the life totals; no deals, no kingmaking.

## Ramp-up

- Lull: the boss's side's creatures can't attack and its schemes wait until the boss's third turn, so the heroes have three turns before the first attack. The Omen shows the first scheme from the start.
- The boss bots hold back sweepers (wipes, and damage or sacrifice to each player or creature without a target) through the boss's first three turns.
- Reserves and Reinforcements begin on the boss's second turn and Warpath on its fourth; Escalation grows by 1 every four boss turns and Enrage begins on the tenth.

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

- After each lieutenant every hero gets 50 gold, a pick of one of three relics and a pick of one of three cards in their colours (skipping a card gives 10 gold). A new card replaces a basic land of the deck.
- Relics (`content/cards.json`) are emblems the hero keeps for the run, each with a role that shapes the party: Healer, Removal, Damage, Support, and Risky relics whose effect is symmetrical and helps the boss too (all red spells cost {2} less, every player draws an extra card, all creatures get +2/+0). Risky relics come only from loot and treasure, never the shop.
- Camps: Treasure offers a relic pick; the Shop sells three cards (35 gold), two relics and a draught; an Event offers each hero a choice with gold, life, relic or card outcomes and seeded gambles; a Campfire offers rest or a card pick.

## Run state

- A run is the fold of its events (`run.rs`): `Joined`, `Left`, `DeckChosen`, `Started`, `Voted`, `FightCreated`, `FightEnded`, `Picked`, `Abandoned`. The ladder is regenerated from the seed, never stored.
- Decks are chosen once, in the lobby; afterwards only rewards change them. A fight is made from those decks (`FightPlan.run` names the run, so the game links back); its game lobby refuses a deck change (409 `deck_fixed`). A hero whose deck no longer plays (deleted, nothing playable) holds the fight: the run view lists it in `deck_problems`, and only that hero may then choose another deck (`DeckChosen` while the fight waits for its game).
- The party's list leaves abandoned runs out; the rows stay stored. A run unchanged for 7 days (`STALE_DAYS`) with no live fight is abandoned when the list is read.
- Every random draw comes from the seed keyed by room, hero and purpose (`rng.rs`), so a reload never rerolls loot.
- Votes: only a choice of two camps is voted on; a camp's single way on, the next boss, is entered once every hero answered the camp (stored votes for it fold as nothing). When every hero voted, the room with the most votes wins and a tie goes to the torch bearer, a role that moves one hero on with every cleared room. Bot allies vote once the players did, with the majority, and answer offers with the first choice (rest when hurt, the first affordable relic at the shop).

## Interface

- The run page reads as a descent: a header with the party's life, hearts, Heat and seed; the ladder as a path from the first boss to the final one, every room cleared, current, next or locked; votes show who chose which camp.
- Each camp is a scene of its own (loot, shop, campfire, event); a risky relic is always marked Dangerous. Implementation in [web.md](web.md), Delves.
- A fight opens with the boss intro (the run view's `fight`: the plan's boss, life, custom cards, phase thresholds and minions), in place of the game's lobby: the boss's abilities in full, the party's decks and carried life, and Ready to fight. When the game ends the table returns straight to the run, which greets the loot or the lost heart.
- At the table the boss's side sits on top and the party together at the bottom (boss mode); the boss's abilities, phases to come, scheme and Omen are a row of chips with their rules. Their words come from the content records the engine plays, matched to the emblems in the boss's command zone and its `BossView`.

## Content

- Custom cards are not on Scryfall: id, name, type line, rules text, art placeholder key, role and price, and their abilities in the card DSL. The engine plays each as an emblem; the web draws them from the content endpoint.
- A content test checks that every boss and reward card is Verified in the corpus, every boss deck has 40 cards and every custom card keeps all its abilities.

## Tuning

- `cargo run --release -p mtg-campaign --example delve_sim -- fights <games> <heroes> <level> [first seed] [threads] [heat]` plays each boss in turn against a fresh party of bot heroes on the four fixture Commander decks; `runs` plays whole runs with life carried and loot taken. Games are untimed and seeded, so each result replays. `DELVE_SIM_DROP` (boss cards, phases or schemes by id), `DELVE_SIM_LIFE` (percent), `DELVE_SIM_HAND` and `DELVE_SIM_MINIONS` change the plan to measure what each part is worth; `DELVE_SIM_IDLE_SPIRITS` makes every Spirit pass, to measure what haunt actions are worth. Each line also counts falls, rises and haunt actions.
- Minutes are estimated at 0.6 per hero turn and 0.5 per turn of the boss's side.
- Each fight prints the pressure each hero took, the round a hero first fell to half life and the round a hero first fell.
- The target at Heat 0 for a party of Hard bots, at every size: each fight won in 50 to 60 % of fights, a whole run in 20 to 35 %, a fight in 25 to 60 minutes for four heroes; no hero takes more than 40 % of the boss's pressure with three or more heroes (60 % with two).

Measured with Hard heroes at Heat 0 on the four fixture Commander decks (three precons and a tuned player deck), seeds 1 to 4 (`fights 1 <heroes> hard <seed>`, one boss per seed), with the spotlight and the ramp-up:

| Heroes | Fights won | Turns (boss side) | Minutes | Largest hero share (worst) | First at half life | First fall |
|---|---|---|---|---|---|---|
| 1 | 2/4 | 14.8 (7.2) | 8 | 100 % | round 7.0 | round 7.5 |
| 2 | 1/4 | 23.8 (8.2) | 13 | 60 % (85 %) | round 6.2 | round 7.5 |
| 3 | 3/4 | 34.8 (8.5) | 20 | 44 % (49 %) | round 5.8 | round 7.0 |

- The grace keeps every hero above 0 life through their first three turns.
- Four to six heroes take more than an hour of machine time per fight and are not in this table.
