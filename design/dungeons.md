# Delves: a co-op dungeon campaign for Magic (design pack dg47)

Design for a campaign mode in which a group of friends generates a dungeon, gathers a party and fights through it together against AI monsters and a boss.

---

## Part 1. Summary for the owner

**The idea in one paragraph.** A *Delve* is a run through a randomly generated dungeon. The group (1 to 4 friends, gaps filled by bot allies) picks heroes (a Commander deck each), sees a branching map of rooms and votes on the path. Most rooms are short Magic games against AI monster seats: a small pack of goblins, a necromancer's crypt, an elite with a nasty twist. Other rooms are shops, campfires, treasure and story events. At the end waits a boss: an Archenemy-style AI seat with a large life total, a deck of "schemes" it sets in motion every turn, and phases that change the fight when its life drops. Between rooms the party earns gold, picks new cards that join their decks for the run, and gains relics (permanent boons that sit in the command zone like emblems). A run takes 1 to 3 evenings and saves after every room. A shared code (the seed) lets another group play the same dungeon.

**Why it works with our engine.** Almost all of it is built already: N vs 1 team games, bot seats in four strengths that play for their team, handicaps (life and hand size per seat), emblems, rule-changing effects, dungeons and the initiative, stickers, recorded games that resume after a restart, replays and take-backs. A fight is simply a normal recorded game made from a generated recipe. The campaign layer around it (map, rewards, voting, saving) is new but small, and pure data plus one server module.

**What is genuinely missing.**
1. Custom cards that are not on Scryfall (boss cards, monster rule cards, relics, room cards) with their own printed text and a rendered frame.
2. Setting up a game with objects already in place: emblems/relics in the command zone, a room card, stickered cards in the library.
3. Archenemy pieces: a scheme deck that the boss sets in motion each turn, and shared team turns for a party of 3 or 4 (we have it for Two-Headed Giant only).
4. A "scripted" monster seat that plays from the top of its deck without thinking (like the Theros Hydra or Horde Magic), for fast and predictable minor fights.
5. Bot personalities and a "spread the pain" rule so the AI does not gang up on one friend.
6. The campaign itself: generator, run state, the map, reward and vote screens.

**The five best original ideas** (more in section 3.12):
1. **Nobody sits out: Spirits.** A hero who drops to 0 life becomes a Spirit for the rest of that fight with a small hand of "haunt" cards to help the living. Player elimination, the worst part of group Magic, is gone.
2. **The Nemesis remembers.** The boss adapts to how the party played: if the party won with fliers, the boss gains reach; if a hero dominated, the boss hunts them. Across runs, a boss that killed your party returns, stronger and gloating.
3. **Every room is a place.** Each room has an environment card (like a Planechase plane): a flooded hall where islands tap for two, a burning library where everybody discards at end of turn. Same monsters, very different fights.
4. **The scouts' forecast.** Before the run starts, our bots play the generated dungeon hundreds of times in the background. The map shows each room's measured danger, and the generator throws away dungeons that are unfair. Difficulty is measured, not guessed.
5. **Burdens and a loot draft for mixed skill.** Strong players may volunteer to carry burdens (handicaps) in exchange for shared gold; loot is drafted in snake order with the first pick going to whoever had the hardest fight. The veteran and the newcomer both have something to do and nobody is carried.

**Minimal first version (MVP).** One dungeon theme, one floor of 4 rooms plus a boss on a generated branching map, a party of 1 to 4 (bots fill empty seats), precon Commander decks, card rewards and gold, one campfire and one shop, a seed code, save and resume. About 20 to 25 pieces of 15 minutes for the engine and server, and a similar amount for the web. See Part 4.

**Decisions I need from you** are in Part 5. The big ones: Commander decks vs small starter decks that grow; whether a "dead" hero can return; fight length target; whether the boss may cheat (draw extra, more mana) or must play fair cards.

---

## Part 2. Research: what makes each reference fun for a group, and what fails

