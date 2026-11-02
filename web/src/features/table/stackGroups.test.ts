// Goal: a crowded stack folds runs of the same ability of one source (and copies of one spell) into groups with their members top first and a key that survives the top ones resolving, never folds spells, other controllers, other texts or items apart, and the strip keeps as many chips as fit its width, the top one always.
import { describe, expect, it } from 'vitest'
import type { ObjectView } from '@/protocol/generated'
import { chipsThatFit, groupStack } from './stackGroups'

const item = (id: number, extra: Partial<ObjectView> = {}): ObjectView => ({
  id,
  owner: 0,
  controller: 0,
  zone: 'Stack',
  name: 'Soul Warden',
  mana_cost: null,
  supertypes: [],
  types: [],
  subtypes: [],
  colors: [],
  power: null,
  toughness: null,
  abilities: [],
  permanent: null,
  kind: 'Ability',
  source: 100,
  rules_text: 'You gain 1 life.',
  ...extra,
})

const spell = (id: number, name = 'Lightning Bolt'): ObjectView =>
  item(id, { kind: 'Card', name, source: undefined, rules_text: undefined })

const byId = (objects: ObjectView[]): Record<number, ObjectView> =>
  Object.fromEntries(objects.map((object) => [object.id, object]))

const copy = (id: number): ObjectView =>
  item(id, { kind: 'Card', name: 'Lightning Bolt', source: undefined, rules_text: undefined, copy: { of: 1 } })

describe('groupStack', () => {
  it.each<[string, ObjectView[], number[], number[][]]>([
    ['six triggers of one source', [1, 2, 3, 4, 5, 6].map((id) => item(id)), [1, 2, 3, 4, 5, 6], [[1, 2, 3, 4, 5, 6]]],
    ['two spells of the same name', [spell(1), spell(2)], [1, 2], [[1], [2]]],
    ['copies of one spell', [spell(1), copy(2), copy(3)], [1, 2, 3], [[1], [2, 3]]],
    ['the same text of two sources', [item(1), item(2, { source: 101 })], [1, 2], [[1], [2]]],
    ['two texts of one source', [item(1), item(2, { rules_text: 'Draw a card.' })], [1, 2], [[1], [2]]],
    ['two controllers', [item(1), item(2, { controller: 1 })], [1, 2], [[1], [2]]],
    ['a run broken by a spell', [item(1), item(2), spell(3), item(4)], [1, 2, 3, 4], [[1, 2], [3], [4]]],
    ['an object not in the view', [item(1)], [1, 9], [[1], [9]]],
  ])('%s', (_case, objects, ids, expected) => {
    expect(groupStack(ids, byId(objects)).map((group) => group.ids)).toEqual(expected)
  })

  it('keys a group by its bottom member, so the key stays as the top ones resolve', () => {
    const objects = [1, 2, 3].map((id) => item(id))
    const before = groupStack([1, 2, 3], byId(objects))
    const after = groupStack([2, 3], byId(objects))
    expect(before[0]?.key).toBe(after[0]?.key)
  })
})

describe('chipsThatFit', () => {
  it.each<[string, number[], number, number]>([
    ['all fit', [80, 170, 260], 300, 3],
    ['the last one runs over', [80, 170, 260], 200, 2],
    ['an edge on the line fits', [80, 170], 170, 2],
    ['even the top one is too wide', [400, 500], 300, 1],
    ['nothing laid out', [0, 0, 0], 0, 3],
    ['no chips', [], 300, 0],
  ])('%s', (_case, rights, width, expected) => {
    expect(chipsThatFit(rights, width)).toBe(expected)
  })
})
