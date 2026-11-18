# Delves

Design of Delves, the co-op campaign: a party of 1 to 6 heroes fights a ladder of bosses with their own Commander decks.

## Shape

- A run is a boss rush. Its ladder (`mtg-campaign`, `map.rs`) holds three of the theme's lieutenants drawn from the seed, then a final boss drawn from the theme's final bosses and the upgraded forms of the lieutenants left off the ladder (a run of a content generation before 4 meets every lieutenant of its generation, then the theme's final boss); between two bosses the party votes for one of two camps: Treasure, Shop, Event or Campfire.
- Every boss fight is a fresh, normally recorded Commander game: board, hand and library reset. The party is one team of seats from seat 0, the boss one seat after them and its minions after it (`FightPlan`).
- Each side takes one shared team turn (`TeamMode::SharedTurns`, CR 805): every hero untaps, draws, plays a land and casts sorceries in the party's turn, a side's turn is led by its first seat (`turn_lead`: the first living hero, the boss) and priority goes round the table from it. In the declare-attackers step each player of the side chooses the attacks of their own creatures in turn order, seeing what the teammates before chose; the side declares every choice at once, so limits, requirements and "attacks alone" read the whole declaration (`party_attacks`), and each hero pays the attack costs of their own creatures. A choice illegal only together is refused with its reason and chosen again; after two refusals the engine declares the plain legal attack. Life totals stay per hero. The table names the party's turn ("Your party's turn").
- Life carries over between fights. After a won fight each hero keeps their end life and heals 15; a hero who ended it as a Spirit comes back at 10 first. A Campfire rest heals 20, the shop's draught 15; nothing heals above 40 plus relic bonuses.
- The party has 3 hearts. A lost fight costs one and the fight is retried: against the first boss every hero regroups at full life, against a later one at half life; with no hearts left the run ends in defeat.
- From content generation 5 a run is endless: when a floor's final boss falls the party votes to bank the win or descend to the next floor (Endless).
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

Runs of content generation 8 and later (`REWORK_FROM`) play the reworked boss (`boss_plan.rs`): one innate, one encounter modifier, two phases by round, a satchel, a precon deck and relics by depth. The boss has nothing else; its brain is Hard at every Heat and depth.

- Record (`content/<theme>.json`): `deck_file` names its stored Commander precon (`content/decks/<code>-<name>.json`, the face commander shuffled in as an ordinary card), `innate` its innate card, `round_phases` its Phase 1 and Phase 2 cards, `satchel` its four satchel spells (`reusable`, or once per fight), `makeup` what its deck is made of, `art` the real card whose art crop pictures it. `deck` stands in until the precon lands.
- Depth: the bosses the party defeated in the run over all floors (`Run::defeated`); a lost and retried fight adds none.
- Innate: written once with a number N over five levels (`"levels": [n1, …, n5]`, `{N}` in its text, `"$N"` in its abilities); loading makes the card of each level (`<id>_i` to `<id>_v`, `innate.rs`) and refuses levels that fall. The level is `1 + (depth points + Heat) / 2`, at most V (`Scale::innate_level`); depth points are the depth for the first four bosses, then one more per two bosses.

| Depth | 0 | 1 | 2 | 3 | 4–5 | 6–7 | 8–9 | 10–11 | 12+ |
|---|---|---|---|---|---|---|---|---|---|
| Heat 0 Casual | I | I | II | II | III | III | IV | IV | V |
| Heat 1 Seasoned | I | II | II | III | III | IV | IV | V | V |
| Heat 2 Heroic | II | II | III | III | IV | IV | V | V | V |

- Encounter modifier: one game-warping rule for the whole fight, drawn from the pool both themes share (`content/modifiers.json`, its cards in `content/encounters.json`) by the seed keyed by floor and room, so a retried fight keeps it. A modifier never meets a boss its `not_with` names (`modifiers.rs`).
- Phases: Phase 1 begins at the beginning of the end step of the last turn of round 8, Phase 2 of round 14 (`PHASE_ROUNDS`, `PhaseStart::AtRound`). A round is one turn of each seat or team (`Turn::round`); an extra turn does not advance it. A phase is an emblem for the rest of the fight; its "when this phase begins" ability (`TriggerPattern::PhaseBegins`) uses the stack once.
- Satchel: four spells in the boss's command zone, cast like cards in hand and public; a reusable one returns as it leaves the stack and costs {2} more for each earlier cast this fight, a once card does not return.
- Relics: `depth − offset` relics, the offset 4, 3 and 2 at Heat 0, 1 and 2, at most 6 (`boss_relics.rs`). They come from the heroes' whole relic pool of the run's generation, risky relics included, never one that names teammates (`team`), and only those the deck's makeup can use. Each is Rare by chance (10, 15 or 20 % at Heat 0, 1 and 2 with one relic, 10 points more for each further relic, at most 60 %) or, from four relics at Heat 1 and 2, Mythic (5 % at four, 10 at five, 15 at six); the rest are Common. Copies can repeat and stack. The draw is keyed by floor and room.
- Life: 40 and 15 for each hero after the first (`Scale::boss_life`); Heat and depth do not change it.
- Its advantages are open: the innate, modifier and relics are emblems in its command zone from the start, named with their rules on the seat's sheet (`boss_sheet.rs`, `Reworked::sheet`). The intro (`DelveFightView`) lists the two effects, the phases with their rounds ("Phase 1 at round 8"), the satchel with each card's mark and the relics; at the table the bar shows the innate with its level, the modifier and relics as chips, and a pill counting down ("Phase 1 in 3 rounds") that names the phase in force once it begins, with the next one's countdown. A reusable satchel card reads "returns · +{N}" with its tax now, a once card "once" (`BossView`: `innate`, `modifier`, `round_phases`, `phase_in`, `relics`, `satchel`).
- No loops: the bots of the boss's side answer any loop offer with at most 5 iterations or Stop and never a draw (`loop_answer_in`, `BOSS_LOOP_CAP`).
- Drain rule: a boss's innate, modifier, phase or satchel card takes life from a hero only as damage or unless the hero pays.
- Relics for heroes: from generation 8 a relic can drop again; owning it never lowers its chance, and copies stack.

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

- The boss bots hold back sweepers (wipes, and damage or sacrifice to each player or creature without a target) through the boss's first three turns.

## Commander rules in a fight

- Heroes play their Commander decks under the Commander rules: 40 life at the start of a run (carried after), the commander tax, 21 combat damage from one commander eliminates a hero.
- The boss and its minions have no commander. They take commander damage as normal damage but never lose to 21 of it, so a big commander shortens the fight without ending it in one hit.

## Difficulty

The host picks the Heat when creating a run; the creation form and the run header word it as a tier:

| Heat | Tier | In plain words |
|---|---|---|
| 0 | Casual | any deck can win with some luck |
| 1 | Seasoned | needs a deck that suits the format |
| 2 | Heroic | needs a team of decks that work together and suit the format |
Depth and Heat act on the boss through its innate level and relics (Bosses); the party size sets its life.

## Endless

From content generation 5 (`ENDLESS_FROM`) a run continues below its first floor (`Run.floor`, from 1).

- Floor cleared: when a floor's final boss falls the run waits on the cleared floor (`Status::FloorCleared`, `Run.floors_cleared` counts it). Each hero votes to bank the win or descend (`RunEvent::FloorVoted`, `DelveAction::FloorVote`); with every vote in, the most votes win and a tie goes to the torch bearer's vote, as the camp vote. Bank ends the run in victory.
- Descent: the next floor's ladder is drawn from the seed keyed by the floor (`map::generate_floor`): three lieutenants and a final boss as on the first floor from the roster of the run's generation (the uber boss included), avoiding by identity the bosses of the floor above while enough others are left. The heroes keep their life, relics, Satchel, upgrades, banes and gold; the party gets one heart back, up to 3 (`DESCENT_HEARTS`). The floor's first boss awaits at once.
- Descent loot: from content generation 7 (`DESCENT_LOOT_FROM`) a vote to descend first gives every hero the final boss's loot as after a lieutenant (Loot: 50 gold, a relic pick and a card pick, drawn for the floor), the party still in the cleared floor's last room (`Status::Room`, `Run.descending`); the descent follows once every hero answered. Bank takes no loot. A run of generation 5 or 6 descends at once, so its log replays unchanged.
- Bots: bot allies vote once every player voted; they descend while the party holds 2 hearts or more and 60 % of its life cap or more (`Run::bots_descend`), else bank.
- The first floor of an endless run is the ladder `generate_for` draws and plays at its scale and fight plans as before; only the final boss's fall asks the vote. A run of an earlier generation ends in victory at its final boss as before.

## Loot

- After each lieutenant, and from generation 7 after a final boss the party descends from (Endless), every hero gets 50 gold (`LOOT_GOLD`), a pick of one of three relics and a pick of one of three cards in their colours (skipping a card gives 10 gold).
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

- A run is the fold of its events (`run.rs`): `Content`, `Joined`, `Left`, `DeckChosen`, `Timers`, `Started`, `Voted`, `FightCreated`, `FightEnded`, `Picked`, `FloorVoted`, `Abandoned`. The ladder is regenerated from the seed, never stored.
- Decks are chosen once, in the lobby; afterwards only rewards change them. A fight is made from those decks (`FightPlan.run` names the run, so the game links back); its game lobby refuses a deck change (409 `deck_fixed`). A hero whose deck no longer plays (deleted, nothing playable) holds the fight: the run view lists it in `deck_problems`, and only that hero may then choose another deck (`DeckChosen` while the fight waits for its game).
- The party's list leaves abandoned runs out; the rows stay stored. A run unchanged for 7 days (`STALE_DAYS`) with no live fight is abandoned when the list is read.
- Every random draw comes from the seed keyed by room, hero and purpose (`rng.rs`), so a reload never rerolls loot.
- Content generations: a new run stores the generation it is made with (`RunEvent::Content`, `CONTENT`); an event joins at a generation (`since`) and only runs of that generation or later draw it, so a stored run replays the same events. Generation 2 adds the larger relic pool (a relic's `since`) and the relic filter (Loot); generation 3 (`RARITY_FROM`) adds rarity-driven offers and prices, the Rare and Mythic relics of its own, and the trade events, while an event an older generation met names its last one (`until`). Wares appended to the shop go after the draught for the same reason.
- Votes: only a choice of two camps is voted on; a camp's single way on, the next boss, is entered once every hero answered the camp (stored votes for it fold as nothing). When every hero voted, the room with the most votes wins and a tie goes to the torch bearer, a role that moves one hero on with every cleared room. Bot allies vote once the players did, with the majority, and answer offers with the first choice (rest when hurt, the first affordable relic at the shop, the newest Satchel card at the Mirror Pool).

## Interface

- The run page reads as a descent: a header with the party's life, hearts, Heat, floor and seed; the ladder of the floor as a path from the first boss to the final one, every room cleared, current, next or locked, with its floor; votes show who chose which camp.
- A cleared floor shows the choice to bank the win or descend, each hero's ballot and what a descent changes (`DescendPanel`), and the loot before a descent names the floor it leads to; the run summary counts the floors cleared and the fight log names each fight's boss and, below the first floor, its floor.
- Each camp is a scene of its own (loot, shop, campfire with Temper, event, the Mirror Pool, a card or relic to give up); a risky relic is always marked Dangerous, and every relic chip and card shows its rarity by colour and word. The run header lists each hero's Satchel, commander upgrades and banes, and a cut life cap. An event shows each option's cost and reward before the choice (relic rewards in their rarity's colour, odds in an older run, why an option is shut) and a "What happened" panel with the chronicle after it (Events). Implementation in [web.md](web.md), Delves.
- A fight opens with the boss intro (the run view's `fight`: the plan's boss, life, custom cards, phase thresholds and minions), in place of the game's lobby: the boss's abilities in full, the party's decks and carried life, and Ready to fight; the host also has Retreat, confirmed first. When the game ends the final table stays under the fight's result (`FightEnd`: won or lost, each seat's end, the log's last turns) until the player presses "Continue to the run", or after a 60 s countdown that "Stay" stops; the run then greets the loot or the lost heart.
- The run keeps each fight whose game ended or lost its table (`Run.fights`: the game, the room, won or lost; a voided game and a retreat before a game leave none). The run page lists them newest first as the Fight log, each linking to its recorded game (`/replay/$gameId`), where the whole log of the fight reads back.
- At the table the boss's side sits on top and the party together at the bottom (boss mode); the boss's passive, Enrage (with the heroes' turn it begins on), modifier, relic, phases to come, scheme and Omen are a row of chips with their rules, and the intro lists the same. Their words come from the content records the engine plays, matched to the emblems in the boss's command zone and its `BossView`.

## Leaderboard

- `GET /api/delves/leaderboard` ([server.md](server.md), Delves) folds every stored run past its lobby, abandoned ones included, into one board per Heat the run was started at (Heroic, Seasoned, Casual), the greatest first.
- Depth (`Run::depth`): the floors whose final boss fell, then the bosses beaten on the floor below them (none while the party stands on a cleared floor). A deeper run ranks first by floors, then by bosses; a tie goes to the run that last changed earlier, then by name.
- Each signed-in player stands once per board with their deepest run: their name in it, its depth, their commanders (the deck name for a deck chosen before commanders were read), the party size with bot allies and the day the run last changed. A run with no boss beaten and a bot ally never stand on it; a board lists at most 20 (`BOARD_LIMIT`).

## Content

- Custom cards are not on Scryfall: id, name, type line, rules text, art placeholder key, role and price, and their abilities in the card DSL. The engine plays each as an emblem; the web draws them from the content endpoint.
- A content test checks that every boss, commander and reward card plays automatically (Verified or Read) in the corpus, every deck as written has 40 cards, every bracket deck keeps the rules of Bosses at every bracket and every custom card keeps all its abilities.
- Content generations of bosses: 4 brings the bracket decks, the commander cards, Seraphine, Ysolde and the upgraded forms; 5 brings Hollowmere, Morwenna, Korrath, Sahrra, Valdric, the upgraded forms of Vex, Seraphine and Ysolde and the Unmaker (`ROSTER_FROM`), and endless floors (`ENDLESS_FROM`, Endless); 6 brings Ozrith, Aurenne, Vorthana and Sivanne (`PAIRS_FROM`); 7 brings the final boss's loot before a descent (`DESCENT_LOOT_FROM`, Endless); `RunEvent::Content` stores the generation and the map is drawn again for it.

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
| Ozrith | 27 (30), 9 | 46/64 (72 %) | 14.5 (7.1) |
| Aurenne | 44 (48), 10 | 48/64 (75 %) | 15.0 (7.3) |
| Vorthana | 32 (36), 9 | 46/64 (72 %) | 16.5 (8.1) |
| Sivanne | 27 (30), 10 | 47/64 (73 %) | 16.0 (7.8) |

- Before tuning the heroes won 40 of 75 fights against Seraphine at 27 life and 9 cards, 69 of 75 against Ysolde at 27 life and 9 cards, 1 of 8 against Morwenna at 26 life with a toll of {2}, 60 of 64 against Aurenne and 53 of 64 against Vorthana at 27 life and 9 cards, 35 of 64 against Vorthana at 38 life and 10 cards, and 50 of 64 against Sivanne at 27 life and 9 cards.
- The samples are small: 64 fights for each generation-6 boss, 40 for Seraphine and Ysolde, 31 and 23 for Morwenna and Hollowmere, 8 for Korrath, Sahrra and Valdric, so a rate can move by 15 points or more between series. Hollowmere, Korrath and Valdric sit above the target and Morwenna below it (dl15l).
- The Unmaker as the final boss at Heat 2 (bracket 5, a Hard brain), `DELVE_SIM_RANK=3 DELVE_SIM_BOSSES=unmaker_void fights 6 2 normal 1 6 2`: no fight ended within 8 minutes of machine time, so it is not measured yet (dl15l).
