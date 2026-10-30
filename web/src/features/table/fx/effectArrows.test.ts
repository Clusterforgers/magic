// Goal: the arrows from a resolving object to what it affects are measured when the batch arrives and drawn only when both ends are on screen, each ends on its own timer after the arrow and the mark have played (stretched by the slow-down), none starts while the layer is off, and `clear` ends them at once.
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { FakeTimers } from '@/test-utils/fakes'
import { place } from '@/test-utils/fxDom'
import { createEffectArrows, EFFECT_ARROW_MS } from './effectArrows'
import type { MarkArrow } from './marks'
import { MOVE_MS } from './moves'

let page: HTMLElement
let timers: FakeTimers

beforeEach(() => {
  page = document.createElement('main')
  document.body.replaceChildren(page)
  timers = new FakeTimers()
})
afterEach(() => {
  document.body.replaceChildren()
})

const BOLT = { x: 300, y: 250, width: 120, height: 32 }
const BEAR = { x: 100, y: 400, width: 90, height: 126 }

const arrows = (over: Partial<MarkArrow> = {}): MarkArrow[] => [{ by: 7, object: 8, kind: 'died', ...over }]

function setup(enabled = true, slow = 1) {
  const effects = createEffectArrows({ timers, enabled: () => enabled, slow, root: page })
  return { effects, now: () => effects.store.getState() }
}

describe('an arrow of the effect of a resolving object', () => {
  it('runs from the object to what it affects, between the boxes measured now', () => {
    place(page, { 'data-object-id': '7' }, BOLT, 'button')
    place(page, { 'data-object-id': '8' }, BEAR, 'button')
    const { effects, now } = setup()
    effects.play(arrows())
    expect(now()).toEqual([{ token: 1, kind: 'died', from: BOLT, to: BEAR }])
  })

  it('keeps the boxes it measured when the objects are gone from the table afterwards', () => {
    const bolt = place(page, { 'data-object-id': '7' }, BOLT, 'button')
    const bear = place(page, { 'data-object-id': '8' }, BEAR, 'button')
    const { effects, now } = setup()
    effects.play(arrows())
    bolt.remove()
    bear.remove()
    expect(now()[0]).toMatchObject({ from: BOLT, to: BEAR })
  })

  it.each([
    ['the object is not on screen', '8'],
    ['what it affects is not on screen', '7'],
  ])('is not drawn when %s', (_, only) => {
    place(page, { 'data-object-id': only }, BOLT, 'button')
    const { effects, now } = setup()
    effects.play(arrows())
    expect(now()).toEqual([])
  })

  it('ends on its own after the arrow and the mark, stretched by the slow-down', () => {
    place(page, { 'data-object-id': '7' }, BOLT, 'button')
    place(page, { 'data-object-id': '8' }, BEAR, 'button')
    const { effects, now } = setup(true, 2)
    effects.play(arrows())
    expect(EFFECT_ARROW_MS).toBe(MOVE_MS.arrow + MOVE_MS.mark)
    timers.advance(2 * EFFECT_ARROW_MS - 1)
    expect(now()).toHaveLength(1)
    timers.advance(1)
    expect(now()).toEqual([])
  })

  it('is left alone while the layer is off and is ended at once by `clear`', () => {
    place(page, { 'data-object-id': '7' }, BOLT, 'button')
    place(page, { 'data-object-id': '8' }, BEAR, 'button')
    const off = setup(false)
    off.effects.play(arrows())
    expect(off.now()).toEqual([])
    const on = setup()
    on.effects.play(arrows())
    expect(on.now()).toHaveLength(1)
    on.effects.clear()
    expect(on.now()).toEqual([])
    expect(timers.pending()).toEqual([])
  })
})
