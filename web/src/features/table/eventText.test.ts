// Goal: every kind of log event, public and hidden, reads as one plain sentence with the right names, and the quiet ones (priority, mana, untaps) are marked minor.
import { describe, expect, it } from 'vitest'
import type {
  ContinuousEffect,
  Event,
  EventView,
  HiddenEvent,
  RuleEffect,
  Snapshot,
} from '@/protocol/generated'
import { describeEvent, joinWords } from './eventText'
import type { EventNames } from './eventText'

const SEATS = ['Ana', 'Bo', 'Cy']
const CARDS: Record<number, string> = {
  1: 'Grizzly Bears',
  2: 'Hill Giant',
  3: 'Forest',
  4: 'Lightning Bolt',
  5: 'Serra Angel',
  6: 'Jace Beleren',
}
const names: EventNames = {
  seat: (seat) => SEATS[seat] ?? `Seat ${seat + 1}`,
  card: (id) => CARDS[id] ?? 'a card',
}

/** The name of every variant of a union whose members are strings or objects with one key. */
type VariantName<E> = E extends string ? E : E extends object ? keyof E : never

const PUBLIC: Record<VariantName<Event>, [Event, string]> = {
  FirstPlayerChosen: [{ FirstPlayerChosen: { player: 1 } }, 'Bo goes first'],
  LibraryShuffled: [{ LibraryShuffled: { player: 0, order: [] } }, 'Ana shuffles their library'],
  MulliganPhaseBegan: ['MulliganPhaseBegan', 'The mulligan phase begins'],
  HandDealt: [{ HandDealt: { player: 0, cards: [1, 2, 3, 4, 5, 6, 3] } }, 'Ana is dealt 7 cards'],
  MulliganDeclared: [{ MulliganDeclared: { player: 1 } }, 'Bo decides to mulligan'],
  MulliganTaken: [{ MulliganTaken: { player: 1, library_order: [] } }, 'Bo takes a mulligan'],
  MulliganKept: [{ MulliganKept: { player: 0 } }, 'Ana keeps their hand'],
  CardsBottomed: [
    { CardsBottomed: { player: 0, cards: [1, 2] } },
    'Ana puts 2 cards on the bottom of their library',
  ],
  LibraryBottomed: [
    { LibraryBottomed: { player: 0, cards: [1, 2], random: false } },
    'Ana puts 2 cards on the bottom of their library',
  ],
  Entered: [
    { Entered: { object: 3, tapped: true, controller: 1 } },
    "Forest enters tapped under Bo's control",
  ],
  Attached: [{ Attached: { object: 3, to: 1 } }, 'Forest becomes attached to Grizzly Bears'],
  AttachedToPlayer: [{ AttachedToPlayer: { object: 3, player: 1 } }, 'Forest enchants Bo'],
  Unattached: [{ Unattached: { object: 3 } }, 'Forest becomes unattached'],
  JoinedAttack: [{ JoinedAttack: { object: 3, target: { Player: 0 } } }, 'Forest enters attacking Ana'],
  ManualAction: [
    { ManualAction: { player: 0, action: { Move: { object: 3, to: 'Graveyard' } } } },
    'Ana moves Forest to their graveyard by hand',
  ],
  Transformed: [{ Transformed: { object: 3, face: 1 } }, 'Forest transforms'],
  BecameCopy: [
    { BecameCopy: { object: 3, of: { id: 5, counter: 0 } } },
    'Forest becomes a copy of Serra Angel',
  ],
  SpellCopied: [
    { SpellCopied: { object: 9, of: { id: 5, counter: 0 }, controller: 0 } },
    'Ana copies Serra Angel',
  ],
  AbilityCopied: [
    { AbilityCopied: { object: 9, of: { id: 5, counter: 0 }, controller: 0 } },
    'Ana copies the ability of Serra Angel',
  ],
  CopyEnded: [{ CopyEnded: { object: 3 } }, 'Forest stops being a copy'],
  DayNightChanged: [{ DayNightChanged: { to: 'Night' } }, 'It becomes night'],
  MonarchChanged: [{ MonarchChanged: { player: 1 } }, 'Bo becomes the monarch'],
  Proliferated: [{ Proliferated: { player: 1 } }, 'Bo proliferates'],
  Learned: [{ Learned: { player: 1 } }, 'Bo learns'],
  CitysBlessingGained: [{ CitysBlessingGained: { player: 1 } }, "Bo gets the city's blessing"],
  Explored: [{ Explored: { object: 3, player: 1 } }, 'Forest explores'],
  Connived: [{ Connived: { object: 3, player: 1 } }, 'Forest connives'],
  Amassed: [{ Amassed: { object: 3, player: 1 } }, 'Bo amasses'],
  Monstrous: [{ Monstrous: { object: 3, player: 1 } }, 'Forest becomes monstrous'],
  TurnedFaceUp: [{ TurnedFaceUp: { object: 3, player: 1 } }, 'Bo turns Forest face up'],
  Mutated: [{ Mutated: { object: 3, spell: 9, over: true, player: 1 } }, 'Bo mutates a creature over Forest'],
  Adapted: [{ Adapted: { object: 3, player: 1 } }, 'Forest adapts'],
  Bolstered: [{ Bolstered: { object: 3, player: 1 } }, 'Bo bolsters'],
  CommanderStayed: [{ CommanderStayed: { object: 3 } }, 'Forest stays where it is'],
  MulliganPhaseEnded: ['MulliganPhaseEnded', 'The mulligan phase ends'],
  TurnBegan: [{ TurnBegan: { number: 3, active: 1 } }, "Turn 3 begins: Bo's turn"],
  StepBegan: [{ StepBegan: { step: 'DeclareAttackers' } }, 'Declare attackers begins'],
  NothingToDo: [{ NothingToDo: { player: 0, kind: 'Attack' } }, 'Ana has no possible attackers'],
  PriorityGranted: [{ PriorityGranted: { player: 0 } }, 'Ana receives priority'],
  PriorityPassed: [{ PriorityPassed: { player: 1 } }, 'Bo passes priority'],
  LandPlayed: [{ LandPlayed: { player: 0, object: 3 } }, 'Ana plays Forest'],
  SpellCast: [{ SpellCast: { player: 1, object: 4 } }, 'Bo casts Lightning Bolt'],
  SpellResolved: [{ SpellResolved: { object: 4 } }, 'Lightning Bolt resolves'],
  PermanentTapped: [{ PermanentTapped: { object: 3 } }, 'Forest becomes tapped'],
  PermanentUntapped: [{ PermanentUntapped: { object: 3 } }, 'Forest becomes untapped'],
  ManaAdded: [{ ManaAdded: { player: 0, mana: 'Green', source: 3 } }, 'Ana adds {G} from Forest'],
  ManaSpent: [{ ManaSpent: { player: 0, mana: ['Green', 'Red', 'Colorless'] } }, 'Ana spends {G}{R}{C}'],
  ManaPoolEmptied: [{ ManaPoolEmptied: { player: 0 } }, "Ana's mana pool empties"],
  ManaBurned: [{ ManaBurned: { player: 0, amount: 3 } }, "Ana's unspent mana burns"],
  LibraryRefilled: [
    { LibraryRefilled: { player: 0, order: [] } },
    'Ana shuffles their graveyard into their library',
  ],
  LifePaid: [{ LifePaid: { player: 1, amount: 4 } }, 'Bo pays 4 life'],
  CardDrawn: [{ CardDrawn: { player: 0, object: 3 } }, 'Ana draws Forest'],
  DrewFromEmptyLibrary: [{ DrewFromEmptyLibrary: { player: 1 } }, 'Bo tries to draw from an empty library'],
  CardDiscarded: [{ CardDiscarded: { player: 0, object: 2 } }, 'Ana discards Hill Giant'],
  Cycled: [{ Cycled: { player: 0, object: 2 } }, 'Ana cycles Hill Giant'],
  AttackersDeclared: [
    {
      AttackersDeclared: {
        player: 1,
        attackers: [
          [1, { Player: 0 }],
          [2, { Player: 0 }],
        ],
      },
    },
    'Bo attacks Ana with Grizzly Bears and Hill Giant',
  ],
  BlockersDeclared: [
    { BlockersDeclared: { player: 0, blocks: [[2, 1]] } },
    'Ana declares blockers: Hill Giant blocks Grizzly Bears',
  ],
  DamageAssigned: [
    {
      DamageAssigned: {
        attacker: 5,
        assignment: [
          [1, 3],
          [2, 2],
        ],
      },
    },
    'Serra Angel assigns 3 to Grizzly Bears and 2 to Hill Giant',
  ],
  DamageDealt: [
    { DamageDealt: { source: 4, target: { Player: 1 }, amount: 3 } },
    'Lightning Bolt deals 3 damage to Bo',
  ],
  CombatEnded: ['CombatEnded', 'Combat ends'],
  DamageCleared: ['DamageCleared', 'Damage wears off'],
  EffectCreated: [
    {
      EffectCreated: {
        effect: effect(objects(1), { ModifyPowerToughness: { power: 2, toughness: -1 } }, 'UntilEndOfTurn'),
      },
    },
    'Grizzly Bears gets +2/-1 until end of turn',
  ],
  EffectExpired: [{ EffectExpired: { id: 1 } }, 'An effect ends'],
  RuleCreated: [
    { RuleCreated: { effect: ruleEffect({ CantGainLife: [0, 1] }, 'UntilEndOfTurn') } },
    "Ana and Bo can't gain life until end of turn",
  ],
  RuleExpired: [{ RuleExpired: { id: 1 } }, 'A rule change ends'],
  Regenerated: [{ Regenerated: { object: 1 } }, 'Grizzly Bears regenerates'],
  ShieldUsed: [{ ShieldUsed: { id: 2, left: 3 } }, 'A prevention shield has 3 left'],
  CreatureDied: [
    { CreatureDied: { object: 1, cause: 'LethalDamage' } },
    'Grizzly Bears dies from lethal damage',
  ],
  PlayerLost: [{ PlayerLost: { player: 1, reason: 'ZeroLife' } }, 'Bo ran out of life'],
  ObjectLeftGame: [{ ObjectLeftGame: { object: 2 } }, 'Hill Giant leaves the game'],
  GameEnded: [{ GameEnded: { result: { Winner: 0 } } }, 'Ana wins the game'],
  AbilityPutOnStack: [
    {
      AbilityPutOnStack: {
        object: 90,
        source: { id: 5, counter: 0 },
        snapshot: snapshotOf(5),
        ability: 0,
        controller: 1,
      },
    },
    'An ability of Serra Angel goes on the stack',
  ],
  AbilityActivated: [
    { AbilityActivated: { player: 1, source: { id: 5, counter: 0 }, ability: 0 } },
    'Bo activates an ability of Serra Angel',
  ],
  AbilityTriggered: [
    {
      AbilityTriggered: {
        source: { id: 6, counter: 0 },
        ability: 1,
        controller: 0,
        cause: { subjects: [], snapshots: [] },
        snapshot: snapshotOf(6),
      },
    },
    'An ability of Jace Beleren triggers',
  ],
  TriggersOrdered: [
    { TriggersOrdered: { player: 1, order: [1, 0] } },
    'Bo puts their triggered abilities in order',
  ],
  TriggerRemoved: [
    {
      TriggerRemoved: { source: { id: 6, counter: 0 }, ability: 1, cause: { subjects: [], snapshots: [] } },
    },
    'A triggered ability of Jace Beleren is removed',
  ],
  DelayedTriggerCreated: [
    {
      DelayedTriggerCreated: {
        id: 0,
        source: { id: 6, counter: 0 },
        snapshot: snapshotOf(6),
        controller: 0,
        ability: 0,
        when: null,
        once: true,
        effects: [],
        bindings: [],
      },
    },
    'A delayed ability of Jace Beleren is set up',
  ],
  DelayedTriggerExpired: [{ DelayedTriggerExpired: { id: 0 } }, 'A delayed ability ends without triggering'],
  ResolutionBegan: [{ ResolutionBegan: { object: 4 } }, 'Lightning Bolt begins to resolve'],
  ChoiceMade: [
    { ChoiceMade: { player: 0, choice: { Cards: [1, 2] } } },
    'Ana chooses Grizzly Bears and Hill Giant',
  ],
  StepDone: [{ StepDone: { position: [1], bound: [], snapshots: [] } }, 'A step of the resolution is done'],
  ResolutionEnded: [
    { ResolutionEnded: { object: 4, outcome: 'Resolved' } },
    'Lightning Bolt finishes resolving',
  ],
  ZoneChanged: [
    { ZoneChanged: { object: 1, from: 'Battlefield', to: { Graveyard: 0 }, cause: 'Destroyed' } },
    'Grizzly Bears is destroyed',
  ],
  LifeGained: [{ LifeGained: { player: 0, amount: 3 } }, 'Ana gains 3 life'],
  LifeLost: [{ LifeLost: { player: 1, amount: 2 } }, 'Bo loses 2 life'],
  CountersChanged: [
    { CountersChanged: { on: { Object: { id: 1, counter: 0 } }, counter: '+1/+1', delta: 2 } },
    'Grizzly Bears gets 2 +1/+1 counters',
  ],
  TokenCreated: [
    { TokenCreated: { object: 91, controller: 2, name: 'Treasure' } },
    'Cy creates a Treasure token',
  ],
  LibraryLooked: [
    { LibraryLooked: { player: 0, cards: [1, 2, 3] } },
    'Ana looks at 3 cards of their library',
  ],
  CardsFound: [{ CardsFound: { player: 1, zone: { Library: 1 }, cards: [3] } }, 'Bo finds 1 card'],
  Scried: [{ Scried: { player: 0, top: [1], bottom: [2, 3] } }, 'Ana scries 3'],
  Surveilled: [{ Surveilled: { player: 0, top: [1], graveyard: [2, 3] } }, 'Ana surveils 3'],
  Revealed: [{ Revealed: { player: 0, cards: [1, 2] } }, 'Ana reveals Grizzly Bears and Hill Giant'],
}

