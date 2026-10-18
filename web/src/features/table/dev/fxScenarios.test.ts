// Goal: every dev move (attack, block, damage, death, life, enter) builds a batch that follows on from the table, folds into a consistent table and derives the effects its name promises at any table size, and can be played again and again, each time on other creatures, without ever breaking the table or running out.
import { describe, expect, it } from 'vitest'
import { createPlayStore } from '@/play/store'
import type { PlayStore } from '@/play/store'
import type { StateView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { DEV_FX } from '../devSearch'
import type { DevFx } from '../devSearch'
import { deriveEffects } from '../fx/derive'
import { batchBetween } from '../fx/batch'
import type { Effect } from '../fx/model'
import { applyBatchToStore } from './applyBatch'
import { fixtureState } from './devGame'
import { buildFixture } from './fixture'
import { devBatch, playDevFx } from './fxScenarios'

function table(seats = 2): PlayStore {
  const store = createPlayStore()
  store.setState(fixtureState(buildFixture({ seats })))
  return store
}

/** Plays `kind` and returns the effects the effects layer would play for it. */
function effectsOf(store: PlayStore, kind: DevFx): Effect[] {
  const before = store.getState()
  expect(playDevFx(store, kind), `${kind} did nothing`).toBe(true)
  const step = must(batchBetween(before, store.getState()), 'a batch the effects layer sees')
  return deriveEffects(step.before, step.after, step.events)
}

const kinds = (effects: Effect[]): string[] => effects.map((effect) => effect.kind)

/** What every table must keep true however it came about. */
function expectConsistent(view: StateView): void {
  const byId = new Map(view.objects.map((object) => [object.id, object]))
  const battlefield = must(view.battlefield.cards)
  expect(view.battlefield.count).toBe(battlefield.length)
  expect(new Set(battlefield).size).toBe(battlefield.length)
  for (const id of battlefield) {
    const object = must(byId.get(id), `object ${id}`)
    expect(object.zone, `object ${id}`).toBe('Battlefield')
  }
  for (const player of view.players) {
    const yard = must(player.graveyard.cards)
    expect(player.graveyard.count).toBe(yard.length)
    for (const id of yard) expect(byId.get(id)?.zone).toEqual({ Graveyard: player.seat })
  }
  expect(view.objects.map((object) => object.id)).toEqual(
    view.objects.map((object) => object.id).toSorted((a, b) => a - b),
  )
  expect(new Set(view.objects.map((object) => object.id)).size).toBe(view.objects.length)
}

describe('each move', () => {
  it.each(DEV_FX)('%s builds a batch that follows on from the table', (kind) => {
    const store = table()
    const state = store.getState()
    const batch = must(devBatch(kind, state))
    expect(batch.from_seq).toBe(state.seq)
    expect(batch.events.length).toBeGreaterThan(0)
    expect(batch.gone).toEqual([])
    expect(batch.events.every((event) => 'Public' in event)).toBe(true)
    // It lists only objects that exist and that it changed.
    const ids = new Set(must(state.view).objects.map((object) => object.id))
    for (const object of batch.objects) {
      const before = state.objectsById[object.id]
      if (before !== undefined) expect(object).not.toEqual(before)
      else expect(ids.has(object.id)).toBe(false)
    }
  })

  it.each(DEV_FX)('%s leaves the table it is played on untouched until it is applied', (kind) => {
    const store = table()
    const before = structuredClone(store.getState().view)
    devBatch(kind, store.getState())
    expect(store.getState().view).toEqual(before)
  })

  it('does nothing without a table', () => {
    const store = createPlayStore()
    for (const kind of DEV_FX) expect(playDevFx(store, kind)).toBe(false)
  })

  it('is refused when the batch does not follow on', () => {
    const store = table()
    const batch = must(devBatch('life', store.getState()))
    expect(applyBatchToStore(store, { ...batch, from_seq: batch.from_seq + 1 })).toBe(false)
    expect(applyBatchToStore(store, batch)).toBe(true)
    expect(applyBatchToStore(store, batch)).toBe(false)
  })
})

describe('what each move shows', () => {
  it('attack: two of the viewer’s creatures lean and glow', () => {
    const effects = effectsOf(table(), 'attack')
    expect(effects).toMatchObject([
      { kind: 'attacked', own: true },
      { kind: 'attacked', own: true },
    ])
  })

  it('block: an opponent’s creature attacks and two of the viewer’s block it', () => {
    const effects = effectsOf(table(), 'block')
    expect(effects).toMatchObject([
      { kind: 'attacked', own: false },
      { kind: 'blocked', own: true },
      { kind: 'blocked', own: true },
    ])
  })

  it('damage: creatures of both sides are hurt and none dies', () => {
    const effects = effectsOf(table(), 'damage')
    expect(kinds(effects)).toEqual(['damaged', 'damaged', 'damaged'])
    for (const effect of effects) expect(effect).toMatchObject({ amount: expect.any(Number) })
  })

  it('death: one creature of each side leaves a ghost that carries the damage that killed it', () => {
    const effects = effectsOf(table(), 'death')
    expect(effects).toHaveLength(2)
    expect(effects.map((effect) => (effect.kind === 'died' ? effect.seat : null)).sort()).toEqual([0, 1])
    for (const effect of effects)
      expect(effect).toMatchObject({ kind: 'died', row: 'creatures', damage: expect.any(Number) })
  })

  it('life: the viewer gains 4 and the opponent loses 5', () => {
    expect(effectsOf(table(), 'life')).toEqual([
      { kind: 'healed', seat: 0, amount: 4 },
      { kind: 'damaged', seat: 1, amount: 5 },
    ])
  })

  it('enter: a creature comes in for each side', () => {
    const effects = effectsOf(table(), 'enter')
    expect(kinds(effects)).toEqual(['entered', 'entered'])
  })
})

describe('played again and again', () => {
  it.each(DEV_FX)('%s works ten times in a row on other creatures and keeps the table consistent', (kind) => {
    const store = table()
    let previous = ''
    for (let n = 0; n < 10; n += 1) {
      const effects = effectsOf(store, kind)
      expect(effects.length, `press ${n + 1}`).toBeGreaterThan(0)
      expectConsistent(must(store.getState().view))
      // Damage that is only a mark: whoever takes it lives.
      if (kind === 'damage') {
        for (const effect of effects) {
          if (effect.kind !== 'damaged' || !('object' in effect)) continue
          const hurt = must(store.getState().objectsById[effect.object])
          expect(must(hurt.permanent).damage, `object ${effect.object}`).toBeLessThan(must(hurt.toughness))
        }
      }
      // A move that picks creatures never picks the same ones for the same job twice running, so every press shows.
      if (kind === 'attack' || kind === 'enter' || kind === 'death') {
        const key = JSON.stringify(
          effects.map((effect) => ('object' in effect ? effect.object : effect.kind)),
        )
        expect(key, `press ${n + 1}`).not.toBe(previous)
        previous = key
      }
    }
  })

  it('death shrinks the battlefield by two creatures a time and fills the graveyards', () => {
    const store = table()
    const start = must(store.getState().view)
    for (let n = 0; n < 5; n += 1) effectsOf(store, 'death')
    const end = must(store.getState().view)
    expect(end.battlefield.count).toBe(start.battlefield.count - 10)
    expect(end.players[0]?.graveyard.count).toBe((start.players[0]?.graveyard.count ?? 0) + 5)
    expect(end.players[1]?.graveyard.count).toBe((start.players[1]?.graveyard.count ?? 0) + 5)
  })

  it('enter gives every new creature an id of its own', () => {
    const store = table()
    const before = must(store.getState().view).objects.length
    for (let n = 0; n < 6; n += 1) effectsOf(store, 'enter')
    const ids = must(store.getState().view).objects.map((object) => object.id)
    expect(ids).toHaveLength(before + 12)
    expect(new Set(ids).size).toBe(ids.length)
  })
})

describe('at other table sizes', () => {
  it.each([2, 3, 4, 6, 12])('every move works at %i seats', (seats) => {
    const store = table(seats)
    for (const kind of DEV_FX) {
      expect(effectsOf(store, kind).length, `${kind} at ${seats} seats`).toBeGreaterThan(0)
      expectConsistent(must(store.getState().view))
    }
  })

  it('aims at the active opponent, and at the next seat on the viewer’s own turn', () => {
    const theirs = table(4)
    effectsOf(theirs, 'life')
    const active = must(theirs.getState().view).turn.active
    const players = must(theirs.getState().view).players
    expect(players[active]?.life).toBeLessThan(40 - 2 * active)

    const own = createPlayStore()
    own.setState(fixtureState(buildFixture({ seats: 4, active: 0 })))
    effectsOf(own, 'life')
    expect(must(own.getState().view).players[1]?.life).toBe(40 - 2 - 5)
  })
})
