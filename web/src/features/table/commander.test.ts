// Goal: the words and figures of Commander at the table are pure: the tax chip shows only once a commander was cast or carries tax, the cost of a cast from the command zone adds the tax, damage levels follow the lethal amount of the table (21 or a house rule), the return question names where a no leaves the commander, and the log line of a hit carries the running total, and the colour identity of a seat's commanders reads off their colours and costs.
import { describe, expect, it } from 'vitest'
import type { Color, ManaCost, ObjectView, Zone } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import {
  castCostLine,
  commanderDamageLine,
  commanderDestination,
  damageFraction,
  damageLevel,
  damageWords,
  identityOf,
  returnDetail,
  returnNoLabel,
  returnQuestion,
  shortName,
  taxChip,
  taxWords,
} from './commander'
import type { CommanderDestination, DamageLevel } from './commander'

const ATRAXA: ManaCost = [
  { Colored: 'White' },
  { Colored: 'Blue' },
  { Colored: 'Black' },
  { Colored: 'Green' },
]
const KRENKO: ManaCost = [{ Generic: 2 }, { Colored: 'Red' }, { Colored: 'Red' }]
const ESPER: ManaCost = [{ Generic: 2 }, { Colored: 'White' }, { Colored: 'Blue' }, { Colored: 'Black' }]

describe('the tax chip', () => {
  it.each([
    [0, 0, null],
    [1, 2, 'Cast 1× · tax {2}'],
    [2, 4, 'Cast 2× · tax {4}'],
    [3, 6, 'Cast 3× · tax {6}'],
    // A table that waives the tax still counts the casts.
    [2, 0, 'Cast 2× · tax {0}'],
  ])('%i casts with tax %i reads %s', (casts, tax, text) => {
    expect(taxChip({ casts, tax })).toBe(text)
  })

  it.each([
    [1, 2, 'Atraxa: cast 1 time from the command zone, tax 2 mana'],
    [2, 4, 'Atraxa: cast 2 times from the command zone, tax 4 mana'],
  ])('says %i casts in words', (casts, tax, words) => {
    expect(taxWords('Atraxa', { casts, tax })).toBe(words)
  })
})

describe('the cost of a cast from the command zone', () => {
  it.each<[ManaCost | null, number, string]>([
    [KRENKO, 0, '{2}{R}{R}'],
    [KRENKO, 2, '{2}{R}{R} + {2} tax'],
    [ATRAXA, 4, '{W}{U}{B}{G} + {4} tax'],
    [null, 2, '{2} tax'],
    [null, 0, ''],
  ])('%j with tax %i costs %s', (cost, tax, line) => {
    expect(castCostLine(cost, tax)).toBe(line)
  })
})

describe('commander damage', () => {
  it.each<[number, number | undefined, DamageLevel]>([
    [0, 21, 'plain'],
    [14, 21, 'plain'],
    [15, 21, 'warn'],
    [17, 21, 'warn'],
    [18, 21, 'danger'],
    [20, 21, 'danger'],
    [21, 21, 'lethal'],
    [25, 21, 'lethal'],
    // House rules: a lower or higher lethal amount moves every threshold with it.
    [9, 15, 'warn'],
    [12, 15, 'danger'],
    [15, 15, 'lethal'],
    [24, 30, 'warn'],
    [27, 30, 'danger'],
    [2, 5, 'danger'],
    [1, 5, 'warn'],
    // The rule turned off: damage is only counted.
    [40, undefined, 'plain'],
  ])('%i of %s is %s', (damage, lethal, level) => {
    expect(damageLevel(damage, lethal)).toBe(level)
  })

  it.each<[number, number | undefined, string, string]>([
    [14, 21, '14/21', '14 of 21 commander damage from Atraxa'],
    [21, 21, '21/21', '21 of 21 commander damage from Atraxa, lethal'],
    [10, 15, '10/15', '10 of 15 commander damage from Atraxa'],
    [7, undefined, '7', '7 commander damage from Atraxa'],
  ])('%i of %s reads %s', (damage, lethal, fraction, words) => {
    expect(damageFraction(damage, lethal)).toBe(fraction)
    expect(damageWords('Atraxa', damage, lethal)).toBe(words)
  })

  it.each<[number, number, number | undefined, string]>([
    [7, 14, 21, 'Atraxa deals 7 commander damage to Bo (14/21)'],
    [6, 21, 21, 'Atraxa deals 6 commander damage to Bo (21/21)'],
    [5, 5, undefined, 'Atraxa deals 5 commander damage to Bo (5)'],
  ])('a hit of %i to %i reads %s', (amount, total, lethal, text) => {
    expect(commanderDamageLine('Atraxa', amount, 'Bo', total, lethal)).toBe(text)
  })
})

