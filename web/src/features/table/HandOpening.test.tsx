// Goal: while the Mulligan prompt waits, the hand dock draws the opening hand larger than any other hand (cards up to 136 px, the dock grown by `--hand-grow` to hold them in one row), and a hand at any other decision, or an opening hand on a phone, keeps the usual size and dock.
import { screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { PlayState } from '@/play/store'
import { must } from '@/test-utils/batches'
import { KINDS } from '@/test-utils/decisions'
import { renderTable } from '@/test-utils/tableHarness'
import { clampedHeight, HAND_GROW_VAR, HAND_PHONE_CLAMP } from './layout'
import {
  HAND_DOCK_SIDES_PX,
  HAND_FLOOR_PX,
  handCardWidth,
  OPENING_CARD_MAX_PX,
  openingDockNeed,
} from './handFit'

/** The room every element reports; jsdom has no layout. */
const room = { width: 0, height: 0 }

beforeEach(() => {
  vi.spyOn(Element.prototype, 'clientWidth', 'get').mockImplementation(() => room.width)
  vi.spyOn(Element.prototype, 'clientHeight', 'get').mockImplementation(() => room.height)
})

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
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

/** The room the zone of a phone's hand dock leaves its cards: the dock (`HAND_PHONE_CLAMP`) less its border and padding. */
const phoneRoom = (width: number, height: number) => ({
  width: width - HAND_DOCK_SIDES_PX,
  height: Math.round(clampedHeight(HAND_PHONE_CLAMP, height)) - 9,
})

describe('the opening hand on a phone', () => {
  // The hand of 7 and two partner commanders do not fit one row of cards 68 px wide, so the row scrolls.
  it.each<[string, number, number]>([
    ['390x844', 390, 844],
    ['414x896', 414, 896],
    ['360x740', 360, 740],
    ['320x568', 320, 568],
    ['600x960', 600, 960],
  ])(
    'at %s draws the cards the size they have at any other decision, in a row that scrolls, and grows no dock',
    (_name, width, height) => {
      const seen = (kind: 'Mulligan' | 'Priority') => {
        Object.assign(room, phoneRoom(width, height))
        const table = renderTable({
          seats: 2,
          state: waitingOn(kind),
          options: { viewport: { width, height } },
        })
        const drawn = {
          card: cardWidth(),
          scrolls: hand().getAttribute('data-scrolls'),
          rows: hand().getAttribute('data-rows'),
          grown: grown(),
        }
        table.unmount()
        return drawn
      }
      const opening = seen('Mulligan')
      expect(opening).toEqual(seen('Priority'))
      expect(opening.scrolls).toBe('true')
      expect(opening.rows).toBe('1')
      expect(opening.grown).toBe('')
      // As wide as the dock's height holds, down to the narrowest card of a hand and never the opening hand's 136 px.
      const widest = handCardWidth(phoneRoom(width, height).height - 8)
      expect(opening.card).toBe(Math.min(HAND_FLOOR_PX, widest))
      expect(opening.card).toBeLessThan(OPENING_CARD_MAX_PX)
    },
  )

  it('draws the cards of a 390x844 phone 68 px wide', () => {
    Object.assign(room, phoneRoom(390, 844))
    expect(room).toEqual({ width: 366, height: 109 })
    renderTable({
      seats: 2,
      state: waitingOn('Mulligan'),
      options: { viewport: { width: 390, height: 844 } },
    })
    expect(cardWidth()).toBe(HAND_FLOOR_PX)
    expect(hand()).toHaveAttribute('data-scrolls', 'true')
  })

  it("keeps the size of a touch screen's cards, as the hand has at any other decision", () => {
    vi.stubGlobal(
      'matchMedia',
      (query: string) =>
        ({
          matches: query === '(pointer: coarse)',
          addEventListener: () => undefined,
          removeEventListener: () => undefined,
        }) as unknown as MediaQueryList,
    )
    Object.assign(room, phoneRoom(390, 844))
    renderTable({
      seats: 2,
      state: waitingOn('Mulligan'),
      options: { viewport: { width: 390, height: 844 } },
    })
    expect(cardWidth()).toBe(handCardWidth(room.height - 8))
    expect(hand()).toHaveAttribute('data-scrolls', 'true')
    expect(grown()).toBe('')
  })
})
