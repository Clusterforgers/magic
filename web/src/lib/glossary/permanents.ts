import type { Entry } from './types'

/** Keyword abilities of permanents: how they enter, attach, grow, leave and come back, and the commander words. */
export const PERMANENTS: readonly Entry[] = [
  [
    'Unleash',
    'You may have this creature enter with a +1/+1 counter. It can not block as long as it has a +1/+1 counter.',
  ],
  ['Tribute', 'As this creature enters, an opponent of your choice may put N +1/+1 counters on it.'],
  [
    'Evolve',
    'Whenever a creature you control enters with greater power or toughness, put a +1/+1 counter on this creature.',
  ],
  ['Exploit', 'When this creature enters, you may sacrifice a creature.'],
  [
    'Renown',
    'When this creature deals combat damage to a player, if it is not renowned, put N +1/+1 counters on it and it becomes renowned.',
  ],
  ['Riot', 'This creature enters with your choice of a +1/+1 counter or haste.'],
  ['Ravenous', 'This creature enters with X +1/+1 counters. If X is 5 or more, draw a card when it enters.'],
  [
    'Backup',
    "When this creature enters, put N +1/+1 counters on target creature. If that is another creature, it gains this creature's abilities until end of turn.",
  ],
  ['Afterlife', 'When this permanent dies, create N 1/1 white and black Spirit creature tokens with flying.'],
  [
    'Undying',
    'When this creature dies, if it had no +1/+1 counters on it, return it to the battlefield with a +1/+1 counter on it.',
  ],
  [
    'Persist',
    'When this creature dies, if it had no -1/-1 counters on it, return it to the battlefield with a -1/-1 counter on it.',
  ],
  [
    'Fabricate',
    'When this permanent enters, put N +1/+1 counters on it or create N 1/1 colorless Servo artifact creature tokens.',
  ],
  [
    'Amplify',
    'As this creature enters, reveal any number of cards sharing a creature type with it from your hand. It enters with N +1/+1 counters for each.',
  ],
  ['Absorb', 'If a source would deal damage to this creature, prevent N of that damage.'],
  [
    'Modular',
    'This permanent enters with N +1/+1 counters. When it dies, you may put its +1/+1 counters on target artifact creature.',
  ],
  ['Frenzy', 'Whenever this creature attacks and is not blocked, it gets +N/+0 until end of turn.'],
  [
    'Mutate',
    'You may cast this for its mutate cost, putting it over or under a non-Human creature you own. They mutate into one creature with all abilities.',
  ],
  [
    'Graft',
    'This enters with N +1/+1 counters. Whenever another creature enters, you may move a +1/+1 counter from this onto it.',
  ],
  [
    'Hideaway',
    'This permanent enters tapped. When it does, look at the top N cards of your library, exile one face down, then put the rest on the bottom.',
  ],
  ['Level up', 'Pay the level up cost to put a level counter on this. Level up only as a sorcery.'],
  ['Outlast', 'Pay the cost and tap: put a +1/+1 counter on this creature. Outlast only as a sorcery.'],
  [
    'Reconfigure',
    'Attach to target creature you control for the cost, or unattach it. It is not a creature while attached.',
  ],
  [
    'Living weapon',
    'When this Equipment enters, create a 0/0 black Phyrexian Germ token, then attach this to it.',
  ],
  ['Equip|Equip to', 'Attach to target creature you control for the cost. Equip only as a sorcery.'],
  ['Fortify', 'Attach to target land you control for the cost. Fortify only as a sorcery.'],
  [
    'Enchant|Enchant player',
    'Cast this Aura targeting the stated object or player; it stays attached to it.',
  ],
  [
    'Bestow',
    'You may cast this card for its bestow cost as an Aura that gives +N/+N or abilities; it becomes a creature if it becomes unattached.',
  ],
  [
    'Crew',
    'Tap any number of untapped creatures you control with total power N or more: this Vehicle becomes an artifact creature until end of turn.',
  ],
  [
    'Saddle|Saddled',
    'Tap any number of other untapped creatures you control with total power N or more: this Mount becomes saddled until end of turn. Saddle only as a sorcery.',
  ],
  [
    'Station',
    "Tap another untapped creature you control: put charge counters on this Spacecraft equal to that creature's power. Station only as a sorcery.",
  ],
  ['Living metal', 'As long as it is your turn, this Vehicle is also a creature.'],
  [
    'Compleated',
    'This planeswalker enters with two fewer loyalty counters for each Phyrexian mana paid with life.',
  ],
  [
    'For Mirrodin!',
    'When this Equipment enters, create a 2/2 red Rebel creature token, then attach this to it.',
  ],
  [
    'Job select',
    'When this Equipment enters, create a 1/1 colorless Hero creature token, then attach this to it.',
  ],
  [
    'Specialize',
    'Pay the cost and discard a card to turn this face up as its specialized version of the stated color.',
  ],
  [
    'Cipher',
    'After resolving, you may exile this spell encoded on a creature you control. Whenever that creature deals combat damage to a player, cast a copy of the card.',
  ],
  [
    'Champion',
    'When this enters, sacrifice it unless you exile another creature you control of the stated type. When it leaves, return the exiled card.',
  ],
  ['Augment', 'Reveal this card from your hand and pay its cost to combine it with a non-Human creature.'],
  [
    'Increment',
    "Whenever you cast a spell, if the mana spent is greater than this creature's power or toughness, put a +1/+1 counter on it.",
  ],
  [
    'Echo',
    'At the beginning of your upkeep, if this came under your control since your last upkeep, sacrifice it unless you pay its echo cost.',
  ],
  [
    'Cumulative upkeep',
    'At the beginning of your upkeep, put an age counter on this permanent, then sacrifice it unless you pay its upkeep cost for each age counter.',
  ],
  [
    'Vanishing',
    'This permanent enters with N time counters. At your upkeep remove one; when the last is removed, sacrifice it.',
  ],
  [
    'Fading',
    'This permanent enters with N fade counters. At your upkeep remove one or sacrifice it if you can not.',
  ],
  [
    'Impending',
    'If you cast this for its impending cost, it enters with N time counters and is not a creature until the last is removed.',
  ],
  [
    'Prototype',
    'You may cast this spell with a different mana cost, color and size. It keeps its abilities and types.',
  ],
  [
    'Hidden agenda|Double agenda',
    'Name a card as the game starts, hidden from the other players; reveal it when instructed.',
  ],
  [
    'Companion',
    'If your deck meets the stated condition, you may put this card from your sideboard into your hand once, for {3}.',
  ],
  ['Partner', 'You can have two commanders if both have partner.'],
  [
    'Partner with',
    'You can have two commanders if both are the named pair. When this enters, target player may put the named card into their hand from their library, then shuffle.',
  ],
  ['Partner group|Friends forever', 'You can have two commanders if both have this ability.'],
  [
    "Doctor's companion",
    'You can have two commanders if the other is a Doctor with no other partner abilities.',
  ],
  ['Choose a Background', 'You can have a Background as a second commander.'],
  ['Eerie', 'Triggers whenever an enchantment you control enters and whenever you fully unlock a Room.'],
  [
    'Solved',
    'A Case is solved when its "to solve" condition is true at the beginning of your end step; then it gets its solved abilities.',
  ],
  [
    'Commander tax',
    'A commander costs {2} more to cast from the command zone for each time it was cast from there before.',
  ],
  ['Commander damage', 'A player who has taken 21 combat damage from one commander loses the game.'],
  [
    'Partner commander',
    'Two commanders share one command zone; each has its own tax and its own commander damage.',
  ],
]
