import type { Entry } from './types'
import { CARD_TYPES, COUNTERS, LAYERS, STEPS, SUPERTYPES, ZONES } from './vocabulary'

/** The topic of a rules term, for the lexicon's grouping. */
export type RuleTopic =
  | 'Turn'
  | 'Zones'
  | 'Abilities'
  | 'Effects'
  | 'Costs and mana'
  | 'Cards and permanents'
  | 'Combat'
  | 'Commander'
  | 'Players'
  | 'Game'

const TURN: readonly Entry[] = [
  ...Object.values(STEPS),
  [
    'Turn',
    'Each turn has five phases: beginning, first main, combat, second main and ending; the players take turns in seat order.',
  ],
  ['Beginning phase', 'The untap, upkeep and draw steps.'],
  [
    'Combat phase',
    'Beginning of combat, declare attackers, declare blockers, combat damage and end of combat.',
  ],
  ['Ending phase', 'The end step and the cleanup step.'],
  [
    'Active player',
    'The player whose turn it is; they get priority first in every step and their triggers go on the stack first.',
  ],
  [
    'Extra turn',
    'A turn taken right after the current one; several extra turns are taken in the reverse order they were made.',
  ],
  ['Skip', 'A skipped step, phase or turn does not happen at all, so nothing in it triggers.'],
  [
    'Additional phase|Additional combat phase|Extra combat',
    'A phase added to the turn right after the current one, such as a second combat phase in which creatures attack again.',
  ],
]

const ZONE_TERMS: readonly Entry[] = [
  ...Object.values(ZONES),
  [
    'Zone',
    'A place a card can be: library, hand, battlefield, graveyard, stack, exile and the command zone.',
  ],
  [
    'Public zone|Hidden zone',
    'Everyone may see the battlefield, graveyards, the stack and exile; libraries and hands are hidden.',
  ],
  [
    'Enters|Enters the battlefield',
    'An object that moves onto the battlefield; "enters" triggers look at it as it arrives.',
  ],
  [
    'Leaves the battlefield',
    'An object that moves off the battlefield; it becomes a new object with no memory of its past.',
  ],
  ['Dies', 'A creature or planeswalker dies when it goes from the battlefield to a graveyard.'],
  [
    'Owner',
    "The player who started the game with a card in their deck, or who made a token; a card always goes to its owner's zones.",
  ],
]

const ABILITIES: readonly Entry[] = [
  [
    'Ability',
    'Text on an object that makes it do something; an ability on the stack is not a spell and is not countered by "counter target spell".',
  ],
  [
    'Triggered ability',
    'An ability that starts with "when", "whenever" or "at"; it goes on the stack the next time a player would get priority after its event.',
  ],
  [
    'Activated ability',
    'An ability written "cost: effect"; you may activate it whenever you have priority unless it says otherwise.',
  ],
  [
    'Static ability',
    'An ability that is simply true while its object is in play, such as "creatures you control get +1/+1".',
  ],
  [
    'Mana ability',
    'An ability that makes mana and has no target; it does not use the stack and resolves at once.',
  ],
  [
    'Loyalty ability',
    'An activated ability of a planeswalker whose cost adds or removes loyalty; one a turn, at sorcery speed.',
  ],
  [
    'Keyword ability',
    'A word or phrase that stands for a longer ability, such as flying or ward; the lexicon gives each one.',
  ],
  [
    'Ability word',
    'An italic word that groups cards with similar abilities; it has no rules meaning of its own.',
  ],
  [
    'Delayed triggered ability',
    'A trigger an effect sets up for later, such as "at the beginning of the next end step, return it".',
  ],
  [
    'Reflexive triggered ability',
    'A trigger inside an effect: "when you do, ..." happens only if the thing before it was done.',
  ],
  [
    'Intervening if clause',
    'A trigger written "when X, if Y, ..." checks Y when it triggers and again when it resolves.',
  ],
  [
    'Characteristic-defining ability',
    'An ability that defines a value of its own object everywhere, such as "this creature\'s power is equal to ...".',
  ],
  [
    'Spell',
    'A card or copy on the stack being cast; it becomes a permanent on resolution unless it is an instant or sorcery.',
  ],
  [
    'Target',
    'An object or player chosen when a spell or ability is put on the stack; if every target is gone at resolution, it does nothing.',
  ],
  [
    'Modal|Choose one',
    'A spell or ability with options to choose from as it is cast; only the chosen modes happen.',
  ],
]

