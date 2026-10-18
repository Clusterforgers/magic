// Goal: the decoration store starts each effect on its card, seat or battlefield row, ends it on its own timer after exactly its time (stretched by the slow-down), restarts rather than doubles a repeat, ends a whole batch in one update, leaves every other key's value untouched, plays nothing while off and only the still highlights under reduced motion, and stops cleanly when cleared.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { FakeTimers } from '@/test-utils/fakes'
import { card } from '@/test-utils/cards'
import type { DiedEffect, Effect, FxKind } from './model'
import { FX_MS, rowKey } from './model'
import { createFx, MAX_EFFECTS, NO_FX, systemMode } from './store'
import type { FxMode, FxState } from './store'

const KINDS: FxKind[] = ['entered', 'attacked', 'blocked', 'damaged', 'healed', 'died']

const lean = (object: number, kind: 'attacked' | 'blocked' = 'attacked'): Effect => ({
  kind,
  object,
  own: true,
})
const hurt = (object: number, amount = 2): Effect => ({ kind: 'damaged', object, amount })
const life = (seat: number, kind: 'damaged' | 'healed' = 'damaged', amount = 3): Effect => ({
  kind,
  seat,
  amount,
})
const dies = (object: number, seat = 1, before: number | null = null): DiedEffect => ({
  kind: 'died',
  object,
  seat,
  row: 'creatures',
  snapshot: card({ id: object }),
  before,
  damage: 0,
})
/** An effect of each kind on a target of its own. */
const EACH: Record<FxKind, Effect> = {
  entered: { kind: 'entered', object: 1 },
  attacked: lean(2),
  blocked: lean(3, 'blocked'),
  damaged: hurt(4),
  healed: life(0, 'healed'),
  died: dies(5),
}

