// Goal: applyBatch, fromFirstBatch and fromSnapshot give the view the reference fold gives, sort objects by id, never mutate their inputs, and keep the identity of everything that did not change.
import { describe, expect, it } from 'vitest'
import type { EventBatch, StateView, TableView } from '@/protocol/generated'
import { batchOf, clone, deepFreeze, must, relabel, tableOf, withTapped } from '@/test-utils/batches'
import { expectFold, reference } from '@/test-utils/foldChecks'
import { lateJoin } from '@/test-utils/transcripts'
import { applyBatch, fromFirstBatch, fromSnapshot, indexById, sameJson } from './view'

const base: StateView = lateJoin.expected
const permanent = must(
  base.objects.find((object) => object.permanent !== null),
  'permanent',
)
const inHand = must(
  base.objects.find((object) => typeof object.zone === 'object' && 'Hand' in object.zone),
  'card in hand',
)
const [first, second] = base.objects
const missingId = 5000

const tableWith = (view: StateView, change: (table: TableView) => void): TableView => {
  const table = clone(tableOf(view))
  change(table)
  return table
}

interface Case {
  name: string
  batch: (view: StateView) => EventBatch
  check: (next: StateView, view: StateView) => void
}

const cases: Case[] = [
  {
    name: 'a batch that changes nothing gives back the very same view',
    batch: (view) => batchOf(view),
    check: (next, view) => expect(next).toBe(view),
  },
  {
    name: 'a life total changes one player and nothing else',
    batch: (view) =>
      batchOf(view, {
        table: tableWith(view, (table) => {
          must(table.players[0], 'player').life = 7
        }),
      }),
    check: (next, view) => {
      expect(next).not.toBe(view)
      expect(next.players[0]?.life).toBe(7)
      expect(next.players[1]).toBe(view.players[1])
      expect(next.players[0]?.hand).toBe(view.players[0]?.hand)
      expect(next.objects).toBe(view.objects)
      expect(next.battlefield).toBe(view.battlefield)
    },
  },
  {
    name: 'a new step replaces the turn and keeps every player',
    batch: (view) =>
      batchOf(view, {
        table: tableWith(view, (table) => {
          table.turn.step = 'End'
        }),
      }),
    check: (next, view) => {
      expect(next.turn.step).toBe('End')
      expect(next.players).toBe(view.players)
      expect(next.objects).toBe(view.objects)
    },
  },
  {
    name: 'a bigger battlefield replaces that zone and not the stack',
    batch: (view) =>
      batchOf(view, {
        table: tableWith(view, (table) => {
          table.battlefield.count += 1
          table.battlefield.cards?.push(missingId)
        }),
      }),
    check: (next, view) => {
      expect(next.battlefield).not.toBe(view.battlefield)
      expect(next.stack).toBe(view.stack)
      expect(next.exile).toBe(view.exile)
    },
  },
  {
    name: 'one changed object replaces that object and keeps all the others',
    batch: (view) => batchOf(view, { objects: [withTapped(permanent, !permanent.permanent?.tapped)] }),
    check: (next, view) => {
      const index = view.objects.findIndex((object) => object.id === permanent.id)
      expect(next.objects).not.toBe(view.objects)
      expect(next.objects[index]).not.toBe(view.objects[index])
      expect(next.objects[index]?.permanent?.tapped).toBe(!permanent.permanent?.tapped)
      // The parts of the changed object that did not change are shared too.
      expect(next.objects[index]?.types).toBe(view.objects[index]?.types)
      expect(next.objects[index]?.abilities).toBe(view.objects[index]?.abilities)
      expect(next.players).toBe(view.players)
    },
  },
  {
    name: 'an object listed again exactly as it is changes nothing',
    batch: (view) => batchOf(view, { objects: [clone(permanent)] }),
    check: (next, view) => expect(next).toBe(view),
  },
  {
    name: 'a new object with an id between two others lands between them',
    batch: (view) => batchOf(view, { objects: [relabel(inHand, 3, 'newcomer')] }),
    check: (next, view) => {
      expect(next.objects).toHaveLength(view.objects.length + 1)
      expect(next.objects.map((object) => object.id).slice(0, 3)).toEqual([2, 3, 4])
    },
  },
  {
    name: 'a new object with the highest id goes last',
    batch: (view) => batchOf(view, { objects: [relabel(inHand, 1000)] }),
    check: (next) => expect(next.objects.at(-1)?.id).toBe(1000),
  },
  {
    name: 'several new objects listed out of order come out in id order',
    batch: (view) =>
      batchOf(view, { objects: [relabel(inHand, 1000), relabel(inHand, 1), relabel(inHand, 3)] }),
    check: (next) => {
      const ids = next.objects.map((object) => object.id)
      expect(ids.slice(0, 4)).toEqual([1, 2, 3, 4])
      expect(ids.at(-1)).toBe(1000)
    },
  },
  {
    name: 'the same id listed twice keeps the later entry',
    batch: (view) => batchOf(view, { objects: [relabel(inHand, 3, 'first'), relabel(inHand, 3, 'second')] }),
    check: (next) => {
      const found = next.objects.filter((object) => object.id === 3)
      expect(found.map((object) => object.name)).toEqual(['second'])
    },
  },
  {
    name: 'gone ids leave the view and the rest keep their identity',
    batch: (view) => batchOf(view, { gone: [permanent.id, inHand.id] }),
    check: (next, view) => {
      expect(next.objects).toHaveLength(view.objects.length - 2)
      expect(next.objects.some((object) => object.id === permanent.id)).toBe(false)
    },
  },
  {
    name: 'a gone id the view never had is ignored',
    batch: (view) => batchOf(view, { gone: [missingId] }),
    check: (next, view) => expect(next).toBe(view),
  },
  {
    name: 'an object that is both listed and gone ends up gone, as in the Rust fold',
    batch: (view) => batchOf(view, { objects: [withTapped(permanent, true)], gone: [permanent.id] }),
    check: (next) => expect(next.objects.some((object) => object.id === permanent.id)).toBe(false),
  },
  {
    name: 'everything at once',
    batch: (view) =>
      batchOf(view, {
        table: tableWith(view, (table) => {
          must(table.players[1], 'player').life = 3
          table.turn.number += 1
        }),
        objects: [withTapped(permanent, !permanent.permanent?.tapped), relabel(inHand, 2000)],
        gone: [must(second, 'object').id],
      }),
    check: (next, view) => {
      expect(next.players[0]).toBe(view.players[0])
      expect(next.objects.at(-1)?.id).toBe(2000)
    },
  },
]