/** The ways the pipeline's events that need more than one sentence differ, each next to its text. */
const MORE: [Event, string][] = [
  [{ Mutated: { object: 3, spell: 9, over: false, player: 1 } }, 'Bo mutates a creature under Forest'],
  [
    { ResolutionEnded: { object: 4, outcome: 'Fizzled' } },
    'Lightning Bolt does nothing: its targets are gone',
  ],
  [{ ResolutionEnded: { object: 4, outcome: 'Countered' } }, 'Lightning Bolt is countered'],
  [
    { ResolutionEnded: { object: 4, outcome: 'Lapsed' } },
    'Lightning Bolt does nothing: its condition is no longer true',
  ],
  [
    { ZoneChanged: { object: 1, from: 'Battlefield', to: { Graveyard: 0 }, cause: 'Sacrificed' } },
    'Grizzly Bears is sacrificed',
  ],
  [
    { ZoneChanged: { object: 3, from: { Library: 0 }, to: { Hand: 0 }, cause: 'Moved' } },
    "Forest moves to Ana's hand",
  ],
  [{ ZoneChanged: { object: 3, from: 'Battlefield', to: 'Exile', cause: 'Moved' } }, 'Forest moves to exile'],
  [{ ZoneChanged: { object: 3, from: { Hand: 0 }, to: 'Exile', cause: 'Foretold' } }, 'Forest is foretold'],
  [
    {
      ZoneChanged: {
        object: 3,
        from: { Library: 0 },
        to: 'Exile',
        cause: { Hideaway: { by: { id: 9, counter: 1 } } },
      },
    },
    'Forest is exiled face down',
  ],
  [
    { ZoneChanged: { object: 3, from: 'Stack', to: 'Battlefield', cause: 'Moved' } },
    'Forest moves to the battlefield',
  ],
  [{ ChoiceMade: { player: 1, choice: { Cards: [] } } }, 'Bo chooses nothing'],
  [{ ChoiceMade: { player: 1, choice: { Number: 3 } } }, 'Bo chooses 3'],
  [{ ChoiceMade: { player: 1, choice: { YesNo: true } } }, 'Bo answers yes'],
  [{ ChoiceMade: { player: 1, choice: { YesNo: false } } }, 'Bo answers no'],
  [{ ChoiceMade: { player: 1, choice: { Order: [1, 2] } } }, 'Bo puts 2 cards in order'],
  [{ ChoiceMade: { player: 1, choice: { Option: 0 } } }, 'Bo chooses option 1'],
  [{ ChoiceMade: { player: 1, choice: { Replacement: 1 } } }, 'Bo chooses which effect applies first'],
  [{ CountersChanged: { on: { Player: 1 }, counter: 'poison', delta: -1 } }, 'Bo loses a poison counter'],
  [{ CountersChanged: { on: { Player: 1 }, counter: 'poison', delta: 3 } }, 'Bo gets 3 poison counters'],
  [
    { CountersChanged: { on: { Player: 0 }, counter: { Other: 'energy' }, delta: 2 } },
    'Ana gets 2 energy counters',
  ],
  [
    { CountersChanged: { on: { Object: { id: 1, counter: 0 } }, counter: '-1/-1', delta: 1 } },
    'Grizzly Bears gets a -1/-1 counter',
  ],
  [
    { CountersChanged: { on: { Object: { id: 1, counter: 0 } }, counter: '+1/+1', delta: -2 } },
    'Grizzly Bears loses 2 +1/+1 counters',
  ],
  [
    { CountersChanged: { on: { Object: { id: 1, counter: 1 } }, counter: 'loyalty', delta: 3 } },
    'Grizzly Bears gets 3 loyalty counters',
  ],
  [
    { CountersChanged: { on: { Object: { id: 1, counter: 1 } }, counter: 'loyalty', delta: -1 } },
    'Grizzly Bears loses a loyalty counter',
  ],
  [
    { ZoneChanged: { object: 1, from: 'Battlefield', to: { Graveyard: 0 }, cause: 'StateBased' } },
    'Grizzly Bears is put into the graveyard',
  ],
]

