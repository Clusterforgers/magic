// Goal: while the Mulligan prompt waits off a phone the opening hand floats in the centre overlay, and the hand dock keeps its size and place without drawing the hand a second time, the overlay covers the opponents and the strip but not your board, while another choice in the overlay leaves the hand drawn; on a phone the dock draws the hand as at any other decision, in a row that scrolls.
import { screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { PlayState } from '@/play/store'
import { must } from '@/test-utils/batches'
import { KINDS } from '@/test-utils/decisions'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from './dev/fixture'
import { DEV_PROMPTS } from './dev/prompts'
import { CHOICE_AREAS, clampedHeight, HAND_GROW_VAR, HAND_PHONE_CLAMP, TABLE_AREAS } from './layout'
import { HAND_DOCK_SIDES_PX, HAND_FLOOR_PX, handCardWidth } from './handFit'

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

const hand = (): HTMLElement => screen.getByRole('list', { name: "Alice's hand", hidden: true })
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

describe('the hand dock at the opening hand', () => {
  it.each([2, 4])(
    'keeps its size and its cards at %i seats while the hand floats in the centre, and draws none of them',
    (seats) => {
      room.width = 1100
      room.height = 220
      const seen = (kind: 'Mulligan' | 'Priority') => {
        const table = renderTable({ seats, state: waitingOn(kind) })
        const drawn = {
          card: cardWidth(),
          grown: grown(),
          hidden: hand().closest('[data-floating]') !== null,
        }
        table.unmount()
        return drawn
      }
      const opening = seen('Mulligan')
      const other = seen('Priority')
      expect(opening.card).toBe(other.card)
      expect(opening.grown).toBe('')
      expect(opening.hidden).toBe(true)
      expect(other.hidden).toBe(false)
    },
  )

  it('draws its cards under another choice in the overlay, a long list of targets', () => {
    room.width = 1100
    room.height = 220
    const fixture = buildFixture({ seats: 5 })
    const pending = must(DEV_PROMPTS['targets-many'])(fixture)
    renderTable({ fixture, state: (state) => ({ ...state, pending }) })
    const host = must(document.querySelector<HTMLElement>('[data-slot="choice-host"]'))
    expect(host.hidden).toBe(false)
    expect(host.querySelector('button[data-candidate]')).not.toBeNull()
    expect(hand().closest('[data-floating]')).toBeNull()
  })

  it('covers only the opponents and the strip at the opening hands, so your board and life stay in view; another choice covers both boards', () => {
    room.width = 1100
    room.height = 220
    const main = () => must(document.querySelector<HTMLElement>('[data-slot="table-main"]'))
    const table = renderTable({ seats: 2, state: waitingOn('Mulligan') })
    expect(main().style.gridTemplateAreas).toBe(TABLE_AREAS)
    table.unmount()
    const fixture = buildFixture({ seats: 5 })
    const pending = must(DEV_PROMPTS['targets-many'])(fixture)
    renderTable({ fixture, state: (state) => ({ ...state, pending }) })
    expect(main().style.gridTemplateAreas).toBe(CHOICE_AREAS)
  })

  it('draws its cards again once the keep or mulligan is answered', () => {
    room.width = 1100
    room.height = 220
    const table = renderTable({ seats: 2, state: waitingOn('Mulligan') })
    expect(hand().closest('[data-floating]')).not.toBeNull()
    table.update(() => ({ pending: null }))
    expect(hand().closest('[data-floating]')).toBeNull()
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
      // As wide as the dock's height holds, down to the narrowest card of a hand.
      const widest = handCardWidth(phoneRoom(width, height).height - 8)
      expect(opening.card).toBe(Math.min(HAND_FLOOR_PX, widest))
    },
  )

  it('draws the cards of a 390x844 phone 68 px wide', () => {
    Object.assign(room, phoneRoom(390, 844))
    expect(room).toEqual({ width: 366, height: 126 })
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
