// Goal: every dev move (attack, block, damage, death, life, enter, the moments of play: cast, activate, resolve, counter, land, draw, and equip) builds a batch that follows on from the table, folds into a consistent table and derives the effects or moves its name promises at any table size, and can be played again and again, each time on other cards, without ever breaking the table or running out.
import { describe, expect, it } from 'vitest'
import { createPlayStore } from '@/play/store'
import type { PlayStore } from '@/play/store'
import type { StateView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { DEV_FX, DEV_PLAYS } from '../devSearch'
import type { DevFx } from '../devSearch'
import { deriveAttaches, deriveEffects } from '../fx/derive'
import { batchBetween } from '../fx/batch'
import type { Effect } from '../fx/model'
import { anchorOf, deriveMoves } from '../fx/moves'
import type { Moves } from '../fx/moves'
import { applyBatchToStore } from './applyBatch'
import { fixtureState } from './devGame'
import { withEquip } from './equip'
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

/** Plays `kind` and returns the moves and marks the travel layer would play for it. */
function movesOf(store: PlayStore, kind: DevFx): Moves {
  const before = store.getState()
  expect(playDevFx(store, kind), `${kind} did nothing`).toBe(true)
  const step = must(batchBetween(before, store.getState()), 'a batch the effects layer sees')
  return deriveMoves(step.before, step.after, step.events)
}

/** Plays `kind` and returns a key for everything it shows: its effects, else its moves and marks. */
function shownBy(store: PlayStore, kind: DevFx): string[] {
  if (kind === 'equip') {
    const before = store.getState()
    expect(playDevFx(store, kind), `${kind} did nothing`).toBe(true)
    const step = must(batchBetween(before, store.getState()), 'a batch the effects layer sees')
    return [
      ...deriveEffects(step.before, step.after, step.events).map((effect) =>
        'object' in effect ? `${effect.kind} ${effect.object}` : effect.kind,
      ),
      ...deriveAttaches(step.before, step.after).map(
        (change) => `attach ${change.object} > ${JSON.stringify(change.to)}`,
      ),
    ]
  }
  if (!DEV_PLAYS.some((play) => play === kind))
    return effectsOf(store, kind).map((effect) => ('object' in effect ? `${effect.object}` : effect.kind))
  const { moves, marks } = movesOf(store, kind)
  return [
    ...moves.map((move) => `${move.kind} ${move.object}`),
    ...marks.map((mark) => `${mark.kind} ${mark.object}`),
  ]
}

const kinds = (effects: Effect[]): string[] => effects.map((effect) => effect.kind)

const shape = ({ moves }: Moves): string[] =>
  moves.map(
    (move) => `${move.kind} ${anchorOf(move.from)} > ${anchorOf(move.to)}${move.spotlight ? ' *' : ''}`,
  )

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
    // Only the opponent's draw is hidden from the viewer.
    expect(batch.events.filter((event) => 'Hidden' in event)).toHaveLength(kind === 'draw' ? 1 : 0)
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

describe('equip', () => {
  const attaches = (store: PlayStore) => {
    const before = store.getState()
    expect(playDevFx(store, 'equip')).toBe(true)
    const step = must(batchBetween(before, store.getState()))
    return deriveAttaches(step.before, step.after)
  }

  it('brings an Equipment onto the first creature of a table that has none, then moves it host to host', () => {
    const store = table()
    expect(attaches(store)).toEqual([])
    const view = must(store.getState().view)
    const sword = must(view.objects.find((object) => object.subtypes.includes('Equipment')))
    expect(sword.attached_to).toBeDefined()
    const [first, second] = attaches(store)
    expect(second).toBeUndefined()
    expect(first).toMatchObject({ object: sword.id, from: { card: sword.attached_to }, seat: 0 })
    expect(first?.to).not.toEqual(first?.from)
  })

  it('moves the Equipment of the attachments scene from its host to a creature of the viewer', () => {
    const store = createPlayStore()
    store.setState(fixtureState(withEquip(buildFixture({ seats: 2 }))))
    const before = must(store.getState().view)
    const gear = must(
      before.objects.find((object) => object.name === 'Bonesplitter' && object.attached_to !== undefined),
    )
    const [change] = attaches(store)
    expect(change).toMatchObject({ object: gear.id, from: { card: gear.attached_to } })
    const after = must(store.getState().view)
    const to = must(change?.to)
    const host = 'card' in to ? to.card : -1
    expect(after.objects.find((object) => object.id === gear.id)?.attached_to).toBe(host)
    expect(after.objects.find((object) => object.id === host)?.attachments).toContain(gear.id)
    const old = must(after.objects.find((object) => object.id === gear.attached_to))
    expect(old.attachments ?? []).not.toContain(gear.id)
  })
})

describe('what each moment of play shows', () => {
  it.each<[DevFx, string[], string[]]>([
    ['cast', ['cast 0:hand > stack'], ['targeted']],
    ['activate', ['activated 0:battlefield > stack'], ['activated', 'targeted']],
    ['resolve', ['resolved stack > 0:graveyard'], ['resolved']],
    [
      'counter',
      ['cast 0:hand > stack *', 'countered stack > 0:graveyard', 'resolved stack > 0:graveyard'],
      ['targeted', 'countered', 'resolved'],
    ],
    ['land', ['landed 0:hand > 0:battlefield'], []],
    ['draw', ['drawn 0:library > 0:hand', 'drawn 1:library > 1:hand'], ['drawn']],
  ])('%s', (kind, moves, marks) => {
    const shown = movesOf(table(), kind)
    expect(shape(shown)).toEqual(moves)
    expect(shown.marks.map((mark) => mark.kind)).toEqual(marks)
  })

  it('resolve on an empty stack has the opponent cast a creature that passes through it onto the battlefield', () => {
    const store = table()
    for (let n = 0; n < 3; n += 1) movesOf(store, 'resolve')
    expect(must(store.getState().view).stack.count).toBe(0)
    expect(shape(movesOf(store, 'resolve'))).toEqual([
      'cast 1:hand > stack *',
      'resolved stack > 1:battlefield',
    ])
  })
})

describe('played again and again', () => {
  it.each(DEV_FX)('%s works ten times in a row on other cards and keeps the table consistent', (kind) => {
    const store = table()
    let previous = ''
    for (let n = 0; n < 10; n += 1) {
      if (kind === 'damage') {
        // Damage that is only a mark: whoever takes it lives.
        for (const effect of effectsOf(store, kind)) {
          if (effect.kind !== 'damaged' || !('object' in effect)) continue
          const hurt = must(store.getState().objectsById[effect.object])
          expect(must(hurt.permanent).damage, `object ${effect.object}`).toBeLessThan(must(hurt.toughness))
        }
        expectConsistent(must(store.getState().view))
        continue
      }
      const shown = shownBy(store, kind)
      expect(shown.length, `press ${n + 1}`).toBeGreaterThan(0)
      expectConsistent(must(store.getState().view))
      // A move that picks cards never picks the same ones for the same job twice running, so every press shows.
      if (kind !== 'block' && kind !== 'life' && kind !== 'draw') {
        const key = JSON.stringify(shown)
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
      expect(shownBy(store, kind).length, `${kind} at ${seats} seats`).toBeGreaterThan(0)
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