| Reference | What it does | Fun for a group | What fails |
|---|---|---|---|
| **Dice Throne Adventures** | Co-op campaign for 1 to 4: four "portal crawls" (rooms of minion fights) alternating with boss battles; loot cards go into your hero's deck; upgrades and salves carry over; boss vs party and boss vs boss modes. | Everyone keeps their own hero and dice identity; loot gives personal growth inside a shared goal; bosses have big signature moves everybody fears together. | Long setup between rooms; loot is luck-heavy; the expansion relies on the base game's heroes, so the campaign layer is thin; solo reviewers find it "just Dice Throne". Lesson: the layer between fights must be fast and meaningful. |
| **Forge Adventure / Shandalar** | Overworld you walk on; towns with shops; roaming enemies you duel for their cards; bosses in castles; your deck grows from winnings (Forge Quest also offers three random opponents to choose from and booster rewards). | Building a deck out of what you won is the strongest hook in Magic PvE; picking which enemy to fight is a real choice. | Single-player; long grind; enemy decks are just constructed decks, so fights feel like normal duels; random rewards often useless for your colours. Lesson: rewards must fit the player's deck; enemies need identity beyond a decklist. |
| **Archenemy** | One player at 40 life with a scheme deck; sets one scheme in motion each turn (some "ongoing"); the others share a team turn. | Clear villain; schemes are loud, dramatic moments the whole table reacts to; shared turns cut waiting. | One archenemy player has a miserable time (we replace them with AI); schemes can snowball; the archenemy picks on one player. |
| **Theros challenge decks (Face the Hydra, Battle the Horde)** | The enemy is a deck that plays itself: each turn it reveals and casts the top card; Hydra heads regrow from the library; win by clearing all heads; "each player" replaces "you" in co-op; difficulty by starting heads. | Zero-thinking enemy, instant turns, puzzle-like fights with a clear goal; difficulty dial is one number. | Little variety once you know the deck; no real decisions from the enemy. Lesson: perfect for minor rooms, not bosses. |
| **Planechase** | A shared plane card changes the rules; rolling the planar die may cause chaos or planeswalk away. | Shared, visible chaos everybody laughs about; low rules overhead. | Swingy; can decide games randomly. Lesson: environments are great when they are tilted mildly and visible from the start. |
| **Dungeon / venture / the initiative** | A dungeon card with rooms connected by arrows; venture moves you a room; the initiative is a contested designation that ventures into the Undercity. | A tiny map with branching choices inside a game; the initiative creates a fight over one target. | Bookkeeping; snowballs for whoever holds it. We already support it; it is a natural in-fight mini objective. |
| **Alchemy (digital)** | Conjure, seek, perpetual changes. | Card changes that persist feel like "levelling up a card". | Paper players find it opaque. We support conjure; perpetual is missing. Our stickers can play the "upgrade" role instead. |
| **Slay the Spire** | Act maps of 15 floors on a layered graph with branching paths; node types (fight, elite, ?, shop, rest, treasure, boss); boss shown in advance; enemies telegraph intents; relics; ascension ladder (e.g. A1 doubles elite frequency). Map rules: no elites or rests on early floors; elites, shops and rests never twice in a row. | Route planning is a fun group conversation; intents turn fights into puzzles; ascension gives endless replay. | Single player; RNG of a bad seed can be unwinnable. Lesson: show information early; validate seeds. |
| **Monster Train** | Several lanes (floors) defended at once; clan pairing; upgrade stones; "covenant" ladder; the boss climbs floors. | Upgrading a card is as exciting as gaining one; small numbers of sharp choices. | Late runs become "solved" combos. |
| **Inscryption** | The opponent is a character who talks, cheats and reacts; tiny decks; sacrifice; deathcards made from your defeats. | Personality and surprise make the enemy memorable; legacy cards from your own failures are emotionally strong. | Hard to procedurally generate personality. Lesson: authored voice lines plus generated content. |
| **Gloomhaven / Frosthaven** | Scenario level = party average level / 2 rounded up, ±1 for easy or hard; monster stats per level and per player count; scenario goals beyond "kill all"; personal battle goals. | Difficulty is one transparent number the group chooses; secret personal goals add play inside co-op. | Long setup; alpha-player problem (one player plays for everyone). |
| **Spirit Island** | Invaders follow a known, visible pattern (explore, build, ravage step by step); fear cards; the game tells you what will happen next turn. | The AI is fully predictable, so co-op is about planning together; difficulty via adversaries and levels. | Heavy rules; long games. Lesson: a telegraphed AI is the best co-op opponent. |
| **Legends of Runeterra, Path of Champions** | PvE roguelite: semi-linear maps to a boss, relics, champion "star powers" that grow across runs, PvE-only cards. Single player. | Long-term account progression on top of short runs; champion identity. | Grind; power creep; no co-op. |
| **Procedural generation practice** | Layered DAGs (StS), mission graphs with lock-and-key, difficulty budgets that buy encounters from tables, seeds for sharing. | Shareable seeds ("try our dungeon"), daily seeds. | Pure random output feels samey; needs authored set pieces inside the generation. |

**What matters most for a group of friends (synthesis).**
- Nobody waits long and nobody is out of the game (shared turns, fast AI turns, Spirits).
- Shared decisions are discussions, not chores (map votes, loot draft, a visible boss plan).
- Each player has personal identity and growth inside the shared goal (own deck, own picks, burdens).
- Difficulty is chosen up front, transparent and adjustable; a mixed table works.
- A session fits an evening and stops cleanly.
- Failure is a story, not a waste (the Nemesis, graves, unlocks).

---

## Part 3. Design

### 3.1 Vocabulary

