/** Goal: a source chip marks restricted mana `only`, the life and energy a use pays and the permanents or cards it takes, in a fixed order, and says when only some of its ways carry a mark. */
import { describe, expect, it } from 'vitest'
import type { ManaOption } from '@/protocol/generated'
import { source } from '@/test-utils/mana'
import { sourceMarks } from './sourceMarks'

const CREATURES: ManaOption['only'] = [{ Spell: { types: ['Creature'] } }]
const way = (extra: Partial<ManaOption>, letter = 'G', ability = 0): ManaOption => ({
  ...source(1, letter, ability),
  ...extra,
})

describe('sourceMarks', () => {
  it.each<[string, ManaOption[], [string, string][]]>([
    ['a plain land', [way({})], []],
    ['a rider alone', [way({ only: [{ When: { spell: {}, effects: [] } }] })], []],
    ['Ancient Ziggurat', [way({ only: CREATURES })], [['only', 'mana only for creature spells']]],
    [
      'restricted mana from one way of two',
      [way({}), way({ only: CREATURES }, 'R', 1)],
      [['only', 'some mana only for creature spells']],
    ],
    ['Mana Confluence', [way({ life: 1 }), way({ life: 1 }, 'R', 1)], [['1 life', 'pays 1 life']]],
    ['Aether Hub', [way({}, 'C'), way({ energy: 1 }, 'R', 1)], [['1 energy', 'pays 1 energy for some mana']]],
    ['Springleaf Drum', [way({ takes: [{ count: 1, from: [5, 6] }] })], [['tap 1', 'taps 1 more permanent']]],
    [
      'a sacrifice of two',
      [way({ takes: [{ count: 2, sacrifice: true, from: [5, 6] }] })],
      [['sacrifice 2', 'sacrifices 2 more permanents']],
    ],
    [
      'an exiled hand card',
      [way({ takes: [{ count: 1, exile: true, from: [7] }] })],
      [['exile 1', 'exiles 1 card from your hand']],
    ],
    [
      'every mark at once, in order',
      [way({ only: CREATURES, life: 2, energy: 3, takes: [{ count: 1, from: [5] }] })],
      [
        ['only', 'mana only for creature spells'],
        ['2 life', 'pays 2 life'],
        ['3 energy', 'pays 3 energy'],
        ['tap 1', 'taps 1 more permanent'],
      ],
    ],
  ])('%s', (_name, options, expected) => {
    expect(sourceMarks(options).map((mark) => [mark.short, mark.words])).toEqual(expected)
  })
})
