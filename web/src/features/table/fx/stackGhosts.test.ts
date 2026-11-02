// Goal: an ability that leaves the stack with an end mark gets a ghost chip before the chip that followed it, which the store keeps and ends together with the mark; spells, abilities that stay and other marks get none; a counter cast in the same batch starts its arrow at its spotlight.
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { card } from '@/test-utils/cards'
import { FakeTimers } from '@/test-utils/fakes'
import { place } from '@/test-utils/fxDom'
import type { Moment } from './derive'
import { createEffectArrows } from './effectArrows'
import { withStackGhosts } from './marks'
import type { MarkEffect } from './model'
import { FX_MS } from './model'
import { createFx } from './store'

const moment = (cards: number[], kinds: Record<number, string>): Moment =>
  ({
    view: { stack: { cards } },
    objectsById: Object.fromEntries(
      Object.entries(kinds).map(([id, kind]) => [id, { id: Number(id), kind }]),
    ),
  }) as unknown as Moment

describe('the ghosts of abilities that leave the stack', () => {
  it.each(['resolved', 'countered', 'fizzled'] as const)(
    'puts a ghost of an ability that ends as %s before the chip below it',
    (kind) => {
      const before = moment([1, 2, 3], { 1: 'Spell', 2: 'Ability', 3: 'Spell' })
      const after = moment([1, 3], { 1: 'Spell', 3: 'Spell' })
      const [effect] = withStackGhosts([{ kind, object: 2 }], before, after)
      expect(effect?.ghost?.before).toBe(1)
      expect(effect?.ghost?.snapshot.id).toBe(2)
    },
  )

  it('leaves spells, abilities still on the stack and other marks alone', () => {
    const before = moment([1, 2], { 1: 'Spell', 2: 'Ability' })
    const after = moment([2], { 2: 'Ability' })
    const given: MarkEffect[] = [
      { kind: 'resolved', object: 1 },
      { kind: 'resolved', object: 2 },
      { kind: 'drawn', object: 2 },
    ]
    expect(withStackGhosts(given, before, after)).toEqual(given)
  })

  it('ends the ghost at the end of the strip when nothing stays below it', () => {
    const before = moment([2], { 2: 'Ability' })
    const [effect] = withStackGhosts([{ kind: 'resolved', object: 2 }], before, moment([], {}))
    expect(effect?.ghost?.before).toBeNull()
  })

  it('is kept by the store with the mark on its object, both ending together', () => {
    const timers = new FakeTimers()
    const fx = createFx({ timers, mode: () => 'full' })
    const ghost = { snapshot: card({ id: 4, kind: 'Ability' }), before: 2 }
    fx.play([{ kind: 'countered', object: 4, ghost }])
    expect(
      fx.store
        .getState()
        .ghosts.get('stack')
        ?.map((g) => [g.object.id, g.before]),
    ).toEqual([[4, 2]])
    expect(fx.store.getState().objects.get(4)?.[0]?.kind).toBe('countered')
    timers.advance(FX_MS.countered)
    expect(fx.store.getState().ghosts.size).toBe(0)
    expect(fx.store.getState().objects.size).toBe(0)
  })
})

describe('an arrow of a counter cast in the same batch', () => {
  const STAND = { x: 500, y: 120, width: 200, height: 280 }
  const BEAR = { x: 100, y: 400, width: 90, height: 126 }
  let page: HTMLElement
  beforeEach(() => {
    page = document.createElement('main')
    document.body.replaceChildren(page)
    place(page, { 'data-object-id': '8' }, BEAR, 'button')
  })
  afterEach(() => document.body.replaceChildren())
  const play = (box: (object: number) => typeof STAND | null) => {
    const effects = createEffectArrows({
      timers: new FakeTimers(),
      enabled: () => true,
      slow: 1,
      root: page,
      spotlightBox: box,
    })
    effects.play([{ by: 7, object: 8, kind: 'countered' }])
    return effects.store.getState()
  }

  it('starts at the spotlight of the counter when it has no chip yet', () => {
    expect(play((object) => (object === 7 ? STAND : null))).toEqual([
      { token: 1, kind: 'countered', from: STAND, to: BEAR },
    ])
  })

  it('draws nothing when no spotlight stands for it', () => {
    expect(play(() => null)).toEqual([])
  })
})