| Term | Meaning |
|---|---|
| Delve | One run: seed, party, difficulty, map, progress. |
| Hero | A player's seat in the run: a deck, hearts, gold, relics, burdens. |
| Floor | One map with its boss. A full delve has 3 floors; the MVP has 1. |
| Room | A node of the map: Fight, Elite, Event, Shop, Campfire, Treasure, Shrine, Boss. |
| Encounter | What a fight room contains: monster seats, room card, objective, rewards. |
| Monster | An AI seat: a themed deck, a brain (bot level plus personality), affixes, life. |
| Affix | A rule card that makes a monster elite (Vampiric, Swift, Warded, Splitting...). |
| Boss | A monster seat with a boss card in the command zone, a scheme deck and phases. |
| Relic | A boon kept for the run, put into the command zone as an emblem at the start of each fight. |
| Burden | A voluntary handicap a hero carries for shared rewards. |
| Heat | The difficulty ladder (like Ascension): each level adds one named rule. |

### 3.2 The run loop

```
create delve (theme, Heat, seed or random)  ->  party lobby (heroes pick decks, bots fill)
  -> floor map shown with the boss and the scouts' forecast
  -> loop: vote next room -> resolve room (fight = a normal game | screen for shop/event/campfire)
           -> rewards (loot draft, card picks, gold) -> autosave
  -> boss fight -> floor reward -> next floor or victory
  -> run ends: victory, or party wiped (all hearts lost) -> summary, unlocks, Nemesis record
```

- A floor has 6 to 9 rooms in the full game (4 plus boss in the MVP); a fight lasts 15 to 30 minutes; a floor is one evening.
- Save after every room. A fight in progress is a normal recorded game, so a restart resumes it as any game does.
- A missing friend: their hero may be played by a bot for the evening at a level the group chooses ("the hireling carries your pack"), or the run waits.

### 3.3 Heroes, decks and persistence

Two hero styles; the MVP ships the first.

- **Veteran** (MVP): bring a Commander deck (precon or your own). Run rewards are card *swaps* (take a card in, cut a card out; the deck stays 100) and relics. Familiar decks keep the learning curve low.
- **Pilgrim** (later): start from a 40-card starter of the commander's colours (generated from the precon pool) plus the commander; rewards *add* cards (Slay the Spire style growth); campfire and shop can remove cards. The most roguelike feel, best for repeat play.

Persistence across fights:
- **Life does not carry.** Magic games are too swingy for carried life. Each fight starts at the encounter's life (default 20 per hero, the boss fight 30).
- **Hearts** (default 3 per hero) are the run's health. A hero who loses a fight they took part in, or is reduced to 0 during a fight the party wins, loses one heart. A campfire restores one. A hero at 0 hearts is *fallen*: they keep playing as a Spirit in every fight (3.10) and can be revived at a Shrine. The run ends when the whole party loses a fight (a "wipe"); a wipe costs every hero a heart and the party may retry the room once per floor.
- **Scars** (optional, Heat 2+): ending a fight below 5 life adds a Scar to your deck (a dead-weight card with a mild drawback, like StS curses), removed at a campfire or shop.
- **Gold** is personal, may be gifted. **Relics** are personal unless marked "Party".
- **Card upgrades** use our existing **stickers**: at a campfire a hero puts one sticker (power/toughness, ability or name sticker) on a card of their deck for the rest of the run. This is the Magic-native answer to Monster Train's upgrade stones and needs only a setup hook (3.7).

### 3.4 Rooms

| Room | What happens | Notes |
|---|---|---|
| Fight | 1 to 3 monster seats from the floor's theme table, a room card, an objective. | Fast fights; minor monsters can be scripted seats. |
| Elite | One strong monster with 1 to 2 affixes, better reward (relic chance). | Never on the first two rows. |
| Event | A short story choice for the party (vote): gamble life for gold, free a prisoner (a bot ally for the floor), drink from a fountain (random relic or curse). | Authored text, generated pick. |
| Shop | Cards in the hero's colours, sticker sheets, relics, card removal, heart potion. | Personal purchases. |
| Campfire | Each hero chooses: heal a heart, sticker a card, or remove a Scar. | Nobody is forced to agree. |
| Treasure | Shared relic draft. | |
| Shrine | Revive a fallen hero, or take a burden for a strong relic. | |
| Boss | The floor's boss (shown from the start). | |

Objectives beyond "defeat all monsters", to keep fights varied and short: survive N turns, defeat the leader only, protect a caravan (a party-owned token with no abilities), escape (venture to the last room of a dungeon first, using our venture rules), hold the initiative at the end of a turn.

### 3.5 Monsters and AI personalities

A monster entry in the content files:

```json
{
  "id": "goblin_warband",
  "theme": "warrens",
  "colors": ["R"],
  "deck": {"precon_slice": "...", "or": ["oracle ids ..."]},
  "size": 30, "life": 15, "hand": 5,
  "brain": {"kind": "heuristic", "level": "normal", "personality": "reckless"},
  "threat": 4.0,
  "lines": {"enter": "More for the pot!", "dies": "..."}
}
```

- **Brains.** Easy, Normal, Hard and Very Hard stay as they are. A *personality* is a set of `Weights` (`defend`, `press`, `card`, `life`, `board` in `brain/eval.rs`) plus a targeting policy:
  - *Reckless* (goblins): high press, low defend; attacks every turn.
  - *Hoarder* (dragons): card and board heavy; holds back until a big turn.
  - *Coward* (thieves): defends, chump-blocks late, escapes.
  - *Hunter* (wolves): targets the lowest-life hero.
  - *Bully* (ogres): targets the hero with the biggest board.
  - *Fair* (default for groups): rotates its main target across heroes so no friend is ganged up on.