const EFFECTS: readonly Entry[] = [
  [
    'One-shot effect',
    'An effect that does something once and is done, such as drawing a card or dealing damage.',
  ],
  [
    'Continuous effect',
    'An effect that lasts for a time, such as "until end of turn" or while its source is on the battlefield.',
  ],
  [
    'Replacement effect',
    'An effect that uses "instead", "skip" or "enters with" to change an event before it happens; the affected player picks the order when several apply.',
  ],
  [
    'Prevention effect',
    'A replacement effect that stops damage from being dealt, such as "prevent the next 3 damage".',
  ],
  [
    'Copy',
    'A copy takes the copiable values of the original: name, cost, types, text, power and toughness; not counters, tapped state or other effects.',
  ],
  [
    'Control|Controller',
    'The player who controls a permanent decides what it does; you control what you cast unless an effect gives control away.',
  ],
  [
    'Gain control',
    'An effect that makes you the controller of a permanent; it has summoning sickness for you unless it has haste.',
  ],
  [
    'Layers|Layer system',
    'Continuous effects apply in layers: copy, control, text, type, color, abilities, then power and toughness.',
  ],
  ...Object.values(LAYERS),
  [
    'Timestamp',
    'Within a layer, effects apply in the order they started, oldest first, unless one depends on another.',
  ],
  [
    'Damage',
    'Damage to a player is life loss, to a creature stays until cleanup, to a planeswalker removes loyalty and to a battle removes defense.',
  ],
  ['Life', 'Each player starts at 20 life, 40 in Commander; a player at 0 or less loses the game.'],
  [
    'Emblem',
    'An object in the command zone with abilities that last for the rest of the game; it can not be removed.',
  ],
  ['Until end of turn', 'An effect that lasts until the cleanup step of this turn.'],
]

const COSTS: readonly Entry[] = [
  [
    'Cost',
    'What you pay to cast a spell or activate an ability: mana, tapping, life, sacrifice and so on, all at once.',
  ],
  ['Mana cost', 'The mana symbols in the top corner of a card; you pay it to cast the card.'],
  [
    'Mana value|Converted mana cost',
    'The total amount of mana in a mana cost, colors ignored; X is 0 except on the stack.',
  ],
  [
    'Additional cost',
    'A cost paid on top of the mana cost, such as kicker or "as an additional cost, sacrifice a creature".',
  ],
  [
    'Alternative cost',
    'A cost paid instead of the mana cost, such as flashback or evoke; only one may be used.',
  ],
  ['Without paying its mana cost', 'Cast with no mana paid; additional costs still apply and X is 0.'],
  ['Mana pool', 'Where mana waits until spent; unspent mana empties at the end of each step and phase.'],
  ['Generic mana', 'A number in a cost; any type of mana pays for it.'],
  ['Colorless mana', 'Mana of no color, shown as {C}; a {C} in a cost must be paid with colorless mana.'],
  ['Hybrid mana', 'A split symbol paid with either of its two colors, or for {2/W} two generic mana.'],
  ['Phyrexian mana', 'A symbol with a slash and P, paid with its color or 2 life.'],
  ['X', 'A number you choose as you cast the spell or activate the ability, and then pay for.'],
  [
    'Tap symbol|{T}',
    'Part of a cost: tap this permanent. A creature needs to have been under your control since your latest turn began to pay it.',
  ],
]

