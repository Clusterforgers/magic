// Goal: the hand's auto sorts put each card in the first of the account's type groups it belongs to, each group by mana value and name, a custom order keeps its places while new cards follow at the end and gone cards drop out, and a move or a stored order out of range changes nothing.
import { describe, expect, it } from 'vitest'
import type { CardType } from '@/protocol/generated'
import {
  DEFAULT_HAND_SORT,
  handGroup,
  mergeCustom,
  moveCard,
  moveGroup,
  parseCustom,
  sortHand,
} from './handOrder'
import type { HandFacts, HandGroup, HandSort } from './handOrder'

const CARDS: Record<number, HandFacts> = {
  1: { types: ['Instant'], manaValue: 1, name: 'Shock' },
  2: { types: ['Land'], manaValue: 0, name: 'Mountain' },
  3: { types: ['Creature'], manaValue: 3, name: 'Centaur' },
  4: { types: ['Sorcery'], manaValue: 1, name: 'Duress' },
  5: { types: ['Land', 'Creature'], manaValue: 0, name: 'Dryad Arbor' },
  6: { types: ['Artifact', 'Creature'], manaValue: 1, name: 'Ornithopter' },
  7: { types: ['Land'], manaValue: 0, name: 'Forest' },
  8: { types: ['Enchantment'], manaValue: 4, name: 'Bloom' },
}
const facts = (id: number) => CARDS[id]

const DEFAULT = DEFAULT_HAND_SORT.groups
const SPELLS_FIRST: HandGroup[] = [
  'Instant',
  'Sorcery',
  'Creature',
  'Land',
  'Artifact',
  'Enchantment',
  'Planeswalker',
  'Battle',
]
const sort = (by: HandSort['by'], groups: readonly HandGroup[] = DEFAULT): HandSort => ({
  by,
  groups: [...groups],
})

describe('sortHand', () => {
  it.each<[string, HandSort, number[], number[]]>([
    ['as drawn', sort('Drawn'), [1, 2, 3, 4], [1, 2, 3, 4]],
    ['by type in the starting order', sort('Groups'), [1, 2, 3, 4, 5, 6, 7, 8], [5, 7, 2, 6, 3, 8, 1, 4]],
    [
      'by type, spells first',
      sort('Groups', SPELLS_FIRST),
      [1, 2, 3, 4, 5, 6, 7, 8],
      [1, 4, 5, 6, 3, 7, 2, 8],
    ],
    ['by mana value', sort('ManaValue'), [1, 2, 3, 4, 5, 6, 7, 8], [5, 7, 2, 4, 6, 1, 3, 8]],
    ['an empty hand', sort('Groups'), [], []],
    // An unknown card sorts after every group, with mana value 0 and no name, in server order.
    ['unknown cards', sort('Groups'), [1, 99, 98, 2], [2, 1, 99, 98]],
  ])('%s', (_, how, ids, want) => {
    expect(sortHand(ids, facts, how)).toEqual(want)
  })

  it.each<[CardType[], readonly HandGroup[], number]>([
    [['Land'], DEFAULT, 0],
    [['Land', 'Creature'], DEFAULT, 0],
    [['Land', 'Creature'], SPELLS_FIRST, 2],
    [['Artifact', 'Creature'], DEFAULT, 1],
    [['Artifact', 'Creature'], ['Land', 'Artifact', 'Creature'], 1],
    [['Planeswalker'], DEFAULT, 4],
    [['Kindred' as CardType], DEFAULT, 8],
  ])('%j among %j is group %i', (types, groups, group) => {
    expect(handGroup(types, groups)).toBe(group)
  })
})

describe('moveGroup', () => {
  it.each<[number, number, HandGroup[]]>([
    [1, 0, ['Creature', 'Land', 'Artifact']],
    [0, 2, ['Creature', 'Artifact', 'Land']],
    [0, 3, ['Land', 'Creature', 'Artifact']],
    [-1, 0, ['Land', 'Creature', 'Artifact']],
  ])('moves %i to %i', (from, to, want) => {
    expect(moveGroup(['Land', 'Creature', 'Artifact'], from, to)).toEqual(want)
  })
})

describe('mergeCustom', () => {
  it.each<[string, number[], number[], number[]]>([
    ['same cards', [3, 1, 2], [1, 2, 3], [3, 1, 2]],
    ['a drawn card goes to the end', [3, 1, 2], [1, 2, 3, 4], [3, 1, 2, 4]],
    ['two drawn cards keep server order', [2, 1], [1, 2, 5, 4], [2, 1, 5, 4]],
    ['a played card drops out', [3, 1, 2], [1, 3], [3, 1]],
    ['both at once', [3, 1, 2], [2, 3, 9], [3, 2, 9]],
    ['an empty custom order is the server order', [], [4, 5], [4, 5]],
    ['duplicates are kept once', [2, 2, 1], [1, 2], [2, 1]],
  ])('%s', (_, custom, ids, want) => {
    expect(mergeCustom(custom, ids)).toEqual(want)
  })
})

describe('moveCard', () => {
  it.each<[number[], number, number, number[]]>([
    [[1, 2, 3, 4], 0, 2, [2, 3, 1, 4]],
    [[1, 2, 3, 4], 3, 0, [4, 1, 2, 3]],
    [[1, 2, 3, 4], 1, 1, [1, 2, 3, 4]],
    [[1, 2, 3, 4], -1, 2, [1, 2, 3, 4]],
    [[1, 2, 3, 4], 0, 4, [1, 2, 3, 4]],
  ])('%j from %i to %i is %j', (order, from, to, want) => {
    expect(moveCard(order, from, to)).toEqual(want)
  })
})

describe('parseCustom', () => {
  it.each<[string | null, number[] | null]>([
    ['[3,1,2]', [3, 1, 2]],
    ['[]', []],
    [null, null],
    ['', null],
    ['{"a":1}', null],
    ['[1,"2"]', null],
    ['[1.5]', null],
  ])('%j reads as %j', (raw, want) => {
    expect(parseCustom(raw)).toEqual(want)
  })
})