- **Scripted seats** (new `SeatBrain`, small): for hordes and hydras. Each turn the seat plays the top card of its library, attacks with everything, never blocks, picks targets by a fixed rule ("the hero with the most creatures"). Turns take a second; ideal for minor rooms and for the boss's minions.
- **Threat** is a measured number (3.9), not hand-set: the price of the monster in the difficulty budget.

### 3.6 The boss

A boss is a monster seat with three extra parts, all data:

1. **Boss card** in the command zone (a custom card with static and triggered abilities), double-faced: the back is the second phase. Phase change: "When your life total becomes N or less, transform this" (state trigger; check the vocabulary). The transform also sets in motion two schemes at once and may wake a minion seat (minion seats are seated from the start as "dormant" and skip turns until woken; a later phase, G10).
2. **Scheme deck** (Archenemy, CR 314 and 904): at the boss's precombat main phase it sets the top scheme in motion; ongoing schemes stay until abandoned. Schemes are where the boss's big signature moves live ("Your heroes' creatures fight each other").
3. **Omen** (telegraph, from Spirit Island and Slay the Spire): the *next* scheme is face up for everybody. The party plans around it together. Heat levels hide it.

Life scales with the party: `life = base + per_hero * heroes` (start: 25 + 15 per hero), tuned by simulation. Party gets shared team turns (Archenemy rule) so the boss is not drowned by four separate turns and nobody waits three turns.

**The Nemesis** (original): after each floor the boss records what beat it (`NemesisMemory`: party keyword counts, top damage hero, card types cast). On the next floor and in later runs with the same group, the boss's deck gains an *answer package* drawn from a table keyed by the observed pattern (fliers -> reach and "creatures with flying lose flying" scheme; graveyard -> graveyard hate; one dominant hero -> Hunter targeting of that hero, offset by a reward bonus for that hero). A boss that wipes a party keeps a trophy (a named emblem: "Slayer of Chris's Party") and returns in a later delve as an optional revenge room with double rewards.

### 3.7 Expressing it in our DSL and engine (and what is missing)

What we already have and use directly:
- Teams (`HouseRules::teams`, `TeamMode::Teams`): party team vs dungeon team; bots play for their team.
- Handicaps per seat: starting life and hand size; `turn_limit`, `winning_life`.
- `Effect::CreateEmblem`, `Effect::Rule`/`GameRule` (rule changes such as max hand size, mana changes, draw limits, combat quirks), triggers, replacement, layers: enough for nearly all relics, affixes, room cards and schemes written as abilities.
- `PlayerSetup` presets: library, hand, battlefield (`PresetPermanent` with counters and damage), graveyard, life, keywords, counters, stickers.
- Dungeons, venture, the initiative, the monarch: in-fight objectives.
- Conjure (`GameRecipe.conjured`): "the boss conjures a Skeleton into your library".
- Recipes, fingerprints, resume, replays, take-backs (turned off in boss fights at Heat 3+, a house rule we have).

Gaps, smallest first:

| # | Gap | Why | Size |
|---|---|---|---|
| G1 | `PlayerSetup::emblems` (command-zone objects at start) | Relics, room card, boss card as an emblem-like object. | S |
| G2 | Stickered cards in the library at setup | Campfire upgrades persist into the next fight. | S |
| G3 | Custom card records: a printed side from our content files (name, cost, type, text, art reference to an existing illustration), keyed by a campaign id, fingerprinted | Boss cards, affixes, relics, schemes, room cards are not on Scryfall. | M |
| G4 | Web frame for custom cards (rendered text frame, art crop from a chosen printing) | They have no Scryfall image. | M |
| G5 | Scripted `SeatBrain` | Fast horde/hydra seats. | S |
| G6 | Personalities: `Weights` per seat, targeting policies (`Fair`, `Hunter`, `Bully`) in the bot's attack and target choice | Group fairness, monster identity. | M |
| G7 | Schemes: scheme zone, set in motion as a turn-based action, ongoing and abandon (CR 314, 701.25, 904), `SchemeView` | Bosses. | M to L |
| G8 | Shared team turns for N vs 1 without shared life (generalise CR 805 beyond Two-Headed Giant; Archenemy uses it) | Waiting time with 3 to 4 heroes. | M |
| G9 | Objective rules: `GameRule::TeamWinsAtEndOfTurn(n)`, "the team wins when seat X loses" (defeat the leader), protect an object | Varied short fights. | S to M |
| G10 | Dormant seats (skip turns until woken) | Boss summons minion seats mid-fight. | M, later |
| G11 | Spirit state: a player at 0 life who is "fallen" stays in the game on their team with a restricted hand (a replacement of the loss, `CantLoseGame`-like with a Spirit flag; Spirit hand of haunt cards; no permanents) | Nobody sits out. | M |
| G12 | Planechase-style room chaos (planar die) | Optional spice. | M, later |
| G13 | Perpetual (Alchemy) | Only if stickers are not enough. | L, optional |

