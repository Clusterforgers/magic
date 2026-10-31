// Goal: while the Mulligan prompt waits, the hand dock draws the opening hand larger than any other hand (cards up to 136 px, the dock grown by `--hand-grow` to hold them in one row), and a hand at any other decision, or an opening hand on a phone, keeps the usual size and dock.
import { screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { PlayState } from '@/play/store'
import { must } from '@/test-utils/batches'
import { KINDS } from '@/test-utils/decisions'
import { renderTable } from '@/test-utils/tableHarness'
import { HAND_GROW_VAR } from './layout'
import { OPENING_CARD_MAX_PX, openingDockNeed } from './handFit'

/** The room every element reports; jsdom has no layout. */
const room = { width: 0, height: 0 }

beforeEach(() => {
  vi.spyOn(Element.prototype, 'clientWidth', 'get').mockImplementation(() => room.width)
  vi.spyOn(Element.prototype, 'clientHeight', 'get').mockImplementation(() => room.height)
})

afterEach(() => {
  vi.restoreAllMocks()
})

const hand = (): HTMLElement => screen.getByRole('list', { name: "Alice's hand" })
const cardWidth = (): number => Number.parseInt(hand().style.getPropertyValue('--card-w'), 10)
const grown = (): string =>
  must(document.querySelector<HTMLElement>('[data-slot="table-main"]')).style.getPropertyValue(HAND_GROW_VAR)

/** The viewer waits on `kind`, a decision of the table's own named kinds. */
const waitingOn =
  (name: 'Mulligan' | 'Priority') =>
  (state: PlayState): PlayState => ({
    ...state,
    pending: { id: 7, decision: { player: 0, kind: KINDS[name] } },
  })

describe('the hand dock at a decision', () => {
  it.each<[string, 'Mulligan' | 'Priority', boolean]>([
    ['the Mulligan prompt', 'Mulligan', true],
    ['a Priority prompt', 'Priority', false],
  ])('at %s grows its cards and itself: %s', (_name, kind, opening) => {
    room.width = 1100
    room.height = 220
    renderTable({ seats: 2, state: waitingOn(kind) })
    // The fixture hand of 7 cards and two partner commanders share one line.
    const need = openingDockNeed({ width: 1100 - 8, count: 7, commanders: 2, most: 0.32 * 1000 })
    if (opening) {
      expect(cardWidth()).toBeGreaterThan(112)
      expect(cardWidth()).toBeLessThanOrEqual(OPENING_CARD_MAX_PX)
      expect(grown()).toBe(`${need}px`)
    } else {
      expect(cardWidth()).toBeLessThanOrEqual(112)
      expect(grown()).toBe('')
    }
  })

  it('lets go of the room once the keep or mulligan is answered', () => {
    room.width = 1100
    room.height = 220
    const table = renderTable({ seats: 2, state: waitingOn('Mulligan') })
    expect(grown()).not.toBe('')
    table.update(() => ({ pending: null }))
    expect(grown()).toBe('')
    expect(cardWidth()).toBeLessThanOrEqual(112)
  })

  it('keeps the dock of a phone as it is', () => {
    room.width = 366
    room.height = 106
    renderTable({
      seats: 2,
      state: waitingOn('Mulligan'),
      options: { viewport: { width: 390, height: 844 } },
    })
    expect(grown()).toBe('')
  })
})