function snapshotOf(id: number): Snapshot {
  return {
    id,
    counter: 0,
    controller: 0,
    owner: 0,
    zone: 'Battlefield',
    characteristics: { types: 4, colors: 0, power: 1, toughness: 1, abilities: [] },
    counters: [],
    attachments: [],
  }
}

const HIDDEN: Record<VariantName<HiddenEvent>, [HiddenEvent, string]> = {
  CardDrawn: [{ CardDrawn: { player: 1 } }, 'Bo draws a card'],
  HandDealt: [{ HandDealt: { player: 1, count: 7 } }, 'Bo is dealt 7 cards'],
  LibraryShuffled: [{ LibraryShuffled: { player: 1 } }, 'Bo shuffles their library'],
  LibraryRefilled: [
    { LibraryRefilled: { player: 1, count: 5 } },
    'Bo shuffles their graveyard into their library',
  ],
  MulliganTaken: [{ MulliganTaken: { player: 1 } }, 'Bo takes a mulligan'],
  CardsBottomed: [
    { CardsBottomed: { player: 1, count: 1 } },
    'Bo puts 1 card on the bottom of their library',
  ],
  ObjectLeftGame: ['ObjectLeftGame', 'A card leaves the game'],
  LibraryLooked: [{ LibraryLooked: { player: 1, count: 2 } }, 'Bo looks at 2 cards of their library'],
  CardsFound: [{ CardsFound: { player: 1, count: 1 } }, 'Bo finds 1 card'],
  Scried: [{ Scried: { player: 1, top: 1, bottom: 1 } }, 'Bo scries 2'],
  Surveilled: [{ Surveilled: { player: 1, top: 1, graveyard: 2 } }, 'Bo surveils 3'],
  ZoneChanged: [
    { ZoneChanged: { from: { Library: 1 }, to: { Hand: 1 }, cause: 'Moved' } },
    "A card moves to Bo's hand",
  ],
  ChoiceMade: [{ ChoiceMade: { player: 1 } }, 'Bo chooses'],
  ManualMove: [
    { ManualMove: { player: 1, to: 'LibraryTop' } },
    'Bo moves a card to the top of their library by hand',
  ],
  StepDone: [{ StepDone: { position: [2] } }, 'A step of the resolution is done'],
  AbilityTriggered: [
    { AbilityTriggered: { source: { id: 6, counter: 0 }, ability: 1, controller: 0 } },
    'An ability of Jace Beleren triggers',
  ],
  AbilityPutOnStack: [
    { AbilityPutOnStack: { object: 90, source: { id: 6, counter: 0 }, controller: 0 } },
    'An ability of Jace Beleren goes on the stack',
  ],
  TriggerRemoved: [
    { TriggerRemoved: { source: { id: 6, counter: 0 }, ability: 1 } },
    'A triggered ability of Jace Beleren is removed',
  ],
  DelayedTriggerCreated: [
    { DelayedTriggerCreated: { source: { id: 6, counter: 0 }, controller: 0 } },
    'A delayed ability of Jace Beleren is set up',
  ],
}