const PERMANENTS: readonly Entry[] = [
  ...Object.values(CARD_TYPES),
  ...Object.values(SUPERTYPES),
  ...Object.values(COUNTERS),
  ['Permanent', 'A card or token on the battlefield.'],
  [
    'Token',
    'A permanent that is no card, made by an effect; it ceases to exist when it leaves the battlefield.',
  ],
  [
    'Counters',
    'Markers on a permanent or player that change it or count something; they leave with the object.',
  ],
  [
    'Subtype|Creature type',
    'The words after the dash in the type line, such as Elf, Equipment, Aura or Forest.',
  ],
  [
    'Aura',
    'An enchantment that is attached to an object or player; it goes to the graveyard when that is gone.',
  ],
  ['Equipment', 'An artifact you attach to a creature you control with equip, at sorcery speed.'],
  [
    'Saga',
    'An enchantment that gets a lore counter as it enters and each precombat main phase; each counter triggers a chapter.',
  ],
  ['Room', 'An enchantment with two doors; you unlock a door by paying its cost and then its ability works.'],
  [
    'Attraction|Attractions',
    'A card of a separate deck of Attractions; open one to put it on the battlefield, and roll to visit them at your turn.',
  ],
  [
    'Face down|Face-down permanent',
    'A face-down spell or permanent is a nameless, colorless 2/2 creature with no text; its controller may look at it.',
  ],
  [
    'Double-faced card|Transforming card',
    'A card with two faces; it transforms between them or you choose which face to play.',
  ],
  ['Split card', 'A card with two halves; you cast one half, or both with fuse.'],
  [
    'Adventure',
    'A card with a small spell beside it: cast the spell, exile the card, then cast the creature later from exile.',
  ],
  [
    'Power and toughness|Power|Toughness',
    'Power is the damage a creature deals in combat; toughness is the damage it can take before it is destroyed.',
  ],
  [
    'Summoning sickness',
    'A creature can not attack or use {T} abilities until you have controlled it since your latest turn began.',
  ],
  [
    'Tapped|Untapped',
    'A tapped permanent is turned sideways: it has been used this turn. Tapped creatures can not block.',
  ],
  [
    'Vehicle',
    'An artifact that is not a creature until it is crewed: tap creatures with total power N to make it an artifact creature until end of turn.',
  ],
  [
    'Class',
    "An enchantment with levels: pay a level's cost, at sorcery speed and in order, and it gains that level's abilities for good.",
  ],
  [
    'Case',
    'An enchantment with a "to solve" condition; it becomes solved when the condition is true at the beginning of your end step and then gets its solved ability.',
  ],
  [
    'Flip card|Flip cards',
    'A card with a second, upside-down half; it flips over when its condition is met, becomes its bottom half and stays so until it leaves the battlefield.',
  ],
  [
    'Stun counter',
    'If a permanent with a stun counter would become untapped, remove a stun counter from it instead.',
  ],
  [
    'Shield counter',
    'If a permanent with a shield counter would be dealt damage or destroyed, remove a shield counter from it instead.',
  ],
  ['Finality counter', 'If a permanent with a finality counter would die, exile it instead.'],
  [
    'Time counter',
    'Counts down suspend and vanishing and is added or removed by time travel; the card says what happens at zero.',
  ],
  [
    'Lore counter',
    'A Saga gets one as it enters and at each of your precombat main phases; each one triggers the chapter ability of that number.',
  ],
  [
    'Defense counter',
    "A battle's defense: each point of damage dealt to it removes one, and at zero the battle is defeated.",
  ],
  [
    'Level counter',
    'Level up adds one; a leveler has the abilities and the power and toughness of the range its level counters fall in.',
  ],
  [
    'Age counter',
    'Cumulative upkeep adds one at each of your upkeeps, and its cost is paid once for each age counter.',
  ],
  [
    'Fade counter',
    'Fading removes one at each of your upkeeps; you sacrifice the permanent when you can not remove one.',
  ],
  [
    'Keyword counter',
    'A counter named for a keyword, such as flying or lifelink; the permanent has that ability while it has one.',
  ],
  [
    'Legend rule',
    'Two or more legendary permanents with the same name under one player: they keep one and the rest go to the graveyard.',
  ],
]

const COMBAT: readonly Entry[] = [
  [
    'Attacking creature|Attacker',
    'A creature declared as an attacker; it stays attacking until combat ends or it is removed from combat.',
  ],
  [
    'Blocking creature|Blocker',
    'A creature declared as a blocker; each blocks one attacker unless an ability says more.',
  ],
  [
    'Blocked|Unblocked',
    'An attacker stays blocked even if its blockers leave; an unblocked attacker deals its damage to what it attacks.',
  ],
  [
    'Defending player',
    'The player an attacker attacks, or the controller of the planeswalker or protector of the battle it attacks.',
  ],
  ['Combat damage', 'Damage creatures deal in the combat damage step; equal to their power.'],
  [
    'Lethal damage',
    "Damage equal to a creature's toughness minus the damage it already has; any damage from deathtouch is lethal.",
  ],
  [
    'Removed from combat',
    'A creature that stops attacking or blocking but stays on the battlefield; it deals and gets no combat damage.',
  ],
]

const COMMANDER: readonly Entry[] = [
  [
    'Commander',
    'A legendary creature (or a card that says it can be) leading a 100-card singleton deck; it starts in the command zone.',
  ],
  [
    'Color identity',
    "Every mana symbol on a card, cost and text both; a Commander deck uses only cards within its commander's colors.",
  ],
  ['Singleton', 'One copy of each card in a deck, basic lands excepted.'],
  ['Monarch', 'The monarch draws a card at their end step; deal combat damage to them to take it.'],
  [
    'Initiative',
    'Taking the initiative ventures into the Undercity; deal combat damage to the holder to take it.',
  ],
  ['Dungeon', 'A card in the command zone you move through room by room as you venture.'],
]