describe('applyBatch', () => {
  it.each(cases)('$name', ({ batch, check }) => {
    // Frozen inputs: any write to the view or the batch throws and fails the test.
    const view = deepFreeze(clone(base))
    const incoming = deepFreeze(batch(view))
    const before = JSON.stringify([view, incoming])
    const next = applyBatch(view, incoming)
    expectFold(next, view, incoming)
    check(next, view)
    expect(JSON.stringify([view, incoming])).toBe(before)
  })

  it('sorts the objects even when the view it starts from is not in id order', () => {
    const shuffled: StateView = { ...clone(base), objects: [...clone(base).objects].reverse() }
    const next = applyBatch(deepFreeze(shuffled), deepFreeze(batchOf(shuffled)))
    expect(next.objects.map((object) => object.id)).toEqual(base.objects.map((object) => object.id))
  })

  it('keeps two results of the same view apart', () => {
    const view = deepFreeze(clone(base))
    const one = applyBatch(view, batchOf(view, { objects: [relabel(inHand, 3)] }))
    const two = applyBatch(view, batchOf(view, { objects: [relabel(inHand, 5)] }))
    expect(one.objects).not.toBe(two.objects)
    expect(one.objects.some((object) => object.id === 5)).toBe(false)
    expect(two.objects.some((object) => object.id === 3)).toBe(false)
  })
})

