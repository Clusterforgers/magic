// Goal: the controls dock never hides its pass controls: they stand outside the part that may scroll, and where the controls above them overflow the dock's height (a take-back, the yield line and the stops at 720 to 900 px high) those fold into one wrapping line with the Stops button, unfolding again when the dock grows.
import { act, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { TakeBackOffer } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'

const OFFER: TakeBackOffer = { player: 0, kind: 'Cast', name: 'Lightning Bolt', at: 0 }

/** Makes jsdom measure the dock's upper part `need` px tall content in `room` px, and the dock itself `dock` px high. */
function measure(sizes: { need: number; room: number; dock: number }) {
  const of = (element: HTMLElement, slot: string): boolean =>
    element.getAttribute('data-slot') === slot || element.getAttribute('data-region') === slot
  vi.spyOn(HTMLElement.prototype, 'scrollHeight', 'get').mockImplementation(function (this: HTMLElement) {
    return of(this, 'dock-upper') ? sizes.need : 0
  })
  vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockImplementation(function (this: HTMLElement) {
    if (of(this, 'dock-upper')) return sizes.room
    return of(this, 'controls') ? sizes.dock : 0
  })
}

afterEach(() => {
  vi.restoreAllMocks()
})

const dock = () => must(document.querySelector<HTMLElement>('[data-region="controls"]'))

describe('the controls dock at its height', () => {
  it.each([720, 840, 900])('keeps the pass controls out of the scrolling part at %i px high', (height) => {
    renderTable({
      options: { viewport: { width: 2300, height } },
      state: (state) => ({ ...state, takeBackOffer: OFFER }),
    })
    const upper = must(dock().querySelector('[data-slot="dock-upper"]'))
    const primary = must(dock().querySelector('[data-slot="primary-action"]'))
    expect(upper.contains(primary)).toBe(false)
    expect(upper.className).toContain('fade-scroll')
    expect(must(dock().querySelector('[data-slot="pass-slot"]')).className).toContain('shrink-0')
  })

  it('folds the stops into their button beside the take-back when the full form overflows', () => {
    measure({ need: 120, room: 70, dock: 130 })
    renderTable({
      options: { viewport: { width: 2300, height: 840 } },
      state: (state) => ({ ...state, takeBackOffer: OFFER }),
    })
    expect(dock()).toHaveAttribute('data-tight')
    expect(dock().querySelector('[data-slot="stops-button"]')).not.toBeNull()
    expect(dock().querySelector('[data-slot="stop-ladder"]')).toBeNull()
    expect(must(dock().querySelector('[data-slot="dock-upper"]')).className).toContain('flex-row')
    expect(dock().querySelector('[data-slot="primary-action"]')).not.toBeNull()
  })

  it('keeps the full form while it fits', () => {
    measure({ need: 60, room: 70, dock: 200 })
    renderTable({ options: { viewport: { width: 2300, height: 900 } } })
    expect(dock()).not.toHaveAttribute('data-tight')
    expect(dock().querySelector('[data-slot="stop-ladder"]')).not.toBeNull()
  })

  it('unfolds when the take-back that crowded it goes', async () => {
    measure({ need: 120, room: 70, dock: 130 })
    const { store } = renderTable({
      options: { viewport: { width: 2300, height: 840 } },
      state: (state) => ({ ...state, takeBackOffer: OFFER }),
    })
    expect(dock()).toHaveAttribute('data-tight')
    measure({ need: 60, room: 70, dock: 130 })
    act(() => store.setState({ takeBackOffer: null }))
    await waitFor(() => expect(dock()).not.toHaveAttribute('data-tight'))
  })
})