const GAME: readonly Entry[] = [
  ['Priority', 'The right to act; you may cast a spell or activate an ability only while you have priority.'],
  [
    'Pass|Pass priority',
    'Hand priority on; when every player passes in a row, the top of the stack resolves or the step ends.',
  ],
  [
    'Resolve',
    'A spell or ability does what it says as it leaves the stack; a permanent spell enters the battlefield.',
  ],
  ['Respond|In response', 'Casting or activating while something is on the stack, so yours resolves first.'],
  ['Instant speed', 'Any time you have priority.'],
  ['Sorcery speed', 'Only in your main phase, while the stack is empty and you have priority.'],
  [
    'State-based actions',
    'Checks the game makes before any player gets priority: 0 life loses, lethal damage destroys, the legend rule and more.',
  ],
  [
    'Mulligan',
    'Shuffle your opening hand away and draw a new one; for each mulligan, put one card on the bottom.',
  ],
  ['Maximum hand size', 'Seven cards; in your cleanup step you discard down to it.'],
  [
    'Lose the game',
    'At 0 life, ten poison counters, 21 combat damage from one commander, drawing from an empty library, or conceding.',
  ],
  [
    'Day and night|Day|Night',
    'Once day or night starts it stays; it becomes night after a turn with no spells by its player, and day after one with two.',
  ],
  ['Random', 'Chosen by chance: a die, a coin flip or a shuffled pick.'],
  [
    'Win the game|Draw the game',
    'The last player left wins; a card can also say a player wins. If every remaining player loses at once, or a card says so, the game is a draw.',
  ],
  [
    'Restart the game',
    "The game begins again with the same players: every card goes back to its owner's library and life totals reset, except what the restarting effect keeps.",
  ],
  [
    'Control another player|Control a player',
    "You make that player's decisions during their turn; their cards and permanents stay theirs.",
  ],
]

const PLAYERS: readonly Entry[] = [
  [
    'Ring-bearer|Ring-bearers',
    "The creature you chose when the Ring tempted you. It is legendary and has the Ring's abilities so far; it stops being your Ring-bearer when you choose another, it leaves the battlefield or another player gains control of it.",
  ],
  [
    'Ring ability 1: legendary, evasive',
    'Your Ring-bearer is legendary and can not be blocked by creatures with greater power.',
  ],
  ['Ring ability 2: loot', 'Whenever your Ring-bearer attacks, draw a card, then discard a card.'],
  [
    'Ring ability 3: sacrifice the blocker',
    "Whenever your Ring-bearer becomes blocked by a creature, that creature's controller sacrifices it at end of combat.",
  ],
  [
    'Ring ability 4: drain',
    'Whenever your Ring-bearer deals combat damage to a player, each opponent loses 3 life.',
  ],
  [
    "City's blessing",
    'A status you keep for the rest of the game: you get it when an ascend ability checks and you control ten or more permanents.',
  ],
  [
    'Speed',
    'A number from 1 to 4 that start your engines! gives you. It rises by 1 once each turn when an opponent loses life during your turn, and at 4 you have max speed.',
  ],
  [
    'Energy counter|Energy counters',
    'A counter on a player that effects give; {E} in a cost pays by removing one.',
  ],
  [
    'Experience counter|Experience counters',
    'A counter on a player that effects give; cards count them and nothing removes them.',
  ],
  [
    'Rad counter|Rad counters',
    'A counter on a player. As their precombat main phase begins they mill that many cards, then lose 1 life and remove a rad counter for each nonland card milled.',
  ],
  [
    'Ticket counter|Ticket counters',
    'A counter on a player that pays for stickers: putting a sticker on a card removes as many as its cost.',
  ],
]

/** Every rules term by topic. */
export const RULES: Readonly<Record<RuleTopic, readonly Entry[]>> = {
  Turn: TURN,
  Zones: ZONE_TERMS,
  Abilities: ABILITIES,
  Effects: EFFECTS,
  'Costs and mana': COSTS,
  'Cards and permanents': PERMANENTS,
  Combat: COMBAT,
  Commander: COMMANDER,
  Players: PLAYERS,
  Game: GAME,
}

/** The rules terms marked where a card or log line names them; the rest are plain words there and listed only in the lexicon. */
export const MARKED_RULES: ReadonlySet<string> = new Set([
  'Upkeep',
  'Draw step',
  'Main phase',
  'Postcombat main phase',
  'Beginning of combat',
  'Declare attackers step',
  'Declare blockers step',
  'Combat damage step',
  'End of combat step',
  'End step',
  'Cleanup step',
  'Command zone',
  'Triggered ability',
  'Activated ability',
  'Mana ability',
  'Loyalty ability',
  'Mana value',
  'Additional cost',
  'Alternative cost',
  'Without paying its mana cost',
  'Face down',
  'Attraction',
  'Legend rule',
  'Color identity',
  'Monarch',
  'Initiative',
  'Emblem',
  'Ring-bearer',
  "City's blessing",
  'Energy counter',
  'Experience counter',
  'Rad counter',
  'Ticket counter',
  'Summoning sickness',
  'State-based actions',
])