function objects(...ids: number[]): ContinuousEffect['selector'] {
  return { Objects: ids.map((id) => ({ id, counter: 0 })) }
}

function ruleEffect(
  rule: RuleEffect['rule'],
  duration: RuleEffect['duration'],
  source: RuleEffect['source'] = null,
): RuleEffect {
  return { id: 1, rule, duration, source, controller: 0, turn: 3 }
}

function effect(
  selector: ContinuousEffect['selector'],
  modification: ContinuousEffect['modification'],
  duration: ContinuousEffect['duration'],
  source: ContinuousEffect['source'] = null,
): ContinuousEffect {
  return {
    id: 1,
    layer: 'PtModify',
    selector,
    modification,
    duration,
    timestamp: 1,
    source,
    controller: 0,
    turn: 1,
  }
}

const say = (event: Event): string => describeEvent({ Public: event }, names).text

describe('describeEvent', () => {
  it.each(Object.entries(PUBLIC))('says the public event %s in one sentence', (_name, [event, text]) => {
    expect(describeEvent({ Public: event }, names).text).toBe(text)
  })

  it.each(Object.entries(HIDDEN))('says the hidden event %s in one sentence', (_name, [event, text]) => {
    expect(describeEvent({ Hidden: event }, names).text).toBe(text)
  })

  it('tells the others who hid a card and nothing about it', () => {
    const hidden: HiddenEvent = {
      ZoneChanged: { from: { Library: 1 }, to: 'Exile', cause: { Hideaway: { by: { id: 9, counter: 1 } } } },
    }
    expect(describeEvent({ Hidden: hidden }, names)).toEqual({
      text: 'Bo hides a card face down',
      minor: false,
    })
  })

  it('tells the others who foretold and nothing about the card', () => {
    const hidden: HiddenEvent = { ZoneChanged: { from: { Hand: 1 }, to: 'Exile', cause: 'Foretold' } }
    expect(describeEvent({ Hidden: hidden }, names)).toEqual({ text: 'Bo foretells a card', minor: false })
  })

  it.each(MORE)('says the pipeline event %j in the right words', (event, text) => {
    expect(describeEvent({ Public: event }, names).text).toBe(text)
  })

  it('marks the bookkeeping of a resolution minor and what happened to the game not', () => {
    const minor = (event: Event): boolean => describeEvent({ Public: event }, names).minor
    expect(minor({ StepDone: { position: [1], bound: [], snapshots: [] } })).toBe(true)
    expect(minor({ ResolutionBegan: { object: 4 } })).toBe(true)
    expect(minor({ LifeGained: { player: 0, amount: 1 } })).toBe(false)
    expect(minor({ ResolutionEnded: { object: 4, outcome: 'Fizzled' } })).toBe(false)
  })

  it('covers every variant of both unions, one line each', () => {
    const lines: EventView[] = [
      ...Object.values(PUBLIC).map(([event]): EventView => ({ Public: event })),
      ...Object.values(HIDDEN).map(([event]): EventView => ({ Hidden: event })),
    ]
    expect(lines).toHaveLength(Object.keys(PUBLIC).length + Object.keys(HIDDEN).length)
    for (const line of lines) expect(describeEvent(line, names).text.length).toBeGreaterThan(3)
  })

  it('names an object it does not know "a card" rather than print an id', () => {
    expect(say({ SpellCast: { player: 0, object: 99 } })).toBe('Ana casts a card')
    expect(say({ DamageDealt: { source: 99, target: { Object: 98 }, amount: 1 } })).toBe(
      'a card deals 1 damage to a card',
    )
  })

  it('falls back to "Seat n" for a seat with no name', () => {
    expect(say({ PriorityPassed: { player: 7 } })).toBe('Seat 8 passes priority')
  })
})

