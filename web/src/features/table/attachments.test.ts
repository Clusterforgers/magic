// Goal: an Aura, Equipment or Fortification sits in its host's row right behind it and moves with it, whatever kind of permanent it is, as long as the host is on the battlefield under the same controller; one that enchants another seat's permanent or a player stays in its own controller's row, a chain of attachments follows its first host, and the row of a permanent that is gone is the one that held it.
import { describe, expect, it } from 'vitest'
import type { PlayState } from '@/play/store'
import { indexById } from '@/play/view'
import type { ObjectView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { card, forest } from '@/test-utils/cards'
import { fixtureState } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import { battlefieldStacks, idsOfRows } from './rowStacks'
import { rowHolding, splitBattlefield } from './selectors'

const base = fixtureState(buildFixture({ seats: 2 }))

const bear = (id: number, over: Partial<ObjectView> = {}): ObjectView => card({ id, ...over })
const land = (id: number, over: Partial<ObjectView> = {}): ObjectView => forest({ id, ...over })
const aura = (id: number, over: Partial<ObjectView> = {}): ObjectView =>
  card({
    id,
    name: 'Pacifism',
    types: ['Enchantment'],
    subtypes: ['Aura'],
    power: null,
    toughness: null,
    ...over,
  })
const boots = (id: number, over: Partial<ObjectView> = {}): ObjectView =>
  card({
    id,
    name: 'Boots',
    types: ['Artifact'],
    subtypes: ['Equipment'],
    power: null,
    toughness: null,
    ...over,
  })

/** A play state whose battlefield holds exactly `objects`, in that order. */
function stateOf(objects: readonly ObjectView[]): PlayState {
  const view = must(base.view)
  return {
    ...base,
    view: {
      ...view,
      objects: [...objects],
      battlefield: { count: objects.length, cards: objects.map((o) => o.id) },
    },
    objectsById: indexById([...objects]),
  }
}

const rowsOf = (objects: readonly ObjectView[], seat = 0) => {
  const state = stateOf(objects)
  return must(splitBattlefield(must(state.view), state.objectsById).get(seat))
}

describe('where an attachment sits', () => {
  it('follows its host into the host row, right behind it, in the order of entering', () => {
    const rows = rowsOf([
      bear(1, { attachments: [10, 11] }),
      bear(2),
      aura(10, { attached_to: 1 }),
      boots(11, { attached_to: 1 }),
    ])
    expect(rows.creatures).toEqual([1, 10, 11, 2])
    expect(rows.other).toEqual([])
  })

  it('keeps a free permanent of the same kind in its own row', () => {
    const rows = rowsOf([bear(1, { attachments: [10] }), aura(10, { attached_to: 1 }), aura(12), boots(13)])
    expect(rows.creatures).toEqual([1, 10])
    expect(rows.other).toEqual([12, 13])
  })

  it('goes to the lands row behind a land it enchants', () => {
    const rows = rowsOf([land(1, { attachments: [10] }), aura(10, { attached_to: 1 })])
    expect(rows.lands).toEqual([1, 10])
    expect(rows.other).toEqual([])
  })

  it('follows a chain: an Aura on the Equipment that is on the creature', () => {
    const rows = rowsOf([
      aura(12, { attached_to: 11 }),
      bear(1, { attachments: [11] }),
      boots(11, { attached_to: 1, attachments: [12] }),
    ])
    expect(rows.creatures).toEqual([1, 11, 12])
  })

  it.each<[string, ObjectView[]]>([
    ['another seat controls the host', [bear(1, { controller: 1 }), aura(10, { attached_to: 1 })]],
    ['the host is not on the battlefield', [aura(10, { attached_to: 99 })]],
    ['it enchants a player', [aura(10, { attached_player: 1 })]],
    ['it names itself', [aura(10, { attached_to: 10 })]],
  ])('stays in its own row when %s', (_, objects) => {
    const rows = rowsOf(objects)
    expect(rows.other).toContain(10)
  })

  it('places a loop of attachments in their own rows instead of losing them', () => {
    const rows = rowsOf([aura(10, { attached_to: 11 }), boots(11, { attached_to: 10 })])
    expect([...rows.other].sort()).toEqual([10, 11])
  })

  it('names the row that holds an attachment, and its own row for a permanent not on the battlefield', () => {
    const state = stateOf([bear(1, { attachments: [10] }), aura(10, { attached_to: 1 })])
    const view = must(state.view)
    expect(rowHolding(view, state.objectsById, must(state.objectsById[10]))).toBe('creatures')
    expect(rowHolding(view, state.objectsById, aura(77))).toBe('other')
  })
})

describe('stacks keep attachments behind their host', () => {
  it('draws the equipped bear, its attachments and the free bears in battlefield order', () => {
    const state = stateOf([bear(1), bear(2, { attachments: [10] }), aura(10, { attached_to: 2 }), bear(3)])
    const rows = battlefieldStacks(state, 0).plain
    expect(idsOfRows(rows)).toEqual([1, 2, 10, 3])
    expect(rows.creatures.map((stack) => stack.ids)).toEqual([[1], [2], [10], [3]])
  })

  it('does not hide an equipped token behind a plain one', () => {
    const token = (id: number, over: Partial<ObjectView> = {}): ObjectView =>
      bear(id, { kind: 'Token', name: 'Soldier', ...over })
    const state = stateOf([
      token(1),
      token(2, { attachments: [10] }),
      boots(10, { attached_to: 2 }),
      token(3),
    ])
    const rows = battlefieldStacks(state, 0).plain
    expect(rows.creatures.map((stack) => stack.ids)).toEqual([[1, 3], [2], [10]])
  })
})
