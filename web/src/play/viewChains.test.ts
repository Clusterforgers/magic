// Goal: folding the recorded transcripts and long random chains of batches matches the reference fold at every step and keeps identity wherever nothing changed.
import { describe, expect, it } from 'vitest'
import type { EventBatch, ObjectView, StateView } from '@/protocol/generated'
import { batchOf, clone, deepFreeze, must, relabel, tableOf, withTapped } from '@/test-utils/batches'
import { expectFold } from '@/test-utils/foldChecks'
import { lateJoin, transcripts } from '@/test-utils/transcripts'
import { applyBatch, fromFirstBatch, fromSnapshot } from './view'

describe('the recorded transcripts', () => {
  it.each(transcripts)('$name: every step matches the reference fold and keeps what did not change', (t) => {
    let view: StateView | null = null
    let steps = 0
    for (const message of t.messages) {
      if (message.type === 'Snapshot') {
        view = fromSnapshot(view, message.state)
      } else if (message.type === 'Events') {
        const next: StateView =
          view === null ? must(fromFirstBatch(message), 'view at seq 0') : applyBatch(view, message)
        expectFold(next, view, message)
        view = next
        steps += 1
      }
    }
    expect(steps).toBe(40)
    expect(view).toEqual(t.expected)
  })

  it.each(transcripts)('$name: a batch only ever replaces the objects it lists', (t) => {
    let view: StateView | null = null
    let replaced = 0
    let kept = 0
    for (const message of t.messages) {
      if (message.type === 'Snapshot') {
        view = message.state
      } else if (message.type === 'Events') {
        const before: StateView | null = view
        const next: StateView =
          before === null ? must(fromFirstBatch(message), 'view') : applyBatch(before, message)
        const listed = new Set(message.objects.map((object) => object.id))
        const old = new Map(
          (before?.objects ?? []).map((object): [number, ObjectView] => [object.id, object]),
        )
        for (const object of next.objects) {
          if (listed.has(object.id)) replaced += 1
          else if (old.get(object.id) === object) kept += 1
        }
        view = next
      }
    }
    expect(replaced).toBeGreaterThan(0)
    expect(kept).toBeGreaterThan(replaced)
  })
})

/** A small seeded generator, so a failing chain can be replayed. */
function mulberry32(seed: number): () => number {
  let state = seed
  return () => {
    state = (state + 0x6d2b79f5) | 0
    let t = Math.imul(state ^ (state >>> 15), 1 | state)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

const template = must(
  lateJoin.expected.objects.find((object) => typeof object.zone === 'object' && 'Hand' in object.zone),
  'card in hand',
)

function randomBatch(view: StateView, random: () => number): EventBatch {
  const chance = (p: number): boolean => random() < p
  const table = clone(tableOf(view))
  if (chance(0.5)) {
    must(table.players[Math.floor(random() * table.players.length)], 'player').life += chance(0.5) ? 1 : -1
  }
  if (chance(0.3)) table.turn.number += 1
  if (chance(0.3)) table.battlefield.count += 1
  const objects: ObjectView[] = []
  const gone: number[] = []
  for (const object of view.objects) {
    const roll = random()
    if (roll < 0.15) {
      objects.push(
        object.permanent === null
          ? { ...clone(object), name: `${object.name}!` }
          : withTapped(object, !object.permanent.tapped),
      )
    } else if (roll < 0.2) {
      objects.push(clone(object)) // listed although nothing changed
    } else if (roll < 0.3) {
      gone.push(object.id)
    }
  }
  while (chance(0.4)) objects.push(relabel(template, Math.floor(random() * 300), 'newcomer'))
  if (chance(0.2)) gone.push(Math.floor(random() * 300)) // usually an id the view never had
  return batchOf(view, { table, objects, gone })
}

describe('random chains of batches', () => {
  it.each(Array.from({ length: 12 }, (_, index) => index + 1))(
    'seed %i: 60 steps match the reference fold, never write to their inputs and keep identity',
    (seed) => {
      const random = mulberry32(seed)
      let view = deepFreeze(clone(lateJoin.expected))
      let identical = 0
      for (let step = 0; step < 60; step += 1) {
        const batch = deepFreeze(randomBatch(view, random))
        const next = applyBatch(view, batch)
        expectFold(next, view, batch)
        if (next === view) identical += 1
        view = deepFreeze(next)
      }
      expect(view.objects.length).toBeGreaterThan(0)
      expect(identical).toBeLessThan(60)
    },
  )
})