describe('minor lines', () => {
  const QUIET: Event[] = [
    { PriorityGranted: { player: 0 } },
    { PriorityPassed: { player: 0 } },
    { ManaAdded: { player: 0, mana: 'Red', source: null } },
    { ManaPoolEmptied: { player: 0 } },
    { PermanentUntapped: { object: 3 } },
    { Cycled: { player: 0, object: 2 } },
    { StepBegan: { step: 'Upkeep' } },
    { ShieldUsed: { id: 2, left: 3 } },
    'DamageCleared',
  ]
  const LOUD: Event[] = [
    { SpellCast: { player: 0, object: 4 } },
    { CreatureDied: { object: 1, cause: 'ZeroToughness' } },
    { Regenerated: { object: 1 } },
    { TurnBegan: { number: 1, active: 0 } },
    { PlayerLost: { player: 1, reason: 'Conceded' } },
    { CountersChanged: { on: { Player: 1 }, counter: 'poison', delta: 1 } },
  ]

  it.each(QUIET)('draws %j quietly', (event) => {
    expect(describeEvent({ Public: event }, names).minor).toBe(true)
  })

  it.each(LOUD)('draws %j in full', (event) => {
    expect(describeEvent({ Public: event }, names).minor).toBe(false)
  })
})

describe('events with more than one shape', () => {
  it('tells attackers and blockers apart in NothingToDo', () => {
    expect(say({ NothingToDo: { player: 1, kind: 'Block' } })).toBe('Bo has no possible blockers')
  })

  it('leaves out the source of mana that has none', () => {
    expect(say({ ManaAdded: { player: 0, mana: 'Blue', source: null } })).toBe('Ana adds {U}')
  })

  it('says when nobody attacks or blocks', () => {
    expect(say({ AttackersDeclared: { player: 1, attackers: [] } })).toBe('Bo declares no attackers')
    expect(say({ BlockersDeclared: { player: 0, blocks: [] } })).toBe('Ana declares no blockers')
  })

  it('groups attackers by what they attack, in the order the targets appear', () => {
    expect(
      say({
        AttackersDeclared: {
          player: 2,
          attackers: [
            [1, { Player: 0 }],
            [5, { Planeswalker: 6 }],
            [2, { Player: 0 }],
          ],
        },
      }),
    ).toBe('Cy attacks Ana with Grizzly Bears and Hill Giant; Jace Beleren with Serra Angel')
  })

  it('lists several blocks', () => {
    expect(
      say({
        BlockersDeclared: {
          player: 0,
          blocks: [
            [2, 1],
            [5, 1],
          ],
        },
      }),
    ).toBe('Ana declares blockers: Hill Giant blocks Grizzly Bears and Serra Angel blocks Grizzly Bears')
  })

  it('says a creature with no toughness left died of that', () => {
    expect(say({ CreatureDied: { object: 1, cause: 'ZeroToughness' } })).toBe(
      'Grizzly Bears dies with zero toughness',
    )
  })

  it.each([
    ['LethalDamage', 'Grizzly Bears dies from lethal damage'],
    ['Deathtouch', 'Grizzly Bears dies from deathtouch'],
    ['ZeroToughness', 'Grizzly Bears dies with zero toughness'],
  ] as const)('says why a creature died of %s', (cause, text) => {
    expect(say({ CreatureDied: { object: 1, cause } })).toBe(text)
  })

  it.each([
    ['ZeroLife', 'Bo ran out of life'],
    ['EmptyLibrary', 'Bo could not draw from an empty library'],
    ['Poison', 'Bo had too many poison counters'],
    ['Conceded', 'Bo conceded'],
    ['OpponentReachedLife', 'Bo lost to a player at the winning life total'],
    ['CommanderDamage', 'Bo was dealt 21 or more combat damage by one commander'],
  ] as const)('says why a player lost: %s', (reason, text) => {
    expect(say({ PlayerLost: { player: 1, reason } })).toBe(text)
  })

  it("says a format loss in the format's words", () => {
    expect(say({ PlayerLost: { player: 1, reason: { Format: 'commander damage' } } })).toBe(
      'Bo lost: commander damage',
    )
  })

  it('says a draw', () => {
    expect(say({ GameEnded: { result: 'Draw' } })).toBe('The game ends in a draw')
  })

  it.each([
    [
      effect('AllCreatures', { AddType: 'Artifact' }, 'UntilEndOfTurn'),
      'All creatures become artifact too until end of turn',
    ],
    [
      effect({ CreaturesControlledBy: 1 }, { SetColors: 0b00101 }, 'WhileSourceOnBattlefield', {
        id: 6,
        counter: 0,
      }),
      'Creatures controlled by Bo become white and black while Jace Beleren stays',
    ],
    [
      effect(objects(2), { SetColors: 0 }, 'UntilEndOfTurn'),
      'Hill Giant becomes colorless until end of turn',
    ],
    [
      effect(objects(2), { SetPowerToughness: { power: 0, toughness: 1 } }, 'UntilEndOfTurn'),
      'Hill Giant becomes 0/1 until end of turn',
    ],
    [
      effect(objects(2), { ModifyPowerToughness: { power: 1, toughness: 1 } }, 'UntilEndOfTurn'),
      'Hill Giant gets +1/+1 until end of turn',
    ],
    [
      effect(objects(2), { AddKeyword: 'Haste' }, 'UntilEndOfTurn'),
      'Hill Giant gains haste until end of turn',
    ],
    [
      effect(objects(1, 2), { AddKeyword: 'FirstStrike' }, 'UntilEndOfCombat'),
      'Grizzly Bears and Hill Giant gain first strike until end of combat',
    ],
    [effect(objects(2), { AddKeyword: { Ward: '{2}' } }, 'Forever'), 'Hill Giant gains ward'],
    [
      effect(objects(2), { RemoveKeyword: 'Flying' }, 'UntilEndOfTurn'),
      'Hill Giant loses flying until end of turn',
    ],
    [effect('AllCreatures', 'LoseAbilities', 'Forever'), 'All creatures lose all abilities'],
    [
      effect(objects(2), 'SwitchPowerToughness', 'UntilEndOfTurn'),
      'Hill Giant has power and toughness switched until end of turn',
    ],
    [effect(objects(2), 'CantBlock', 'UntilEndOfTurn'), "Hill Giant can't block until end of turn"],
    [effect(objects(2), 'CantBeBlocked', 'UntilEndOfTurn'), "Hill Giant can't be blocked until end of turn"],
    [
      effect(objects(2), 'AddAllCreatureTypes', 'UntilEndOfTurn'),
      'Hill Giant becomes every creature type until end of turn',
    ],
    [
      effect(
        objects(2),
        { CantBeBlockedBy: { by: [], controller: 0, source: { id: 1, counter: 0 } } },
        'UntilEndOfTurn',
      ),
      "Hill Giant can't be blocked by some creatures until end of turn",
    ],
    [
      effect(objects(2), { AddSubtype: 'Elf' }, 'UntilEndOfTurn'),
      'Hill Giant becomes Elf too until end of turn',
    ],
    [
      effect(objects(2), { SetCreatureTypes: ['Weird'] }, 'UntilEndOfTurn'),
      'Hill Giant becomes Weird until end of turn',
    ],
    [
      effect(objects(2), { SetController: 1 }, 'UntilEndOfTurn'),
      'Hill Giant changes controller until end of turn',
    ],
    [
      effect(objects(1, 2), { ModifyPowerToughness: { power: 3, toughness: 3 } }, 'UntilYourNextTurn'),
      "Grizzly Bears and Hill Giant get +3/+3 until their controller's next turn",
    ],
  ])('says what an effect does (%#)', (created, text) => {
    expect(say({ EffectCreated: { effect: created } })).toBe(text)
  })

  it.each([
    [
      ruleEffect({ MustAttack: [{ id: 1, counter: 0 }] }, 'UntilEndOfTurn'),
      'Grizzly Bears must attack if able until end of turn',
    ],
    [
      ruleEffect(
        {
          MustBlock: [
            { id: 1, counter: 0 },
            { id: 2, counter: 0 },
          ],
        },
        'UntilEndOfCombat',
      ),
      'Grizzly Bears and Hill Giant must block if able until end of combat',
    ],
    [
      ruleEffect({ CantBeCountered: [{ id: 4, counter: 0 }] }, 'Forever'),
      "Lightning Bolt can't be countered",
    ],
    [
      ruleEffect({ CantDraw: [1] }, 'UntilYourNextTurn'),
      "Bo can't draw cards until their controller's next turn",
    ],
    [
      ruleEffect({ CantSearch: [0, 1, 2] }, 'WhileSourceOnBattlefield', { id: 6, counter: 0 }),
      "Ana, Bo and Cy can't search libraries while Jace Beleren stays",
    ],
    [
      ruleEffect({ RevealTop: [2] }, 'UntilEndOfTurn'),
      'Cy play with the top card of their library revealed until end of turn',
    ],
    [
      ruleEffect(
        {
          MayCast: {
            who: [0],
            what: [],
            from: 'Exile',
            free: false,
            flash: false,
            only: [{ id: 4, counter: 1 }],
            play: true,
          },
        },
        'UntilEndOfTurn',
      ),
      'Ana may play Lightning Bolt until end of turn',
    ],
    [
      ruleEffect(
        {
          MayCast: {
            who: [0],
            what: [],
            from: 'Exile',
            free: false,
            flash: false,
            only: [
              { id: 1, counter: 1 },
              { id: 2, counter: 1 },
            ],
            play: false,
          },
        },
        'UntilEndOfYourNextTurn',
      ),
      "Ana may cast Grizzly Bears and Hill Giant until the end of their controller's next turn",
    ],
    [ruleEffect({ TriggerAgain: null }, 'Forever'), 'Triggered abilities trigger an additional time'],
  ])('says what a change to a rule does (%#)', (created, text) => {
    expect(say({ RuleCreated: { effect: created } })).toBe(text)
  })
})

describe('joinWords', () => {
  it.each([
    [[], ''],
    [['A'], 'A'],
    [['A', 'B'], 'A and B'],
    [['A', 'B', 'C'], 'A, B and C'],
  ])('joins %j as "%s"', (items, text) => {
    expect(joinWords(items)).toBe(text)
  })
})
