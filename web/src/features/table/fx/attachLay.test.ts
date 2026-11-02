// Goal: an attachment that had no host has the box it lay in measured when its arrow starts, so the equip arrow can start there; one that had a host keeps null.
import { afterEach, describe, expect, it } from 'vitest'
import { FakeTimers } from '@/test-utils/fakes'
import { place } from '@/test-utils/fxDom'
import { createArrows } from './arrows'
import type { AttachChange } from './derive'

const LAY = { x: 40, y: 300, width: 90, height: 126 }
const change = (object: number, from: AttachChange['from']): AttachChange => ({
  object,
  seat: 0,
  from,
  to: { card: 9, seat: 0 },
})

afterEach(() => document.body.replaceChildren())

describe('where an attachment without a host lay', () => {
  it('is measured when the arrow starts, and only for an attachment that had no host', () => {
    const page = document.createElement('main')
    document.body.replaceChildren(page)
    place(page, { 'data-object-id': '5' }, LAY, 'button')
    const arrows = createArrows({ timers: new FakeTimers(), enabled: () => true, slow: 1, root: page })
    arrows.play([change(5, null), change(6, { card: 1, seat: 0 })])
    expect(arrows.store.getState().map((arrow) => arrow.lay)).toEqual([LAY, null])
  })

  it('is null when the attachment is not on screen', () => {
    const page = document.createElement('main')
    const arrows = createArrows({ timers: new FakeTimers(), enabled: () => true, slow: 1, root: page })
    arrows.play([change(5, null)])
    expect(arrows.store.getState()[0]?.lay).toBeNull()
  })
})
