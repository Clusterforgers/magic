/** Goal: a cost pick or a target pick that takes an object which is both a target and paid for the cost of the same spell or ability is warned of in words naming the source, the zone left, the objects and whether every target is lost, for one and for several, from any zone; any other choice gets no warning. */
import { describe, expect, it } from 'vitest'
import type { ObjectId, Zone } from '@/protocol/generated'
import { clashes, clashWarning, leftZoneWords } from './costTarget'

const OBJECTS: Record<number, { name: string; zone: Zone }> = {
  1: { name: 'Wolf', zone: 'Battlefield' },
  2: { name: 'Ogre', zone: 'Battlefield' },
  3: { name: 'Bear', zone: { Graveyard: 0 } },
}
const objects = (id: ObjectId) => OBJECTS[id]

describe('clashes', () => {
  it.each([
    [[1, 2], [2], [2]],
    [[1, 2], [3], []],
    [
      [2, 1],
      [1, 2],
      [2, 1],
    ],
    [[1], undefined, []],
    [[], [1], []],
  ])('of %j with %j is %j', (chosen, clashing, expected) => {
    expect(clashes(chosen, clashing)).toEqual(expected)
  })
})

describe('leftZoneWords', () => {
  it.each<[Zone | undefined, string]>([
    ['Battlefield', 'the battlefield'],
    [undefined, 'the battlefield'],
    [{ Graveyard: 1 }, 'the graveyard'],
    [{ Hand: 0 }, 'the hand'],
    ['Exile', 'exile'],
    ['Stack', 'the stack'],
  ])('%j reads %s', (zone, words) => {
    expect(leftZoneWords(zone)).toBe(words)
  })
})

describe('clashWarning', () => {
  it.each([
    {
      chosen: [1],
      clashing: [1],
      all: true,
      text: "Goblin Bombardment's target leaves the battlefield: Wolf pays the cost. It will have no target.",
    },
    {
      chosen: [1, 2],
      clashing: [2],
      all: false,
      text: "Goblin Bombardment's target leaves the battlefield: Ogre pays the cost. It will lose that target.",
    },
    {
      chosen: [1, 2],
      clashing: [1, 2],
      all: false,
      text: "2 of Goblin Bombardment's targets leave the battlefield: Wolf and Ogre pay the cost. It will lose those targets.",
    },
    {
      chosen: [3],
      clashing: [3],
      all: true,
      text: "Goblin Bombardment's target leaves the graveyard: Bear pays the cost. It will have no target.",
    },
  ])('warns of $clashing among $chosen', ({ chosen, clashing, all, text }) => {
    const warning = clashWarning({ chosen, clashing, source: 'Goblin Bombardment', objects, all })
    expect(warning?.text).toBe(text)
    expect(warning?.ids).toEqual(clashing)
  })

  it.each([
    [[1], [2]],
    [[1], []],
    [[], [1]],
  ])('stays quiet for %j against %j', (chosen, clashing) => {
    expect(clashWarning({ chosen, clashing, source: 'X', objects, all: true })).toBeNull()
  })

  it('names an unknown source and object plainly', () => {
    const warning = clashWarning({ chosen: [9], clashing: [9], source: undefined, objects, all: true })
    expect(warning?.text).toBe(
      "The spell or ability's target leaves the battlefield: a card pays the cost. It will have no target.",
    )
  })
})