Note on fairness of a boss that "cheats": anything the boss gets beyond its cards (extra draw, starting mana, double life) is a visible rule on its boss card, so the table always sees it.

### 3.8 Data model

New pure crate `mtg-campaign` (no IO, like the engine): content types, generator, run state as a fold of run events, encounter-to-`GameSetup` builder. Content in `crates/mtg-campaign/content/` (themes, monsters, affixes, bosses, schemes, relics, room cards, events), checked by a validation test like the rules files.

```
Delve      { id, seed, generator_version, content_version, theme, heat, party: [Hero],
             floor, position: NodeId, status: Lobby|Map|InRoom|Ended(result) }
Hero       { seat, user_id?, bot_level?, deck: DeckRef, deck_changes: [Swap|Add|Remove|Sticker],
             hearts, gold, relics: [RelicId], burdens: [BurdenId], scars, fallen }
FloorMap   { rows: [[Node]], edges: [(NodeId, NodeId)], boss: BossId }     // derived from seed
Node       { id, row, kind, encounter?: EncounterSpec, forecast?: Forecast }
Encounter  { monsters: [MonsterSpec{id, affixes, level, personality}], room_card, objective,
             hero_life, rewards: RewardSpec }
RunEvent   = DelveCreated | HeroJoined | VoteCast | NodeEntered | FightStarted{game_id}
           | FightEnded{result, per_hero} | RewardOffered | RewardTaken | Purchased
           | CampfireChoice | EventChoice | HeartLost | HeroFell | Revived | FloorCleared | DelveEnded
```

- The map is never stored: it is regenerated from `(seed, generator_version, content_version, heat, party size)`, exactly like the engine replays from a seed. Stored: the `run_events` log. This mirrors our game design (state is the fold of the events) and makes seeds shareable for free.
- Rewards are drawn from the run's random stream keyed by (seed, node, hero), so a reload never rerolls loot (no save-scumming).
- Postgres: `delves (id, owner, recipe jsonb, status, created, updated)`, `delve_members (delve, seat, user_id, ...)`, `delve_events (delve, seq, event jsonb)`, `nemesis (group_key, boss, memory jsonb)`. A fight's `games` row carries `delve` and `node` so its result flows back.
- `GameRecipe` gains `campaign: Option<CampaignFight {delve, node}>` and the presets of G1/G2; the custom cards (G3) are part of the deck fingerprints, so a changed boss card cannot silently alter a resumed fight.

### 3.9 Generation and difficulty budget

**Map** (layered DAG, after Slay the Spire, adapted for a group):
1. Rows R (MVP 4, full 7 to 9) and lanes L = 3 or 4. Draw 3 to 4 paths from row 0 to the last row, each step moving to the same or neighbouring lane; merge where they meet; drop unused nodes.
2. Assign kinds by row rules: row 0 always Fight; no Elite or Campfire in rows 0 to 1; the row before the boss is all Campfire; Elite, Shop and Campfire never twice in a row on a path; at least one Shop and one Treasure reachable on every path; each path has 1 or 2 Elites (more at Heat 1+).
3. Insert one **set piece** per floor from the theme (an authored room: "The Drowned Chapel") so maps do not feel generic.
4. Place encounters by budget (below), then run the **scouts' forecast** (3.11) and reroll if a node's measured win rate is outside its band.

**Budget.**
```
budget(node) = base[kind] * row_factor(row) * party_factor(heroes) * heat_factor(heat)
party_factor = 1.0, 1.6, 2.1, 2.6 for 1..4 heroes (initial guess, then fitted)
```
An encounter buys monsters from the theme's table until the budget is spent: `threat(monster) + sum(threat(affix))`, with at most 3 monster seats and at least one "anchor" (a non-scripted monster). Room cards are free but tagged (tilts toward party or monsters) and balanced per encounter.

**Threat** is fitted, not guessed: an Elo-like rating per monster deck, affix and boss from simulated fights against reference parties (Normal-level bot heroes playing precons). Threat equals the rating converted to "how many reference heroes it beats 50%".

**Heat ladder** (each level keeps the previous ones): 1 more elites; 2 scars; 3 boss omen hidden, no take-backs in boss fights; 4 monsters one bot level higher; 5 heroes start fights at 15 life; 6 boss has a third phase; 7 shops cost more; 8 Nemesis memory on from floor one; 9 one fewer campfire; 10 fewer hearts. Each cleared Heat unlocks the next for the group.

### 3.10 Fairness and fun for mixed-skill groups

