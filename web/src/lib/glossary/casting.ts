import type { Entry } from './types'

/** Keyword abilities that change how a spell is cast, paid for, copied or cast from another zone. */
export const CASTING: readonly Entry[] = [
  [
    'Morph',
    'You may cast this face down as a 2/2 creature for {3}. Turn it face up any time for its morph cost.',
  ],
  ['Megamorph', 'Like morph, but when turned face up, put a +1/+1 counter on it.'],
  [
    'Disguise',
    'You may cast this face down as a 2/2 creature with ward {2} for {3}. Turn it face up any time for its disguise cost.',
  ],
  [
    'Myriad',
    'Whenever this creature attacks, for each opponent other than defending player, you may create a token copy tapped and attacking that player. Exile the tokens at end of combat.',
  ],
  [
    'Exert',
    'You may have this creature not untap during your next untap step as it attacks, for an extra effect.',
  ],
  ['Read ahead', 'Choose a chapter and start with that many lore counters. Skipped chapters do not trigger.'],
  [
    'Ninjutsu|Commander ninjutsu',
    'Pay the cost and return an unblocked attacker you control to hand: put this card from your hand onto the battlefield tapped and attacking.',
  ],
  [
    'Offering',
    "You may cast this card any time you could cast an instant by sacrificing a permanent of the stated type and reducing its cost by that permanent's mana value.",
  ],
  [
    'Convoke',
    "Your creatures can help cast this spell. Each creature you tap while casting it pays for {1} or one mana of that creature's color.",
  ],
  ['Delve', 'Each card you exile from your graveyard while casting this spell pays for {1}.'],
  ['Improvise', 'Your artifacts can help cast this spell. Each artifact you tap pays for {1}.'],
  ['Affinity', 'This spell costs {1} less to cast for each permanent of the stated kind you control.'],
  ['Undaunted', 'This spell costs {1} less to cast for each opponent.'],
  [
    'Emerge',
    "You may cast this spell by sacrificing a creature and paying the emerge cost reduced by that creature's mana value.",
  ],
  [
    'Spectacle',
    'You may cast this spell for its spectacle cost rather than its mana cost if an opponent lost life this turn.',
  ],
  [
    'Surge',
    'You may cast this spell for its surge cost if you or a teammate has cast another spell this turn.',
  ],
  ['Assist', 'Another player can pay up to all of the generic mana of this spell.'],
  ['Bargain', 'You may sacrifice an artifact, enchantment or token as you cast this spell.'],
  [
    'Casualty',
    'As you cast this spell, you may sacrifice a creature with power N or greater. When you do, copy this spell.',
  ],
  [
    'Gift',
    'You may promise an opponent a gift as you cast this spell. If you do, they get it before its other effects.',
  ],
  [
    'Offspring',
    'You may pay an extra cost as you cast this spell. If you do, when it enters, create a 1/1 token copy of it.',
  ],
  [
    'Squad',
    'As an additional cost to cast this spell, you may pay the squad cost any number of times. When it enters, create that many token copies of it.',
  ],
  [
    'Replicate',
    'When you cast this spell, copy it for each time you paid its replicate cost. You may choose new targets for the copies.',
  ],
  [
    'Storm',
    'When you cast this spell, copy it for each spell cast before it this turn. You may choose new targets for the copies.',
  ],
  [
    'Gravestorm',
    'When you cast this spell, copy it for each permanent put into a graveyard from the battlefield this turn.',
  ],
  [
    'Cascade',
    'When you cast this spell, exile cards from the top of your library until you exile a nonland card that costs less. You may cast it without paying its mana cost. Put the exiled cards on the bottom in a random order.',
  ],
  [
    'Kicker|Multikicker',
    'You may pay an additional cost as you cast this spell, once (kicker) or any number of times (multikicker).',
  ],
  [
    'Buyback',
    'You may pay an additional cost as you cast this spell. If you do, put it into your hand as it resolves instead of into your graveyard.',
  ],
  ['Entwine', 'Choose both modes of this spell if you pay the entwine cost.'],
  ['Escalate', 'Pay the escalate cost for each mode chosen beyond the first.'],
  ['Spree', 'Choose one or more modes and pay the additional cost of each when you cast this spell.'],
  ['Tiered', 'Choose one as you cast this spell; each choice has its own additional cost.'],
  [
    'Overload',
    'You may cast this spell for its overload cost. If you do, change its text by replacing all instances of "target" with "each".',
  ],
  [
    'Cleave',
    'You may cast this spell for its cleave cost. If you do, remove the words in square brackets from its text.',
  ],
  ['Fuse', 'You may cast both halves of this split card from your hand.'],
  ['Aftermath', 'Cast this half only from your graveyard. Then exile it.'],
  ['Flashback', 'You may cast this card from your graveyard for its flashback cost. Then exile it.'],
  [
    'Jump-start',
    'You may cast this card from your graveyard by discarding a card in addition to its other costs. Then exile it.',
  ],
  [
    'Retrace',
    'You may cast this card from your graveyard by discarding a land card in addition to its other costs.',
  ],
  [
    'Escape',
    'You may cast this card from your graveyard for its escape cost, exiling the stated number of other cards from it.',
  ],
  [
    'Harmonize',
    'You may cast this card from your graveyard for its harmonize cost. You may tap a creature you control to reduce that cost by its power. Then exile this card.',
  ],
  [
    'Disturb',
    'You may cast this card transformed from your graveyard for its disturb cost. If it would go to a graveyard, exile it instead.',
  ],
  [
    'Madness',
    'If you discard this card, discard it into exile. When you do, cast it for its madness cost or put it into your graveyard.',
  ],
  [
    'Miracle',
    'You may cast this card for its miracle cost when you draw it, if it is the first card you drew this turn.',
  ],
  [
    'Dredge',
    'If you would draw a card, you may mill N cards instead. If you do, return this card from your graveyard to your hand.',
  ],
  [
    'Cycling|Typecycling|Plainscycling|Islandcycling|Swampcycling|Mountaincycling|Forestcycling|Landcycling|Basic landcycling|Slivercycling|Wizardcycling|Halflingcycling|Affinitycycling',
    'Pay the cost and discard this card: draw a card, or for a typecycling variant search your library for a card of that type, reveal it and put it into your hand.',
  ],
  [
    'Suspend',
    'Rather than cast this card from your hand, pay its suspend cost and exile it with N time counters. At the beginning of your upkeep remove one; when the last is removed, cast it without paying its mana cost.',
  ],
  [
    'Foretell',
    'During your turn, you may pay {2} and exile this card from your hand face down. Cast it on a later turn for its foretell cost.',
  ],
  [
    'Plot',
    'You may pay the plot cost and exile this card from your hand. Cast it as a sorcery on a later turn without paying its mana cost.',
  ],
  [
    'Rebound',
    'If you cast this spell from your hand, exile it as it resolves. At the beginning of your next upkeep, you may cast it from exile without paying its mana cost.',
  ],
  ['Evoke', 'You may cast this spell for its evoke cost. If you do, it is sacrificed when it enters.'],
  [
    'Dash',
    'You may cast this spell for its dash cost. If you do, it gains haste and returns to your hand at the beginning of the next end step.',
  ],
  [
    'Blitz',
    'You may cast this spell for its blitz cost. If you do, it gains haste and "When this creature dies, draw a card" and is sacrificed at the beginning of the next end step.',
  ],
  [
    'Mayhem',
    'You may cast this card from your graveyard for its mayhem cost if you discarded it this turn. Exile it if it would leave the stack.',
  ],
  [
    'Prowl',
    'You may cast this spell for its prowl cost if you dealt combat damage to a player this turn with a creature that shares a type with it.',
  ],
  [
    'Unearth',
    'Pay the unearth cost: return this card from your graveyard to the battlefield. It gains haste. Exile it at the beginning of the next end step or if it would leave.',
  ],
  [
    'Encore',
    'Pay the encore cost and exile this card from your graveyard: for each opponent, create a token copy that attacks that player this turn if able. Sacrifice them at the next end step. Encore only as a sorcery.',
  ],
  [
    'Embalm',
    'Pay the embalm cost and exile this card from your graveyard: create a token copy, except it is a white Zombie. Embalm only as a sorcery.',
  ],
  [
    'Eternalize',
    'Pay the eternalize cost and exile this card from your graveyard: create a 4/4 black Zombie token copy. Eternalize only as a sorcery.',
  ],
  [
    'Scavenge',
    "Pay the scavenge cost and exile this card from your graveyard: put +1/+1 counters on target creature equal to this card's power. Scavenge only as a sorcery.",
  ],
  [
    'Transmute',
    'Pay the cost and discard this card: search your library for a card with the same mana value, reveal it, put it into your hand, then shuffle. Transmute only as a sorcery.',
  ],
  [
    'Awaken',
    'If you cast this spell for its awaken cost, also put N +1/+1 counters on target land you control and it becomes a 0/0 Elemental creature with haste.',
  ],
  [
    'Demonstrate',
    'When you cast this spell, you may copy it. If you do, choose an opponent to also copy it.',
  ],
  [
    'Renew',
    'Pay the cost and exile this card from your graveyard to get its stated effect. Renew only as a sorcery.',
  ],
  [
    'Sneak',
    'You may cast this spell for its sneak cost by returning an unblocked attacker you control to hand; it enters tapped and attacking.',
  ],
  [
    'Web-slinging',
    "You may cast this spell for its web-slinging cost by returning a tapped creature you control to its owner's hand.",
  ],
  [
    'Freerunning',
    'You may cast this spell for its freerunning cost if an Assassin or commander you controlled dealt combat damage to a player this turn.',
  ],
  [
    'Warp',
    'You may cast this from your hand for its warp cost. Exile it at the beginning of the next end step, then you may cast it from exile on a later turn.',
  ],
  ['Exhaust', 'Activate each exhaust ability only once.'],
  ['Boast', 'Activate only if this creature attacked this turn and only once each turn.'],
  [
    'Forecast',
    'Activate only during your upkeep and only once each turn, by revealing this card from your hand.',
  ],
  ['Channel', 'Pay the cost and discard this card from your hand to get its stated effect.'],
  [
    'Ripple',
    'When you cast this spell, you may reveal the top N cards of your library. You may cast any revealed cards with the same name without paying their mana costs.',
  ],
  [
    'Splice',
    'As you cast an Arcane spell, you may reveal this card from your hand and pay its splice cost to add its text to that spell.',
  ],
  [
    'Recover',
    'When a creature is put into your graveyard from the battlefield, you may pay the recover cost. If you do, return this card from your graveyard to your hand; otherwise exile it.',
  ],
  [
    'Transfigure',
    'Pay the cost and sacrifice this creature: search your library for a creature with the same mana value, put it onto the battlefield, then shuffle. Transfigure only as a sorcery.',
  ],
  ['Reinforce', 'Pay the cost and discard this card: put N +1/+1 counters on target creature.'],
  [
    'Soulshift',
    'When this creature dies, you may return target Spirit card with mana value N or less from your graveyard to your hand.',
  ],
  [
    'Epic',
    'For the rest of the game you can not cast spells. At the beginning of each of your upkeeps, copy this spell except for its epic ability.',
  ],
  [
    'Haunt',
    'When this creature dies or this spell is put into a graveyard, exile it haunting target creature. When the haunted creature dies, get the haunt effect.',
  ],
  ['Aura Swap', 'Pay the aura swap cost and exchange this Aura with an Aura card in your hand.'],
  ['More Than Meets the Eye', 'You may cast this card converted for its alternative cost.'],
  ['Craft', 'Pay the craft cost and exile this and the stated materials: return this transformed.'],
  ['Sunburst', 'This enters with a +1/+1 or charge counter on it for each color of mana spent to cast it.'],
  ['Bloodthirst', 'If an opponent was dealt damage this turn, this creature enters with N +1/+1 counters.'],
  [
    'Devour',
    'As this enters, you may sacrifice any number of creatures. It enters with N +1/+1 counters for each creature sacrificed.',
  ],
  [
    'Ingest',
    'Whenever this creature deals combat damage to a player, that player exiles the top card of their library.',
  ],
]
