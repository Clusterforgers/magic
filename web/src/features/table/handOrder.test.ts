// Goal: the hand's auto sorts put lands, creatures and other spells in their groups by mana value and name, a custom order keeps its places while new cards follow at the end and gone cards drop out, and a move or a stored order out of range changes nothing.
import { describe, expect, it } from 'vitest'
import type { CardType } from '@/protocol/generated'
import { handGroup, mergeCustom, moveCard, parseCustom, sortHand } from './handOrder'
import type { HandFacts, HandSort } from './handOrder'

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

describe('sortHand', () => {
  it.each<[HandSort, number[], number[]]>([
    ['drawn', [1, 2, 3, 4], [1, 2, 3, 4]],
    ['types', [1, 2, 3, 4, 5, 6, 7, 8], [5, 7, 2, 6, 3, 4, 1, 8]],
    ['mana', [1, 2, 3, 4, 5, 6, 7, 8], [5, 7, 2, 4, 6, 1, 3, 8]],
    ['types', [], []],
    // An unknown card sorts as a spell of mana value 0 with no name, ahead of the known spells, in server order.
    ['types', [1, 99, 98, 2], [2, 99, 98, 1]],
  ])('%s sorts %j as %j', (sort, ids, want) => {
    expect(sortHand(ids, facts, sort)).toEqual(want)
  })

  it.each<[CardType[], number]>([
    [['Land'], 0],
    [['Land', 'Creature'], 0],
    [['Artifact', 'Creature'], 1],
    [['Planeswalker'], 2],
    [['Battle'], 2],
  ])('%j is group %i', (types, group) => {
    expect(handGroup(types)).toBe(group)
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