- **Spirits** (G11): fallen in a fight, you are a Spirit: you take your part of the shared team turn, may play one haunt card each turn (tap a monster, give a teammate +1/+1 until end of turn, scry for a teammate), and are back to life at the next fight (losing a heart). Nobody watches from the sidelines.
- **Loot draft:** shared rewards (relics, treasure) are laid out face up and drafted in snake order. First pick goes to the hero who took the most damage in the fight (catch-up, Mario Kart style), not to the best player.
- **Personal card picks** are drawn from the hero's colours and deck tags, so every reward is playable.
- **Burdens** (original): before a floor, each hero may take burdens ("start each fight with one card fewer", "your commander costs 2 more") and earns gold for the *party* for each. An experienced friend levels the table without anyone being labelled "the beginner".
- **Mentor seat:** an option where a beginner's hero has a bot on Hard suggest a play (preview the answer the bot would give, as a hint button). Uses the existing brain; no new rules.
- **Fair targeting** default for monsters (rotates targets) and visible aggro: each monster shows whom it is focused on.
- **Votes:** the map path is voted; ties go to the **Torch bearer**, a role that rotates each room. Rewards and campfire choices are personal, so votes stay few and quick. A 30-second vote clock with "follow the torch" as default.

### 3.11 Balancing: simulate runs with bots

