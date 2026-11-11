import type { CardType, CounterKind, Layer, Step, Supertype, Zone } from '@/protocol/generated'
import type { Entry } from './types'

/** The name of every zone of the engine's `Zone`, with or without an owner. */
type ZoneName<Z = Zone> = Z extends string ? Z : keyof Z

// Each table is keyed by an engine enum, so a step, zone, type or layer the engine adds fails the typecheck until it has words.

/** Every step and main phase of the engine's turn. */
export const STEPS: Record<Step, Entry> = {
  Untap: [
    'Untap step',
    'The first step of a turn: the active player untaps their permanents. No player gets priority.',
  ],
  Upkeep: [
    'Upkeep|Upkeep step',
    'The step after untap, where "at the beginning of your upkeep" abilities trigger; players get priority.',
  ],
  Draw: [
    'Draw step',
    'The active player draws a card, then players get priority. The player who goes first in a two-player game skips the first draw.',
  ],
  Main1: [
    'Main phase|Precombat main phase|First main phase',
    'You may play one land a turn and cast spells of any type while the stack is empty and you have priority.',
  ],
  BeginCombat: [
    'Beginning of combat|Beginning of combat step',
    'The step that opens combat: the last chance to act before attackers are chosen.',
  ],
  DeclareAttackers: [
    'Declare attackers step',
    'The active player chooses which untapped creatures attack and whom; attacking taps them unless they have vigilance.',
  ],
  DeclareBlockers: [
    'Declare blockers step',
    'Each defending player chooses which of their untapped creatures block which attackers.',
  ],
  FirstStrikeDamage: [
    'First-strike damage step',
    'An extra damage step when a creature in combat has first strike or double strike; those creatures deal their damage here, the rest after.',
  ],
  CombatDamage: [
    'Combat damage step',
    'Attacking and blocking creatures deal their combat damage at the same time; an unblocked attacker damages the player, planeswalker or battle it attacks.',
  ],
  EndCombat: [
    'End of combat step',
    'The step that closes combat: "at end of combat" abilities trigger, then creatures stop attacking and blocking.',
  ],
  Main2: [
    'Postcombat main phase|Second main phase',
    'The main phase after combat: a land if none was played this turn, and spells at sorcery speed.',
  ],
  End: [
    'End step',
    'The step where "at the beginning of the end step" abilities trigger; players get priority.',
  ],
  Cleanup: [
    'Cleanup step',
    'The active player discards down to the maximum hand size, damage is removed and "until end of turn" effects end. Players normally get no priority.',
  ],
}

/** Every zone of the engine. */
export const ZONES: Record<ZoneName, Entry> = {
  Library: [
    'Library',
    "A player's deck, face down and in a random order. A player who must draw from an empty library loses the game.",
  ],
  Hand: [
    'Hand',
    'The cards a player holds, hidden from the other players. The maximum hand size is seven at the cleanup step.',
  ],
  Battlefield: [
    'Battlefield',
    'The shared zone where lands, creatures, artifacts, enchantments, planeswalkers and battles stay as permanents.',
  ],
  Graveyard: [
    'Graveyard',
    "A player's discard pile, face up and in order: destroyed, sacrificed, discarded and resolved cards go there.",
  ],
  Stack: [
    'Stack|The stack',
    'Where spells and abilities wait to resolve; the last one put on resolves first, once every player passes in a row.',
  ],
  Exile: [
    'Exile zone',
    'A zone for cards removed from the game, usually face up; some effects let them come back or be played.',
  ],
  Command: [
    'Command zone',
    'Where commanders, emblems, dungeons and other game objects outside the battlefield wait; a commander is cast from here.',
  ],
  PhasedOut: [
    'Phased out',
    "A permanent that phased out is treated as though it does not exist until it phases in at its controller's untap step.",
  ],
}

