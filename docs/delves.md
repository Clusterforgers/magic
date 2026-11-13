# Delves

Design of Delves, the co-op campaign: a party of 1 to 4 heroes fights a ladder of bosses with their own Commander decks.

## Shape

- A run is a boss rush. Its ladder (`mtg-campaign`, `map.rs`) holds the theme's lieutenants in an order drawn from the seed, then the final boss; between two bosses the party votes for one of two camps: Treasure, Shop, Event or Campfire.
- Every boss fight is a fresh, normally recorded Commander game: board, hand and library reset. The party is one team of seats from seat 0, the boss one seat after them (`FightPlan`).
- Life carries over between fights. After a won fight each hero keeps their end life and heals 10; a hero who ended it as a Spirit comes back at 10 first. A Campfire rest heals 20, the shop's draught 15; nothing heals above 40 plus relic bonuses.
- The party has 3 hearts. A lost fight costs one, every hero regroups at half life and the fight is retried; with no hearts left the run ends in defeat.
- Within a fight a hero at 0 life is a Spirit while a teammate lives (`docs/engine.md`, Campaign seats): their cards leave and the Restless Spirit emblem taps a boss creature each boss upkeep and drains the boss each of their end steps. The party loses when every hero has fallen.

## Bosses

- A boss (`content/<theme>.json`) has a 40-card deck of verified real cards, a brain (Hard by default), life `life + life_per_hero × (heroes − 1)`, an opening hand of 9, a boss card and two phases.
- Its advantages are open: the boss card is an emblem in its command zone from the start; each phase is an emblem that arrives when its life falls to a share of its starting life (66 % and 33 %). The table shows the boss's phase and the next one on the seat's chips.
- Heat raises the difficulty: Heat 1 gives bosses a fifth more life, Heat 2 a stronger brain.

## Loot

- After each lieutenant every hero gets 50 gold, a pick of one of three relics and a pick of one of three cards in their colours (skipping a card gives 10 gold). A new card replaces a basic land of the deck.
- Relics (`content/cards.json`) are emblems the hero keeps for the run, each with a role that shapes the party: Healer, Removal, Damage, Support, and Risky relics whose effect is symmetrical and helps the boss too (all red spells cost {2} less, every player draws an extra card, all creatures get +2/+0). Risky relics come only from loot and treasure, never the shop.
- Camps: Treasure offers a relic pick; the Shop sells three cards (35 gold), two relics and a draught; an Event offers each hero a choice with gold, life, relic or card outcomes and seeded gambles; a Campfire offers rest or a card pick.

## Run state

- A run is the fold of its events (`run.rs`): `Joined`, `Left`, `DeckChosen`, `Started`, `Voted`, `FightCreated`, `FightEnded`, `Picked`, `Abandoned`. The ladder is regenerated from the seed, never stored.
- Every random draw comes from the seed keyed by room, hero and purpose (`rng.rs`), so a reload never rerolls loot.
- Votes: when every hero voted, the room with the most votes wins and a tie goes to the torch bearer, a role that moves one hero on with every cleared room. Bot allies vote once the players did, with the majority, and answer offers with the first choice (rest when hurt, the first affordable relic at the shop).

## Content

- Custom cards are not on Scryfall: id, name, type line, rules text, art placeholder key, role and price, and their abilities in the card DSL. The engine plays each as an emblem; the web draws them from the content endpoint.
- A content test checks that every boss and reward card is Verified in the corpus, every boss deck has 40 cards and every custom card keeps all its abilities.