function setup(mode: FxMode | (() => FxMode) = 'full', slow?: number) {
  const timers = new FakeTimers()
  const fx = createFx({ timers, mode: typeof mode === 'function' ? mode : () => mode, slow })
  const state = (): FxState => fx.store.getState()
  return { timers, fx, state }
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('starting an effect', () => {
  it('puts a card effect under the card, with its amount and side', () => {
    const { fx, state } = setup()
    fx.play([hurt(7, 4), lean(8)])
    expect(state().objects.get(7)).toEqual([
      { kind: 'damaged', token: expect.any(Number), amount: 4, own: false },
    ])
    expect(state().objects.get(8)).toEqual([
      { kind: 'attacked', token: expect.any(Number), amount: 0, own: true },
    ])
  })

  it('puts a life effect under the seat', () => {
    const { fx, state } = setup()
    fx.play([life(1, 'damaged', 5), life(0, 'healed', 2)])
    expect(state().seats.get(1)).toEqual([
      { kind: 'damaged', token: expect.any(Number), amount: 5, own: false },
    ])
    expect(state().seats.get(0)).toEqual([
      { kind: 'healed', token: expect.any(Number), amount: 2, own: false },
    ])
    expect(state().objects.size).toBe(0)
  })

  it('puts a ghost under the row of the seat it stood in, keeping the order they came in', () => {
    const { fx, state } = setup()
    fx.play([dies(5, 1, 6), dies(6, 1), { ...dies(9, 0), row: 'lands' }])
    expect(
      state()
        .ghosts.get(rowKey(1, 'creatures'))
        ?.map((ghost) => [ghost.object.id, ghost.before]),
    ).toEqual([
      [5, 6],
      [6, null],
    ])
    expect(
      state()
        .ghosts.get(rowKey(0, 'lands'))
        ?.map((ghost) => ghost.object.id),
    ).toEqual([9])
    expect(state().objects.size).toBe(0)
  })

  it('keeps the damage on a ghost for it to float', () => {
    const { fx, state } = setup()
    fx.play([{ ...dies(5), damage: 3 }])
    expect(state().ghosts.get(rowKey(1, 'creatures'))?.[0]?.damage).toBe(3)
  })

  it('gives every effect a token of its own', () => {
    const { fx, state } = setup()
    fx.play([lean(1), lean(2), life(0), dies(3)])
    const tokens = [
      ...[...state().objects.values()].flat().map((entry) => entry.token),
      ...[...state().seats.values()].flat().map((entry) => entry.token),
      ...[...state().ghosts.values()].flat().map((ghost) => ghost.token),
    ]
    expect(new Set(tokens).size).toBe(4)
  })

  it('lets different kinds play on one card side by side', () => {
    const { fx, state } = setup()
    fx.play([lean(1)])
    fx.play([hurt(1)])
    expect(
      state()
        .objects.get(1)
        ?.map((entry) => entry.kind),
    ).toEqual(['attacked', 'damaged'])
  })
})

describe('ending an effect', () => {
  it.each(KINDS)('ends %s after exactly its time, on its own', (kind) => {
    const { fx, timers, state } = setup()
    fx.play([EACH[kind]])
    const total = () => state().objects.size + state().seats.size + state().ghosts.size
    expect(total()).toBe(1)
    timers.advance(FX_MS[kind] - 1)
    expect(total()).toBe(1)
    timers.advance(1)
    expect(total()).toBe(0)
    expect(timers.pending()).toEqual([])
  })

  it.each(KINDS)('keeps %s between 250 ms and 600 ms, so the table stays quick', (kind) => {
    expect(FX_MS[kind]).toBeGreaterThanOrEqual(250)
    expect(FX_MS[kind]).toBeLessThanOrEqual(600)
  })

  it('ends effects of different kinds in one batch at their own times', () => {
    const { fx, timers, state } = setup()
    fx.play([EACH.entered, EACH.attacked, EACH.damaged])
    timers.advance(FX_MS.entered)
    expect([...state().objects.keys()]).toEqual([2, 4])
    timers.advance(FX_MS.attacked - FX_MS.entered)
    expect([...state().objects.keys()]).toEqual([4])
    timers.advance(FX_MS.damaged - FX_MS.attacked)
    expect(state().objects.size).toBe(0)
  })

  it('ends a batch of many effects of one kind in one update', () => {
    const { fx, timers } = setup()
    const updates = vi.fn()
    fx.store.subscribe(updates)
    fx.play(Array.from({ length: 30 }, (_, n) => hurt(100 + n)))
    expect(updates).toHaveBeenCalledTimes(1)
    timers.advance(FX_MS.damaged)
    expect(updates).toHaveBeenCalledTimes(2)
    expect(fx.store.getState().objects.size).toBe(0)
  })

  it('stretches every time by the slow-down', () => {
    const { fx, timers, state } = setup('full', 4)
    expect(fx.slow).toBe(4)
    fx.play([EACH.damaged])
    timers.advance(FX_MS.damaged * 4 - 1)
    expect(state().objects.size).toBe(1)
    timers.advance(1)
    expect(state().objects.size).toBe(0)
  })

  it('starts one timer per kind in a batch, not one per effect', () => {
    const { fx, timers } = setup()
    fx.play([hurt(1), hurt(2), hurt(3), lean(4), lean(5), dies(6)])
    expect(timers.pending().sort((a, b) => a - b)).toEqual(
      [FX_MS.attacked, FX_MS.damaged, FX_MS.died].sort((a, b) => a - b),
    )
  })
})

describe('a repeat', () => {
  it('replaces the running effect of the same kind on the same target and restarts its time', () => {
    const { fx, timers, state } = setup()
    fx.play([hurt(1, 2)])
    const first = state().objects.get(1)?.[0]
    timers.advance(300)
    fx.play([hurt(1, 5)])
    const second = state().objects.get(1)?.[0]
    expect(state().objects.get(1)).toHaveLength(1)
    expect(second?.amount).toBe(5)
    expect(second?.token).not.toBe(first?.token)
    // The first timer fires at 550 ms and must not end the second effect.
    timers.advance(FX_MS.damaged - 300)
    expect(state().objects.get(1)).toEqual([second])
    timers.advance(300)
    expect(state().objects.has(1)).toBe(false)
  })

  it('stacks two ghosts of one row, each ending on its own batch', () => {
    const { fx, timers, state } = setup()
    fx.play([dies(5)])
    timers.advance(200)
    fx.play([dies(6)])
    const key = rowKey(1, 'creatures')
    expect(state().ghosts.get(key)).toHaveLength(2)
    timers.advance(FX_MS.died - 200)
    expect(
      state()
        .ghosts.get(key)
        ?.map((ghost) => ghost.object.id),
    ).toEqual([6])
    timers.advance(200)
    expect(state().ghosts.has(key)).toBe(false)
  })
})

describe('what stays the same', () => {
  it('hands the same list to a card whose own effects did not change', () => {
    const { fx, timers, state } = setup()
    fx.play([lean(1)])
    const before = state().objects.get(1)
    fx.play([{ kind: 'entered', object: 2 }, hurt(3)])
    expect(state().objects.get(1)).toBe(before)
    timers.advance(FX_MS.entered)
    expect(state().objects.has(2)).toBe(false)
    expect(state().objects.get(1)).toBe(before)
  })

  it('tells nobody when the first timer of a repeated effect finds its effect already replaced', () => {
    const { fx, timers } = setup()
    fx.play([hurt(1)])
    timers.advance(300)
    fx.play([hurt(1)])
    const updates = vi.fn()
    fx.store.subscribe(updates)
    timers.advance(FX_MS.damaged - 300)
    expect(updates).not.toHaveBeenCalled()
  })
})

describe('the modes', () => {
  it('plays nothing at all while off, and starts no timer', () => {
    const { fx, timers, state } = setup('off')
    const updates = vi.fn()
    fx.store.subscribe(updates)
    fx.play(KINDS.map((kind) => EACH[kind]))
    expect(updates).not.toHaveBeenCalled()
    expect(state().objects.size + state().seats.size + state().ghosts.size).toBe(0)
    expect(timers.pending()).toEqual([])
  })

  it('plays only the still highlights under reduced motion: no ghost and no entrance', () => {
    const { fx, state } = setup('reduced')
    fx.play(KINDS.map((kind) => EACH[kind]))
    expect([...state().objects.keys()]).toEqual([2, 3, 4])
    expect([...state().seats.keys()]).toEqual([0])
    expect(state().ghosts.size).toBe(0)
  })

  it('plays everything when full', () => {
    const { fx, state } = setup('full')
    fx.play(KINDS.map((kind) => EACH[kind]))
    expect([...state().objects.keys()].sort()).toEqual([1, 2, 3, 4])
    expect([...state().seats.keys()]).toEqual([0])
    expect(state().ghosts.size).toBe(1)
  })

  it('reads the mode again at every batch, so a change of setting applies to the next one', () => {
    let mode: FxMode = 'off'
    const { fx, state } = setup(() => mode)
    fx.play([EACH.died])
    expect(state().ghosts.size).toBe(0)
    mode = 'full'
    fx.play([EACH.died])
    expect(state().ghosts.size).toBe(1)
    mode = 'reduced'
    fx.play([EACH.entered, EACH.attacked])
    expect([...state().objects.keys()]).toEqual([2])
    mode = 'off'
    fx.play([EACH.damaged])
    expect(state().objects.has(4)).toBe(false)
  })

  it('takes the system setting by default: reduced when it asks for less motion', () => {
    vi.stubGlobal('matchMedia', () => ({ matches: true }))
    expect(systemMode()).toBe('reduced')
    vi.stubGlobal('matchMedia', () => ({ matches: false }))
    expect(systemMode()).toBe('full')
    vi.unstubAllGlobals()
    expect(systemMode()).toBe('full')
  })

  it('starts nothing from the shared layer that cards outside a table use', () => {
    NO_FX.play([EACH.damaged, EACH.died])
    expect(NO_FX.store.getState().objects.size).toBe(0)
    expect(NO_FX.store.getState().ghosts.size).toBe(0)
  })
})

describe('limits and clearing', () => {
  it(`starts at most ${MAX_EFFECTS} effects of one batch`, () => {
    const { fx, state } = setup()
    fx.play(Array.from({ length: MAX_EFFECTS + 30 }, (_, n) => hurt(1000 + n)))
    expect(state().objects.size).toBe(MAX_EFFECTS)
  })

  it('plays nothing for an empty batch', () => {
    const { fx, timers } = setup()
    const updates = vi.fn()
    fx.store.subscribe(updates)
    fx.play([])
    expect(updates).not.toHaveBeenCalled()
    expect(timers.pending()).toEqual([])
  })

  it('ends everything at once on clear and cancels every timer', () => {
    const { fx, timers, state } = setup()
    fx.play(KINDS.map((kind) => EACH[kind]))
    expect(timers.pending().length).toBeGreaterThan(0)
    fx.clear()
    expect(state().objects.size + state().seats.size + state().ghosts.size).toBe(0)
    expect(timers.pending()).toEqual([])
  })

  it('does not tell anyone that nothing changed when cleared while empty', () => {
    const { fx } = setup()
    const updates = vi.fn()
    fx.store.subscribe(updates)
    fx.clear()
    expect(updates).not.toHaveBeenCalled()
  })

  it('still works after a clear', () => {
    const { fx, timers, state } = setup()
    fx.play([EACH.damaged])
    fx.clear()
    fx.play([EACH.attacked])
    expect([...state().objects.keys()]).toEqual([2])
    timers.advance(FX_MS.attacked)
    expect(state().objects.size).toBe(0)
  })
})
