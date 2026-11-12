// Goal: the art a deck chose reaches every place the table draws a card: the viewer's battlefield and hand and an opponent's battlefield ask for the chosen printing's picture, a card with no chosen art asks for the card's own, and a token keeps its token picture.
import { act } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { resetImageMemory } from '@/features/cards/CardImage'
import { resetImageChoice } from '@/features/cards/imageSetting'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import type { ObjectView } from '@/protocol/generated'
import { fixtureLookup } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import type { Fixture } from './dev/fixture'

afterEach(() => {
  resetImageMemory()
  resetImageChoice()
})

/** Every card of seat 0 in its chosen printing, every other card in its default art. */
function withArt(fixture: Fixture): Fixture {
  const objects = fixture.view.objects.map((object): ObjectView => {
    if (object.kind !== undefined) return object
    const oracle = `id-${object.id}`
    return {
      ...object,
      identity: object.owner === 0 ? { oracle_id: oracle, printing_id: `art-${object.id}` } : { oracle_id: oracle },
    }
  })
  return { ...fixture, view: { ...fixture.view, objects } }
}

const sources = (container: HTMLElement, selector: string): string[] =>
  [...container.querySelectorAll(`${selector} img`)].map((img) => img.getAttribute('src') ?? '')

describe('the chosen art at the table', () => {
  it.each([0, 1])('reaches the board and hand of both seats as seat %i sees them', (viewer) => {
    const fixture = withArt(buildFixture({ seats: 2, viewer, permanents: 6 }))
    const h = startSession()
    h.join(viewer, 2)
    act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
    const { container } = renderLiveTable(h.session, fixtureLookup(fixture), { mode: 'grid' })
    const owned = (seat: number) =>
      fixture.view.objects.filter((o) => o.owner === seat && o.identity !== undefined && o.zone === 'Battlefield')
    for (const seat of [0, 1]) {
      for (const object of owned(seat)) {
        const src = sources(container, `[data-object-id="${object.id}"]`)
        expect(src.length, `object ${object.id}`).toBeGreaterThan(0)
        const expected = seat === 0 ? `/api/images/printing/art-${object.id}/` : `/api/images/oracle/id-${object.id}/`
        for (const url of src) expect(url).toContain(expected)
      }
    }
    if (viewer === 0) {
      const hand = fixture.view.objects.filter((o) => o.owner === 0 && typeof o.zone === 'object' && 'Hand' in o.zone && o.identity !== undefined)
      expect(hand.length).toBeGreaterThan(0)
      for (const object of hand) {
        const src = sources(container, `[data-object-id="${object.id}"]`)
        expect(src.length, `hand object ${object.id}`).toBeGreaterThan(0)
        for (const url of src) expect(url).toContain(`/api/images/printing/art-${object.id}/`)
      }
    }
  })
})