/** Every card type of the engine. */
export const CARD_TYPES: Record<CardType, Entry> = {
  Artifact: [
    'Artifact',
    'A permanent type for objects and devices; colorless unless its cost or an effect says otherwise.',
  ],
  Battle: [
    'Battle',
    'A permanent with defense counters that an opponent protects; attack it to remove counters, and at zero its controller gets its reward.',
  ],
  Creature: [
    'Creature',
    'A permanent with power and toughness that can attack and block; it needs to be under your control since your latest turn began to attack or use {T}.',
  ],
  Enchantment: [
    'Enchantment',
    'A permanent with a lasting effect; an Aura is an enchantment attached to something.',
  ],
  Instant: [
    'Instant',
    "A spell you may cast any time you have priority, even on an opponent's turn or in response.",
  ],
  Kindred: [
    'Kindred|Tribal',
    'A card type that gives a non-creature card creature types, so it counts as an Elf, a Goblin and so on.',
  ],
  Land: [
    'Land',
    'A permanent you play, not cast: one a turn in your main phase while the stack is empty. Most lands tap for mana.',
  ],
  Planeswalker: [
    'Planeswalker',
    'A permanent with loyalty counters; activate one loyalty ability a turn at sorcery speed. Damage dealt to it removes loyalty.',
  ],
  Sorcery: ['Sorcery', 'A spell you may cast only in your main phase while the stack is empty.'],
}

/** Every supertype of the engine. */
export const SUPERTYPES: Record<Supertype, Entry> = {
  Basic: [
    'Basic|Basic land',
    'Plains, Island, Swamp, Mountain, Forest and Wastes; a deck may hold any number of them.',
  ],
  Legendary: [
    'Legendary',
    'A player who controls two or more legendary permanents with the same name chooses one and puts the rest into the graveyard (the legend rule).',
  ],
  Snow: [
    'Snow',
    'A supertype some cards count or reward; a snow permanent that makes mana makes snow mana ({S}).',
  ],
  World: [
    'World',
    'Only one world permanent can be on the battlefield; when a new one arrives the older ones go to the graveyard.',
  ],
}

/** Every layer of the engine's continuous effects, in the order they apply. */
export const LAYERS: Record<Layer, Entry> = {
  Copy: [
    'Layer 1: copy',
    'Copy effects apply first, so a copy copies the printed values plus other copy effects.',
  ],
  Control: ['Layer 2: control', 'Effects that change who controls a permanent.'],
  Text: [
    'Layer 3: text',
    'Effects that change the words of a card, such as a changed color or land type word.',
  ],
  Type: ['Layer 4: type', 'Effects that change card types, subtypes and supertypes.'],
  Color: ['Layer 5: color', 'Effects that change colors.'],
  Ability: ['Layer 6: abilities', 'Effects that add or remove abilities.'],
  PtCharacteristicDefining: [
    'Layer 7a: defining power and toughness',
    'Abilities that define power and toughness, such as "its power is equal to the number of cards in your hand".',
  ],
  PtSet: ['Layer 7b: set power and toughness', 'Effects that set power or toughness to a number.'],
  PtModify: [
    'Layer 7c: modify power and toughness',
    'Effects and counters that add to or subtract from power and toughness.',
  ],
  PtSwitch: ['Layer 7d: switch power and toughness', 'Effects that exchange power and toughness come last.'],
}

/** Every counter the engine names. */
export const COUNTERS: Record<Extract<CounterKind, string>, Entry> = {
  '+1/+1': [
    '+1/+1 counter',
    'Each gives the permanent +1/+1; a +1/+1 and a -1/-1 counter on one permanent cancel out.',
  ],
  '-1/-1': [
    '-1/-1 counter',
    'Each gives the permanent -1/-1; a creature with toughness 0 goes to the graveyard.',
  ],
  loyalty: [
    'Loyalty counter|Loyalty',
    "A planeswalker's loyalty: abilities add or remove them, damage removes them, and at zero it goes to the graveyard.",
  ],
  poison: ['Poison counter|Poison', 'A player with ten or more poison counters loses the game.'],
  charge: [
    'Charge counter',
    'A counter with no rules meaning of its own; the card that uses it says what it does.',
  ],
}