describe('the return choice', () => {
  it.each<[string, Zone | undefined, CommanderDestination | null, string, string]>([
    [
      'Put Atraxa into the command zone?',
      { Graveyard: 0 },
      'graveyard',
      'No leaves it in your graveyard.',
      'Leave in graveyard',
    ],
    ['Put Atraxa into the command zone?', 'Exile', 'exile', 'No leaves it in exile.', 'Leave in exile'],
    [
      'Put Atraxa into the command zone instead of a hand?',
      'Battlefield',
      'hand',
      'No puts it into your hand.',
      'Put into hand',
    ],
    [
      'Put Atraxa into the command zone instead of a library?',
      'Battlefield',
      'library',
      'No puts it into your library.',
      'Put into library',
    ],
    [
      'Put Atraxa into the command zone instead of a graveyard?',
      'Battlefield',
      'graveyard',
      'No leaves it in your graveyard.',
      'Leave in graveyard',
    ],
    [
      'Put Atraxa into the command zone instead of exile?',
      'Stack',
      'exile',
      'No leaves it in exile.',
      'Leave in exile',
    ],
    ['', 'Battlefield', null, 'No leaves it where it is.', 'Leave it'],
  ])('%s from %j goes to %s', (text, zone, destination, detail, label) => {
    expect(commanderDestination(text, zone)).toBe(destination)
    expect(returnDetail(destination)).toBe(detail)
    expect(returnNoLabel(destination)).toBe(label)
  })

  it('asks in the name of the commander', () => {
    expect(returnQuestion('Atraxa, Praetors’ Voice')).toBe(
      'Return Atraxa, Praetors’ Voice to the command zone?',
    )
  })
})

describe('the colour identity of a seat', () => {
  const commander = (id: number, colors: Color[], cost: ManaCost | null): ObjectView =>
    card({ id, colors, mana_cost: cost, zone: { Command: 0 } })
  const seat = (...ids: number[]) => ({ commanders: ids.map((object) => ({ object, casts: 0, tax: 0 })) })
  const objects = (...list: ObjectView[]) => Object.fromEntries(list.map((object) => [object.id, object]))

  it.each<[string, ObjectView[], Color[]]>([
    ['an Esper commander', [commander(1, ['White', 'Blue', 'Black'], ESPER)], ['White', 'Blue', 'Black']],
    [
      'a colour only in the cost symbols',
      [commander(1, [], [{ Colored: 'Red' }, { Phyrexian: 'Green' }])],
      ['Red', 'Green'],
    ],
    [
      'hybrid symbols',
      [
        commander(
          1,
          [],
          [{ Hybrid: ['Blue', 'Red'] }, { MonoHybrid: 'White' }, { ColorlessHybrid: 'Black' }],
        ),
      ],
      ['White', 'Blue', 'Black', 'Red'],
    ],
    [
      'two commanders, in WUBRG order and once each',
      [commander(1, ['Green'], [{ Colored: 'Green' }]), commander(2, ['Blue', 'Green'], KRENKO)],
      ['Blue', 'Red', 'Green'],
    ],
    ['a colourless commander', [commander(1, [], [{ Generic: 7 }, 'Colorless'])], []],
    ['a commander with no cost', [commander(1, [], null)], []],
  ])('reads %s', (_what, list, expected) => {
    const ids = list.map((object) => object.id)
    expect(identityOf(seat(...ids), objects(...list))).toEqual(expected)
  })

  it('is unknown while no commander of the seat is in the view', () => {
    expect(identityOf(seat(), {})).toBeNull()
    expect(identityOf({}, {})).toBeNull()
    expect(identityOf(seat(9), {})).toBeNull()
  })
})

describe('a short name', () => {
  it.each([
    ['Atraxa, Praetors’ Voice', 'Atraxa'],
    ['Krenko, Mob Boss', 'Krenko'],
    ['Tymna the Weaver', 'Tymna the Weaver'],
    [', odd', ', odd'],
  ])('%s is %s', (name, short) => {
    expect(shortName(name)).toBe(short)
  })
})
