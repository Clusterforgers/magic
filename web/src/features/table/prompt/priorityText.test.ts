/** Goal: the Priority prompt tells the player in plain words what they may do (play a land, cast a spell, tap for mana, pass), says when they are responding to a spell on the stack, and groups mana abilities by permanent. */
import { describe, expect, it } from 'vitest'
import type { ManaOption, PriorityOptions } from '@/protocol/generated'
import { manaByPermanent, priorityText } from './priorityText'
import type { StackTop } from './priorityText'

const NAMES = ['Ana', 'Bo', 'Cy']
const name = (seat: number): string => NAMES[seat] ?? `Seat ${seat}`
const options = (over: Partial<PriorityOptions> = {}): PriorityOptions => ({
  playable_lands: [],
  castable: [],
  mana_abilities: [],
  can_pass: true,
  ...over,
})
const mana: ManaOption = { source: { object: 5, ability: 0 }, produces: 'Green' }
const bolt: StackTop = { name: 'Lightning Bolt', controller: 0 }
const growth: StackTop = { name: 'Giant Growth', controller: 1 }

describe('priorityText', () => {
  it.each([
    [
      'nothing to do',
      options(),
      null,
      'You have priority. You have nothing to play right now; pass to go on.',
    ],
    ['a land', options({ playable_lands: [1] }), null, 'You have priority. You can play a land, or pass.'],
    ['a spell', options({ castable: [1] }), null, 'You have priority. You can cast a spell, or pass.'],
    ['mana', options({ mana_abilities: [mana] }), null, 'You have priority. You can tap for mana, or pass.'],
    [
      'a land and a spell',
      options({ playable_lands: [1], castable: [2] }),
      null,
      'You have priority. You can play a land or cast a spell, or pass.',
    ],
    [
      'everything',
      options({ playable_lands: [1], castable: [2], mana_abilities: [mana] }),
      null,
      'You have priority. You can play a land, cast a spell or tap for mana, or pass.',
    ],
    [
      'a land but no pass',
      options({ playable_lands: [1], can_pass: false }),
      null,
      'You have priority. You can play a land.',
    ],
    ['no move at all', options({ can_pass: false }), null, 'You have priority. You have no legal move.'],
    [
      'your own spell on the stack',
      options({ castable: [2] }),
      bolt,
      'You are responding to your Lightning Bolt. You can cast a spell, or pass to let it resolve.',
    ],
    [
      'an opponent spell on the stack',
      options({ castable: [2], mana_abilities: [mana] }),
      growth,
      "You are responding to Bo's Giant Growth. You can cast a spell or tap for mana, or pass to let it resolve.",
    ],
    [
      'a stack and nothing to cast',
      options(),
      growth,
      "You are responding to Bo's Giant Growth. You have nothing to play right now; pass to let it resolve.",
    ],
    [
      'a stack and no pass',
      options({ castable: [2], can_pass: false }),
      bolt,
      'You are responding to your Lightning Bolt. You can cast a spell.',
    ],
  ])('says it for %s', (_name, given, top, expected) => {
    expect(priorityText(given, top, 0, name)).toBe(expected)
  })
})

describe('manaByPermanent', () => {
  it('groups abilities by permanent, keeping the order of the options', () => {
    const second: ManaOption = { source: { object: 5, ability: 1 }, produces: 'Red' }
    const other: ManaOption = { source: { object: 9, ability: 0 }, produces: 'Blue' }
    const grouped = manaByPermanent([mana, other, second])
    expect([...grouped.keys()]).toEqual([5, 9])
    expect(grouped.get(5)).toEqual([mana, second])
    expect(grouped.get(9)).toEqual([other])
  })

  it('is empty without mana abilities', () => {
    expect(manaByPermanent([]).size).toBe(0)
  })
})