- `mtg-testkit` gets a `delve_sim` driver: build the encounter's `GameSetup`, play it with bots (heroes on Normal, the reference party), collect win rate, turns and life left. Runs on the local machine (CPU only, no tokens), in the existing thinking slots or a CLI (`mtg-cli delve sim --theme warrens --n 200`).
- **Targets** for Heat 0 with the reference party: Fight 90% win, Elite 70%, Boss 55%, whole floor about 40 to 50%. Fight length: median 8 turns, 95th percentile 14.
- **Threat fitting:** each night, simulate new or changed monsters against a ladder of reference decks; update ratings in a generated table that is committed with the content (`content/threat.json`), so the generator is deterministic for a given content version.
- **The scouts' forecast** (original): when a delve is created, the server simulates each fight room a few dozen times at low bot budget in the background (the Easy/Normal brains are cheap) and shows a danger meter on each node. Outliers trigger a reroll of that encounter. The forecast is also an honest number for the group: "Boss: the scouts won 6 of 10".
- Real games feed back: every finished fight logs (encounter, heroes' decks, result, length), and a weekly report flags monsters whose real win rate differs from the forecast.

### 3.12 More original ideas (beyond the five in Part 1)

- **Every room is a place:** room cards as environments (Planechase without the die by default; Heat adds the die).
- **Hirelings:** a tavern node hires a bot ally with a precon and a personality for a floor; it votes "with the torch". Also covers absent friends.
- **Graves:** a fallen hero's grave appears as an optional node in a later delve of the same group; visiting it recovers one of their relics and plays a short fight against their "ghost deck" (their own last decklist, Inscryption deathcard style).
- **Party combos:** relics that only work together ("Twin Banners: if two heroes attacked this turn, each draws a card") make the group plan together.
- **Monster voices:** short authored lines on monster events in the chat panel; the boss taunts at phase changes. Cheap and memorable.
- **Daily rift:** a shared weekly seed with a leaderboard among friend groups on the server.
- **Initiative heists:** an objective room where the monsters hold the initiative and the party must take it and finish the Undercity first; reuses a mechanic we already run.
- **Bestiary:** defeated monsters fill a per-account bestiary with their decklists, a gentle collection meta that also teaches card knowledge.

### 3.13 Server flow

- New module `campaign/` in `mtg-server`: REST for create, join (invite code like games), choose hero deck, vote, take reward, buy, campfire, event choice; a small websocket channel (or the existing lobby socket) pushes `DelveState` to members. A delve has one actor like a game (low traffic; serialises votes and picks).
- Entering a fight room: the actor builds the `GameSetup` from the encounter and heroes (decks with run changes applied, relics as emblems, monsters as bot seats with level and personality, house rules: teams, handicaps, objectives), creates the game through `Registry::create_decked` with every hero's seat bound to their account (as tournaments do), and sends everyone to the table. When the game record finishes, the actor folds `FightEnded` and opens rewards (the tournament module already reads results from records; same pattern).
- Abandoning: the host can end a fight as a loss (retreat) to keep sessions short.

### 3.14 Web UI sketch

- **Campaign page** (`/delves`): my delves (status, floor, party, last played), "New delve" (theme, Heat, seed field or random, party size), join by code, the weekly rift.
- **Party lobby:** four hero slots (avatar, deck chooser reused from the game lobby, ready), bot filler toggle with level, burdens to take, the floor's boss card preview.
- **Map screen:** vertical scroll of the floor, rooms as icons on lanes, edges as paths; party token on the current node; each hero's vote shown as a coloured pip on the voted node; torch icon on the torch bearer; danger meter per node from the forecast; the boss card at the top. Right panel: party (hearts, gold, relics as chips reusing `PlayerEffectChips`), run log.
- **Reward screen:** three big card picks per hero in their colours plus "skip for gold"; shared draft row below with whose pick it is.
- **Shop, campfire, event:** single-card-width panels, one decision each, personal.
- **In fight:** the existing table; the boss card and Omen scheme pinned top centre; Spirit heroes shown translucent with their haunt hand; room card in the pods' effect strip.
- Follow the design rules of `docs/web.md`: fits the viewport, Arena-like flow, clear whose turn it is.

### 3.15 Risks

| Risk | Mitigation |
|---|---|
| Fights too long for 4 to 6 rooms an evening | Smaller life (15 to 20), small monster decks, scripted seats, shared team turns, objectives with turn limits, start heroes with a land preset at deeper floors; measure median turns in sims. |
| Bots too weak or too slow as bosses | Boss strength comes from its rule card and schemes, not from the brain; Hard brain only on the boss; monsters' bot delay shorter than heroes'. |
| Card coverage: a monster or reward card the engine runs only in part | Content uses Verified cards only (a content test checks support state). |
| Custom card frames look cheap | Use art crops from real printings plus a clean frame; few custom cards per run. |
| Difficulty swings from Magic variance (mana screw) | Mulligan smoothing (`opening_hand_smoothing`), the free mulligan, retry once per floor, hearts instead of carried life. |
| Alpha player | Personal rewards, personal campfire, votes only on paths; Spirit roles give each player agency. |
| Scope creep | The MVP has no schemes, no Spirits, no Pilgrim decks; those are phases 2 and 3. |
| Content authoring cost | A theme = ~8 monsters, 6 affixes, 1 boss, 10 schemes, 12 relics, 6 room cards, 6 events. Agents can draft content from existing Verified cards; the local model can propose deck slices. |

---

## Part 4. Phased plan (roadmap-style items, about 15 minutes each)

**Phase 1: playable slice (one floor, 4 rooms plus boss, party 1 to 4).** About 45 pieces.
- (c1) `mtg-campaign` crate: content types, validation test, one theme ("Warrens": goblins, rats, a troll boss) using Verified cards only. M (4)
- (c2) Map generator: layered DAG, kind rules, seeded; property tests (every path reaches the boss, kind rules hold, same seed same map). M (3)
- (c3) Budget and encounter builder to `GameSetup`: teams N vs 1 to 3, handicaps for life, Fight objective "defeat all". M (3)
- (c4) Engine G1 `PlayerSetup::emblems` and G2 stickered library cards; relics as emblems. S (2)
- (c5) Custom card records G3 (printed side from content) for relics and the boss card, fingerprinted. M (3)
- (c6) Boss v0: boss card as emblem with static rules and a life-threshold phase trigger; life scaling by party. M (2)
- (c7) Scripted seat brain G5 and personality `Weights` per seat with Fair targeting G6. M (3)
- (c8) Run state as a fold of run events; rewards, gold, hearts, swaps; deterministic loot stream. M (3)
- (c9) Server: `delves` tables and migrations, delve actor, REST, invite code, fight creation via `create_decked`, result read-back. L (6)
- (c10) Protocol types and bindings for `DelveState`, votes, rewards. S (2)
- (c11) Web: delves page, party lobby, map screen with votes and torch, reward and shop and campfire screens, boss pin in the table. L (10)
- (c12) `delve_sim` in testkit and a CLI to tune the one theme; first threat table. M (3)
- (c13) e2e: a 1-hero delve with bot allies from lobby to boss. S (1)

**Phase 2: real campaign.** About 40 pieces.
- Schemes G7 with the Omen; shared team turns G8; objectives G9.
- Elites and affixes; events with votes; treasure loot draft; burdens; Heat 1 to 5.
- Three floors per delve; save between evenings; hirelings and absent-friend bots.
- Custom card frame in the web G4; monster voices.
- Scouts' forecast in the background with reroll; weekly fight report.

**Phase 3: depth and replay.** About 35 pieces.
- Spirits G11; Nemesis memory and revenge rooms; graves.
- Pilgrim hero style (starter decks that grow), card removal.
- Two more themes; Heat 6 to 10; room chaos with the planar die G12; dormant minion seats G10.
- Weekly rift seed and group leaderboard; bestiary.

---

## Part 5. Open questions for the owner

1. **Decks:** start with Veteran (your Commander deck, rewards are swaps) as proposed, or go straight to Pilgrim (small starter decks that grow, more roguelike)?
2. **Format of fights:** Commander rules (command zone, commander damage) inside fights, or a lighter ruleset (20 life, no commander damage) for speed?
3. **Fight length target:** 15 to 20 minutes (more rooms, smaller fights) or 30 to 40 (fewer, bigger fights)?
4. **Death:** is a hero with no hearts left out until revived (hardcore) or always back as a Spirit (friendly)? Proposed: Spirit, with Shrine revival.
5. **Boss honesty:** may bosses have open advantages (double life, extra draws written on the boss card), or should they play only real cards?
6. **Custom cards:** are you happy with custom boss/relic cards that are not real Magic cards (with art borrowed from real printings), or should everything be real cards and real emblems only?
7. **Turns:** shared team turns for the party (Archenemy rule, faster, needs engine work) or separate turns (works today, slower)?
8. **Who builds content:** hand-authored themes only, or let the local model propose monster decks and relics for review?
9. **Progress outside a run** (unlocks, bestiary, Heat ladder): wanted, or keep each delve self-contained?
10. **Priority:** where does this sit against card coverage and the live game items in the roadmap?

---

## Sources

- Dice Throne Adventures: [Co-op Gestalt review](https://coopgestalt.com/2021/01/25/a-review-of-dice-throne-adventures-the-cooperative-expansion-for-dice-throne/), [rulebook (PDF)](https://www.gameontabletop.com/contenu/partners/luckyduck/file/Dice_Throne_Adventures_-_Rulebook_-_2020.08.12_LD.pdf), [solo review](https://www.unboxedtheboardgameblog.com/post/dice-throne-adventures-alone-against-the-mad-king), [TechRaptor guide](https://techraptor.net/tabletop/guides/dice-throne-dice-throne-adventures-introductory-guide)
- Forge quest and adventure: [Quest mode guide](https://www.slightlymagic.net/forum/viewtopic.php?p=106873), [CasualNation on adventure games](https://www.coolstuffinc.com/a/casualnation-11-adventure-ten-games); the Adventure mode details (overworld, towns, roaming enemies) are from general knowledge of Forge, not verified in this search.
- Challenge decks: [Face the Hydra (mtg.wiki)](https://mtg.wiki/page/Face_the_Hydra), [Battle the Horde](https://mtg.wiki/page/Battle_the_Horde), [Challenge Deck](https://mtg.wiki/page/Challenge_Deck)
- Archenemy and Planechase: [Archenemy (Wikipedia)](https://en.wikipedia.com/wiki/Magic:_The_Gathering_Archenemy), [Archenemy rules guide](https://gamertagmythras.com/blog/magic-the-gathering/mtg-archenemy-rules-guide), [Planechase (Wikipedia)](https://en.wikipedia.org/wiki/Planechase), [CR 901 Planechase](https://blogs.magicjudges.org/rules/?p=1541)
- Slay the Spire: [map generation guide](https://kosgames.com/slay-the-spire-map-generation-guide-26769/), [map generation and branching](https://www.ludo.guide/guide/slay-the-spire/pathing-risk-assessment/map-generation-and-branching), [stsmapgen](https://github.com/yurkth/stsmapgen)
- Gloomhaven: [scenario level](https://rules.dized.com/gloomhaven/rule/scenario-setup/scenario-level), [rulebook v1.1 summary](https://www.orderofgamers.com/downloads/Gloomhaven_v1.1.pdf)
- Path of Champions: [Riot guide](https://support-legendsofruneterra.riotgames.com/hc/articles/4410166424723), [Red Bull beginner guide](https://redbull.com/ca-en/legends-of-runeterra-the-path-of-champions-beginner-guide), [Mobalytics 2.0 tips](https://mobalytics.gg/blog/lor/lor-path-of-champions-2-0-tips-tricks/)
- Monster Train, Inscryption, Spirit Island, the dungeon/initiative mechanics and Alchemy: from general knowledge.
- Repo (read only): `docs/engine.md` (Shape, House rules, Teams, Stickers, Rules of the game), `docs/server.md` (Bot seats, Tournaments, Take-backs, Choosing a deck), `docs/cards-dsl.md`, `crates/mtg-engine/src/setup.rs` (`PlayerSetup`, `PresetPermanent`), `crates/mtg-testkit/src/brain/` (`Settings`, `Weights`), `crates/mtg-cards/src/effect.rs` (`CreateEmblem`, `TakeInitiative`), `crates/mtg-cards/dungeons/`, `crates/mtg-server/src/recipe.rs` (`GameRecipe`, conjured).

## Owner decisions (2026-11-13)
- Decks: each hero plays their own Commander deck (precons allowed); run rewards are card swaps/boons on top of it.
- Fights must be hard (too easy is not fun): about 10–15 min per fight is fine, longer is fine too. Tune monsters/bosses with bot simulations so a competent party wins roughly half to two thirds of fights at base Heat.
- Spirits: yes — a hero at 0 life keeps playing as a Spirit with haunt cards.
- Bosses may have open advantages (extra life, extra cards, extra turns phases, scheme decks).

## Owner decisions 2 (2026-11-13): Boss Rush instead of a room crawl
- MTG decks need turns to set up; many short fights reward only aggro. So the run is a BOSS RUSH: a series of big boss fights (each a full fresh game: board, hand, library reset), with loot/rewards between bosses. Optional small side events between bosses are fine, but no short skirmish fights.
- Life carries over between boss fights (with some healing between bosses); tokens and the board do NOT carry over.
- Team roles emerge from decks and loot: Healer (restores life, keeps the team going between fights), Removal (answers boss threats), Damage dealer (kills the boss), others possible. Loot can favour roles.
- Loot = run-long "artifacts" (relics, as emblems, not necessarily MTG artifact cards) with passive buffs. Normal: e.g. "The first blue card you cast each fight is copied", "Your red spells cost {1} less". Powerful but dangerous (risk/reward, the party decides together): e.g. "ALL red spells cost {2} less — including the boss's".
