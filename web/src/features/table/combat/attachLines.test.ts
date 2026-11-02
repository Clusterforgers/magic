// Goal: an attach line starts at the old host, at the box where a hostless attachment lay when one was measured, else at the attachment itself, and ends at the new host; a detach draws none.
import { describe, expect, it } from 'vitest'
import type { AttachArrow } from '../fx/arrows'
import { attachLines } from './attachLines'

const LAY = { x: 1, y: 2, width: 3, height: 4 }
const arrow = (over: Partial<AttachArrow['change']>, lay: AttachArrow['lay'] = null): AttachArrow => ({
  token: 3,
  lay,
  change: { object: 5, seat: 1, from: null, to: { card: 9, seat: 1 }, ...over },
})

describe('attach lines', () => {
  it.each([
    ['an old host', arrow({ from: { card: 2, seat: 1 } }, LAY), { kind: 'card', id: 2, seat: 1 }],
    ['a measured lay', arrow({}, LAY), { kind: 'spot', id: '3:lay', box: LAY }],
    ['the attachment itself', arrow({}), { kind: 'card', id: 5, seat: 1 }],
  ])('starts at %s', (_name, input, from) => {
    expect(attachLines([input])).toEqual([
      { key: 'attach:3', kind: 'attach', pending: false, from, to: { kind: 'card', id: 9, seat: 1 } },
    ])
  })

  it('draws nothing for a detach', () => {
    expect(attachLines([arrow({ to: null })])).toEqual([])
  })
})