describe('fromFirstBatch', () => {
  it.each([1, 2, 17, 1000])('is null for a batch at from_seq %i', (fromSeq) => {
    expect(fromFirstBatch(batchOf(base, { from_seq: fromSeq }))).toBeNull()
  })

  it('builds the view from the batch at from_seq 0: its table, its objects in id order, minus gone', () => {
    const objects = [relabel(inHand, 9), relabel(inHand, 2), relabel(inHand, 5), relabel(inHand, 7)]
    const batch = deepFreeze(batchOf(base, { from_seq: 0, objects, gone: [7, 404] }))
    const view = fromFirstBatch(batch)
    expect(view).toEqual(reference(null, batch))
    expect(view?.objects.map((object) => object.id)).toEqual([2, 5, 9])
    expect(view?.turn).toEqual(batch.table.turn)
  })

  it('does not keep the batch alive as its own object list', () => {
    const batch = batchOf(base, { from_seq: 0, objects: [clone(permanent)] })
    const view = fromFirstBatch(batch)
    expect(view?.objects).not.toBe(batch.objects)
  })
})

describe('fromSnapshot', () => {
  it('is the snapshot itself when there is no view yet', () => {
    expect(fromSnapshot(null, base)).toBe(base)
  })

  it('puts an unsorted snapshot in id order without touching it', () => {
    const snapshot = deepFreeze({ ...clone(base), objects: [...clone(base).objects].reverse() })
    expect(fromSnapshot(null, snapshot).objects.map((object) => object.id)).toEqual(
      base.objects.map((object) => object.id),
    )
    expect(fromSnapshot(clone(base), snapshot).objects.map((object) => object.id)).toEqual(
      base.objects.map((object) => object.id),
    )
  })

  it('gives back the view it replaces when the snapshot says the same thing', () => {
    const view = deepFreeze(clone(base))
    expect(fromSnapshot(view, clone(base))).toBe(view)
  })

  it('keeps every unchanged object, player and zone when one thing differs', () => {
    const view = deepFreeze(clone(base))
    const snapshot = clone(base)
    must(snapshot.players[1], 'player').life = 11
    const index = snapshot.objects.findIndex((object) => object.id === permanent.id)
    must(must(snapshot.objects[index], 'object').permanent, 'permanent').damage = 2
    const next = fromSnapshot(view, snapshot)
    expect(next).toEqual(snapshot)
    expect(next.players[0]).toBe(view.players[0])
    expect(next.players[1]).not.toBe(view.players[1])
    expect(next.players[1]?.hand).toBe(view.players[1]?.hand)
    next.objects.forEach((object, i) => {
      if (i === index) expect(object).not.toBe(view.objects[i])
      else expect(object).toBe(view.objects[i])
    })
  })

  it('drops objects the snapshot no longer has and takes in new ones', () => {
    const view = deepFreeze(clone(base))
    const snapshot = clone(base)
    snapshot.objects = [...snapshot.objects.slice(1), relabel(inHand, 9000)]
    const next = fromSnapshot(view, snapshot)
    expect(next.objects.map((object) => object.id)).toEqual(snapshot.objects.map((object) => object.id))
    expect(next.objects[0]).toBe(view.objects[1])
  })
})

describe('indexById', () => {
  it('finds every object by its id, as the same value', () => {
    const index = indexById(base.objects)
    expect(Object.keys(index)).toHaveLength(base.objects.length)
    for (const object of base.objects) expect(index[object.id]).toBe(object)
    expect(index[missingId]).toBeUndefined()
    expect(first && index[first.id]).toBe(first)
  })
})

describe('sameJson', () => {
  it.each<[string, unknown, unknown, boolean]>([
    ['equal numbers', 1, 1, true],
    ['different numbers', 1, 2, false],
    ['a number and its text', 1, '1', false],
    ['null and undefined', null, undefined, false],
    ['null and an empty object', null, {}, false],
    ['object keys in another order', { a: 1, b: [1, 2] }, { b: [1, 2], a: 1 }, true],
    ['a missing key', { a: 1 }, { a: 1, b: undefined }, false],
    ['a different key with the same count', { a: 1 }, { b: 1 }, false],
    ['arrays of different length', [1, 2], [1, 2, 3], false],
    ['arrays in another order', [1, 2], [2, 1], false],
    ['an array and an object', [], {}, false],
    ['nested equal values', { a: [{ b: null }] }, { a: [{ b: null }] }, true],
    ['nested different values', { a: [{ b: null }] }, { a: [{ b: 0 }] }, false],
  ])('%s', (_name, a, b, expected) => {
    expect(sameJson(a, b)).toBe(expected)
    expect(sameJson(b, a)).toBe(expected)
  })
})
