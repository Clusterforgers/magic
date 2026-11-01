import type { Entry } from './types'

/** Keyword actions (CR 701) and the other named things a card text does. */
export const ACTIONS: readonly Entry[] = [
  [
    'Cloak',
    'Put a card face down as a 2/2 creature with ward {2}. Turn it up for its mana cost if it is a creature card.',
  ],
  [
    'Conspire',
    'As you cast this spell, you may tap two untapped creatures you control that share a color with it. When you do, copy it.',
  ],
  ['Activate', 'Put an activated ability on the stack and pay its costs.'],
  ['Attach', 'Move an Aura, Equipment or Fortification onto an object or player.'],
  ['Cast', 'Put a spell on the stack and pay its costs.'],
  ['Counter', 'Cancel a spell or ability on the stack; it does not resolve and it leaves the stack.'],
  ['Create', 'Put a token onto the battlefield.'],
  ['Destroy', "Move a permanent from the battlefield to its owner's graveyard."],
  ['Discard', 'Move a card from your hand to your graveyard.'],
  ['Exchange', 'Swap two things, such as life totals, control of permanents or cards in zones.'],
  ['Exile', 'Put an object into the exile zone.'],
  ['Play', 'Play a land or cast a spell.'],
  ['Reveal', 'Show a card to all players.'],
  [
    'Sacrifice',
    "Move a permanent you control to its owner's graveyard; it can not be regenerated or responded to by indestructible.",
  ],
  ['Shuffle', 'Randomize the order of a library.'],
  ['Tap|Untap', 'Turn a permanent sideways (tapped) or upright (untapped).'],
  [
    'Scry',
    'Look at the top N cards of your library, then put any on the bottom and the rest on top in any order.',
  ],
  [
    'Surveil',
    'Look at the top N cards of your library, then put any into your graveyard and the rest on top in any order.',
  ],
  ['Mill', 'Put the top N cards of your library into your graveyard.'],
  ['Seek', 'Put a random card with the stated quality from your library into your hand.'],
  [
    'Discover',
    'Exile cards from the top of your library until you exile a nonland card with mana value N or less. Cast it without paying its mana cost or put it into your hand. Put the rest on the bottom in a random order.',
  ],
  [
    'Explore',
    'Reveal the top card of your library. Put it into your hand if it is a land; otherwise put a +1/+1 counter on this creature, then you may put the card into your graveyard.',
  ],
  [
    'Connive',
    'Draw a card, then discard a card. If you discarded a nonland card, put a +1/+1 counter on this creature.',
  ],
  [
    'Learn',
    'You may reveal a Lesson card you own from outside the game and put it into your hand, or discard a card to draw a card.',
  ],
  ['Investigate', 'Create a Clue token. It is an artifact with "{2}, Sacrifice this artifact: Draw a card."'],
  [
    'Treasure',
    'A Treasure token is an artifact with "{T}, Sacrifice this artifact: Add one mana of any color."',
  ],
  ['Food', 'A Food token is an artifact with "{2}, {T}, Sacrifice this artifact: You gain 3 life."'],
  [
    'Blood',
    'A Blood token is an artifact with "{1}, {T}, Discard a card, Sacrifice this artifact: Draw a card."',
  ],
  [
    'Map',
    'A Map token is an artifact with "{1}, {T}, Sacrifice this artifact: Target creature you control explores. Activate only as a sorcery."',
  ],
  [
    'Role token',
    'A Role is an Aura token attached to a creature that gives it an ability. A permanent can have only one Role of each controller.',
  ],
  ['Populate', 'Create a token that is a copy of a creature token you control.'],
  [
    'Proliferate',
    'Choose any number of permanents and/or players with counters, then give each another counter of each kind already there.',
  ],
  ['Monstrosity', "If this creature isn't monstrous, put N +1/+1 counters on it and it becomes monstrous."],
  ['Adapt', 'If this creature has no +1/+1 counters on it, put N +1/+1 counters on it.'],
  ['Support', 'Put a +1/+1 counter on each of up to N target creatures.'],
  [
    'Bolster',
    'Choose a creature with the least toughness among creatures you control and put N +1/+1 counters on it.',
  ],
  [
    'Amass',
    "Put N +1/+1 counters on an Army you control. If you don't control one, create a 0/0 black Army creature token first.",
  ],
  [
    'Fateseal',
    "Look at the top N cards of an opponent's library, then put any on the bottom and the rest on top in any order.",
  ],
  [
    'Manifest',
    'Put the top card of your library onto the battlefield face down as a 2/2 creature. Turn it up any time for its mana cost if it is a creature card.',
  ],
  [
    'Manifest dread',
    'Look at the top two cards of your library. Manifest one and put the other into your graveyard.',
  ],
  [
    'Incubate',
    'Create an Incubator token with N +1/+1 counters on it and "{2}: Transform this artifact." It transforms into a 0/0 Phyrexian artifact creature.',
  ],
  [
    'Detain',
    'Until your next turn, that permanent can not attack or block and its activated abilities can not be activated.',
  ],
  [
    'Goad',
    'Until your next turn, that creature attacks each combat if able and attacks a player other than you if able.',
  ],
  ['Fight', 'Each of two creatures deals damage equal to its power to the other.'],
  [
    'Clash',
    'Each player reveals the top card of their library, then puts it on the top or bottom. A player wins if their card has the higher mana value.',
  ],
  ['Meld', 'Exile both melded cards and return them to the battlefield transformed as one permanent.'],
  ['Transform', 'Turn a double-faced permanent over to its other face.'],
  [
    'Regenerate',
    'The next time the creature would be destroyed this turn, instead tap it, remove it from combat and remove all damage from it.',
  ],
  [
    'Suspect',
    'A suspected creature has menace and can not block. It stays suspected until something says otherwise.',
  ],
  ['Collect evidence', 'Exile cards with total mana value N or more from your graveyard.'],
  ['Forage', 'Exile three cards from your graveyard or sacrifice a Food.'],
  [
    'Endure',
    "Put N +1/+1 counters on this creature, or if you don't, create an N/N white Spirit creature token.",
  ],
  [
    'Heist',
    "Look at three random nonland cards in an opponent's library, exile one face down, and you may cast it, spending mana as any type.",
  ],
  [
    'Time Travel',
    'For each suspended card you own and each permanent you control with a time counter, you may add or remove a time counter.',
  ],
  ['Vote', 'Each player chooses one of the options; the choices are counted when everyone has voted.'],
  [
    'Venture into the dungeon',
    "Move your venture marker down to the next room of a dungeon, starting one if you have none, and get the room's effect.",
  ],
  ['Open an Attraction', 'Put the top card of your Attraction deck onto the battlefield.'],
  ['Roll to Visit Your Attractions', 'Roll a die; each Attraction you control with that number visits.'],
  ['Planeswalk', 'Move to a new plane card in the planar deck.'],
  ['Set in motion', 'Turn a scheme card face up and put it into effect.'],
  ['Abandon', "Put an ongoing scheme card face up on the bottom of its owner's scheme deck."],
  ['Assemble', 'Put the top card of your Contraption deck onto the battlefield.'],
  ['Double|Triple', 'Double or triple the stated quantity: power, life, counters or mana.'],
]
