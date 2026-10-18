// Goal: every kind of log event, public and hidden, reads as one plain sentence with the right names, and the quiet ones (priority, mana, untaps) are marked minor.
import { describe, expect, it } from 'vitest'
import type { ContinuousEffect, Event, EventView, HiddenEvent } from '@/protocol/generated'
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
  LifePaid: [{ LifePaid: { player: 1, amount: 4 } }, 'Bo pays 4 life'],
  CardDrawn: [{ CardDrawn: { player: 0, object: 3 } }, 'Ana draws Forest'],
  DrewFromEmptyLibrary: [{ DrewFromEmptyLibrary: { player: 1 } }, 'Bo tries to draw from an empty library'],
  CardDiscarded: [{ CardDiscarded: { player: 0, object: 2 } }, 'Ana discards Hill Giant'],
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
        effect: effect(
          { Object: 1 },
          { ModifyPowerToughness: { power: 2, toughness: -1 } },
          'UntilEndOfTurn',
        ),
      },
    },
    'Grizzly Bears gets +2/-1 until end of turn',
  ],
  EffectExpired: [{ EffectExpired: { id: 1 } }, 'An effect ends'],
  CreatureDied: [
    { CreatureDied: { object: 1, cause: 'LethalDamage' } },
    'Grizzly Bears dies from lethal damage',
  ],
  PlayerLost: [{ PlayerLost: { player: 1, reason: 'ZeroLife' } }, 'Bo ran out of life'],
  ObjectLeftGame: [{ ObjectLeftGame: { object: 2 } }, 'Hill Giant leaves the game'],
  GameEnded: [{ GameEnded: { result: { Winner: 0 } } }, 'Ana wins the game'],
}

const HIDDEN: Record<VariantName<HiddenEvent>, [HiddenEvent, string]> = {
  CardDrawn: [{ CardDrawn: { player: 1 } }, 'Bo draws a card'],
  HandDealt: [{ HandDealt: { player: 1, count: 7 } }, 'Bo is dealt 7 cards'],
  LibraryShuffled: [{ LibraryShuffled: { player: 1 } }, 'Bo shuffles their library'],
  MulliganTaken: [{ MulliganTaken: { player: 1 } }, 'Bo takes a mulligan'],
  CardsBottomed: [
    { CardsBottomed: { player: 1, count: 1 } },
    'Bo puts 1 card on the bottom of their library',
  ],
  ObjectLeftGame: ['ObjectLeftGame', 'A card leaves the game'],
}

function effect(
  selector: ContinuousEffect['selector'],
  modification: ContinuousEffect['modification'],
  duration: ContinuousEffect['duration'],
): ContinuousEffect {
  return { id: 1, layer: 'PtModify', selector, modification, duration, timestamp: 1 }
}

const say = (event: Event): string => describeEvent({ Public: event }, names).text

describe('describeEvent', () => {
  it.each(Object.entries(PUBLIC))('says the public event %s in one sentence', (_name, [event, text]) => {
    expect(describeEvent({ Public: event }, names).text).toBe(text)
  })

  it.each(Object.entries(HIDDEN))('says the hidden event %s in one sentence', (_name, [event, text]) => {
    expect(describeEvent({ Hidden: event }, names).text).toBe(text)
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
    { StepBegan: { step: 'Upkeep' } },
    'DamageCleared',
  ]
  const LOUD: Event[] = [
    { SpellCast: { player: 0, object: 4 } },
    { CreatureDied: { object: 1, cause: 'ZeroToughness' } },
    { TurnBegan: { number: 1, active: 0 } },
    { PlayerLost: { player: 1, reason: 'Conceded' } },
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
    ['ZeroLife', 'Bo ran out of life'],
    ['EmptyLibrary', 'Bo could not draw from an empty library'],
    ['Conceded', 'Bo conceded'],
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
      effect({ CreaturesControlledBy: 1 }, { SetColors: 0b00101 }, { WhileSourceOnBattlefield: 6 }),
      'Creatures controlled by Bo become white and black while Jace Beleren stays',
    ],
    [
      effect({ Object: 2 }, { SetColors: 0 }, 'UntilEndOfTurn'),
      'Hill Giant becomes colorless until end of turn',
    ],
    [
      effect({ Object: 2 }, { SetPowerToughness: { power: 0, toughness: 1 } }, 'UntilEndOfTurn'),
      'Hill Giant becomes 0/1 until end of turn',
    ],
    [
      effect({ Object: 2 }, { ModifyPowerToughness: { power: 1, toughness: 1 } }, 'UntilEndOfTurn'),
      'Hill Giant gets +1/+1 until end of turn',
    ],
  ])('says what an effect does (%#)', (created, text) => {
    expect(say({ EffectCreated: { effect: created } })).toBe(text)
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
