import type { Entry } from './types'

/** Evergreen and combat keyword abilities, with their reminder text. */
export const COMBAT: readonly Entry[] = [
  ['Flash', 'You may cast this spell any time you could cast an instant.'],
  ['Flying', 'This creature can be blocked only by creatures with flying or reach.'],
  ['Reach', 'This creature can block creatures with flying.'],
  ['First strike', 'This creature deals combat damage before creatures without first strike.'],
  ['Double strike', 'This creature deals both first-strike and regular combat damage.'],
  ['Deathtouch', 'Any amount of damage this deals to a creature is enough to destroy it.'],
  ['Trample', 'This creature can deal excess combat damage to the player or planeswalker it is attacking.'],
  ['Lifelink', 'Damage dealt by this source also causes its controller to gain that much life.'],
  ['Vigilance', 'Attacking does not cause this creature to tap.'],
  ['Haste', 'This creature can attack and use {T} abilities as soon as it comes under your control.'],
  ['Menace', 'This creature can be blocked only by two or more creatures.'],
  ['Defender', 'This creature can not attack.'],
  ['Indestructible', 'Damage and effects that say "destroy" do not destroy this permanent.'],
  [
    'Hexproof',
    'This permanent or player can not be the target of spells or abilities your opponents control.',
  ],
  [
    'Hexproof from',
    'This permanent can not be the target of spells or abilities from sources of the stated quality.',
  ],
  ['Shroud', 'This permanent or player can not be the target of spells or abilities.'],
  [
    'Protection',
    'Can not be damaged, enchanted, equipped, blocked or targeted by anything of the stated quality.',
  ],
  ['Prowess', 'Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.'],
  [
    'Ward',
    'Whenever this permanent becomes the target of a spell or ability an opponent controls, counter it unless that player pays the ward cost.',
  ],
  ['Fear', 'This creature can be blocked only by artifact creatures and/or black creatures.'],
  [
    'Intimidate',
    'This creature can be blocked only by artifact creatures and/or creatures that share a color with it.',
  ],
  [
    'Landwalk|Islandwalk|Swampwalk|Mountainwalk|Forestwalk|Plainswalk|Desertwalk|Legendary landwalk|Nonbasic landwalk',
    'This creature can not be blocked as long as defending player controls a land of the stated type.',
  ],
  ['Skulk', 'This creature can not be blocked by creatures with greater power.'],
  ['Shadow', 'This creature can block or be blocked by only creatures with shadow.'],
  ['Horsemanship', 'This creature can be blocked only by creatures with horsemanship.'],
  [
    'Flanking',
    'Whenever a creature without flanking blocks this creature, the blocking creature gets -1/-1 until end of turn.',
  ],
  [
    'Banding',
    'Creatures with banding attack or block together, and their controller divides the damage dealt to them.',
  ],
  ['Bushido', 'Whenever this creature blocks or becomes blocked, it gets +N/+N until end of turn.'],
  [
    'Rampage',
    'Whenever this creature becomes blocked, it gets +N/+N for each creature beyond the first blocking it.',
  ],
  [
    'Provoke',
    'Whenever this creature attacks, you may have target creature defending player controls untap and block it if able.',
  ],
  [
    'Battle cry',
    'Whenever this creature attacks, each other attacking creature gets +1/+0 until end of turn.',
  ],
  ['Exalted', 'Whenever a creature you control attacks alone, that creature gets +1/+1 until end of turn.'],
  [
    'Melee',
    'Whenever this creature attacks, it gets +1/+1 until end of turn for each opponent you attacked this combat.',
  ],
  [
    'Mentor',
    'Whenever this creature attacks, put a +1/+1 counter on target attacking creature with lesser power.',
  ],
  [
    'Dethrone',
    'Whenever this creature attacks the player with the most life or tied for most life, put a +1/+1 counter on it.',
  ],
  ['Annihilator', 'Whenever this creature attacks, defending player sacrifices N permanents.'],
  [
    'Extort',
    'Whenever you cast a spell, you may pay {W/B}. If you do, each opponent loses 1 life and you gain that much life.',
  ],
  [
    'Enlist',
    'As this creature attacks, you may tap a nonattacking creature you control without summoning sickness. When you do, add its power to this creature until end of turn.',
  ],
  [
    'Training',
    'Whenever this creature attacks with another creature with greater power, put a +1/+1 counter on this creature.',
  ],
  ['Afflict', 'Whenever this creature becomes blocked, defending player loses N life.'],
  [
    'Infect',
    'This creature deals damage to creatures in the form of -1/-1 counters and to players in the form of poison counters.',
  ],
  ['Wither', 'This deals damage to creatures in the form of -1/-1 counters.'],
  ['Toxic', 'Players dealt combat damage by this creature also get N poison counters.'],
  [
    'Poisonous',
    'Whenever this creature deals combat damage to a player, that player gets N poison counters.',
  ],
  [
    'Double team',
    'When this creature attacks, if it is not a token, create a copy of it that enters tapped and attacking, then both lose double team.',
  ],
  ['Decayed', 'This creature can not block. When it attacks, sacrifice it at end of combat.'],
  ['Changeling', 'This card is every creature type.'],
  ['Devoid', 'This card has no color.'],
  [
    'Split second',
    'As long as this spell is on the stack, players can not cast spells or activate abilities that are not mana abilities.',
  ],
  [
    'Mobilize',
    'Whenever this creature attacks, create N tapped and attacking 1/1 red Warrior tokens. Sacrifice them at the beginning of the next end step.',
  ],
  [
    'Phasing',
    "This permanent phases in or out on its controller's untap step. While phased out, it is treated as though it does not exist.",
  ],
  [
    'Daybound|Nightbound',
    'It becomes day as the first daybound permanent enters. A daybound permanent transforms when it becomes night, and back when it becomes day.',
  ],
  [
    'Soulbond',
    'You may pair this creature with another unpaired creature when either enters. They share abilities as long as they stay paired.',
  ],
  [
    'Umbra armor',
    'If enchanted permanent would be destroyed, instead remove all damage from it and destroy this Aura.',
  ],
  ['Ascend', "If you control ten or more permanents, you get the city's blessing for the rest of the game."],
  ['Max speed', 'With speed 4, you have max speed and this ability is active.'],
  [
    'Start your engines!',
    'If you have no speed, it becomes 1. It increases once each turn when an opponent loses life during your turn, to a maximum of 4.',
  ],
]
