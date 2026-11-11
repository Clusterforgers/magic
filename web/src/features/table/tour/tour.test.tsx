// Goal: the table tour starts once by itself, walks every step with Next, Back, Skip and the keys, brings up the side tab a step explains, and its card always fits beside the lit part.
import { act, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'
import { GAP, MARGIN, placeCard, spotlight } from './tourPlace'
import type { Box } from './tourPlace'
import { endTour, moveTour, startTour, TOUR_SEEN_KEY, tourSeen, tourStore } from './tourStore'
import { TOUR } from './tourSteps'

const VIEW = { width: 1400, height: 900 }
const CARD = { width: 352, height: 180 }

const inside = (place: { left: number; top: number }, view = VIEW): boolean =>
  place.left >= MARGIN &&
  place.top >= MARGIN &&
  place.left + CARD.width <= view.width - MARGIN &&
  place.top + CARD.height <= view.height - MARGIN

describe('placeCard', () => {
  it.each<[string, Box | null, string]>([
    ['no target', null, 'centre'],
    ['the top bar', { left: 1300, top: 0, width: 40, height: 44 }, 'below'],
    ['the hand', { left: 0, top: 700, width: 1100, height: 200 }, 'above'],
    ['a tall side column', { left: 0, top: 0, width: 300, height: 900 }, 'right'],
    ['a tall column on the right', { left: 1100, top: 0, width: 300, height: 900 }, 'left'],
    ['the whole window', { left: 0, top: 0, width: 1400, height: 900 }, 'over'],
  ])('puts the card of %s on the %s side, inside the window', (_name, target, side) => {
    const place = placeCard(target, CARD, VIEW)
    expect(place.side).toBe(side)
    expect(inside(place)).toBe(true)
  })

  it('keeps the gap to a target it sits under', () => {
    const target = { left: 500, top: 100, width: 200, height: 60 }
    expect(placeCard(target, CARD, VIEW).top).toBe(target.top + target.height + GAP)
  })

  it.each([[{ width: 390, height: 844 }], [{ width: 844, height: 390 }]])(
    'stays inside a %o window',
    (view) => {
      const card = { width: view.width - 2 * MARGIN, height: 200 }
      const place = placeCard({ left: 0, top: 0, width: view.width, height: view.height }, card, view)
      expect(place.left).toBe(MARGIN)
      expect(place.top + card.height).toBeLessThanOrEqual(view.height - MARGIN)
    },
  )
})

describe('spotlight', () => {
  it('grows the target and cuts it to the window', () => {
    expect(spotlight({ left: 0, top: 10, width: 100, height: 50 }, VIEW)).toEqual({
      left: 0,
      top: 6,
      width: 104,
      height: 58,
    })
  })
})

describe('the tour store', () => {
  beforeEach(() => localStorage.removeItem(TOUR_SEEN_KEY))

  it('walks forward and back and ends past the last step', () => {
    startTour()
    moveTour(-1)
    expect(tourStore.getState().step).toBe(0)
    for (let step = 1; step < TOUR.length; step += 1) {
      moveTour(1)
      expect(tourStore.getState().step).toBe(step)
    }
    expect(tourSeen()).toBe(false)
    moveTour(1)
    expect(tourStore.getState().step).toBeNull()
    expect(tourSeen()).toBe(true)
  })

  it('counts as seen where storage throws', () => {
    const spy = vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('denied')
    })
    expect(tourSeen()).toBe(true)
    spy.mockRestore()
  })

  it('remembers a skip', () => {
    startTour()
    endTour()
    expect(tourSeen()).toBe(true)
  })
})

describe('TableTour', () => {
  beforeEach(() => localStorage.removeItem(TOUR_SEEN_KEY))
  afterEach(() => localStorage.removeItem(TOUR_SEEN_KEY))

  it('starts by itself at a table the browser has not toured', async () => {
    renderTable({ seats: 2, options: { tour: true } })
    expect(await screen.findByRole('dialog', { name: 'Welcome to the table' })).toBeInTheDocument()
  })

  it.each([
    ['once seen', true, true],
    ['where the table turns it off', false, false],
  ])('stays away %s', (_name, seen, tour) => {
    if (seen) localStorage.setItem(TOUR_SEEN_KEY, 'seen')
    renderTable({ seats: 2, options: { tour } })
    expect(screen.queryByRole('dialog', { name: 'Welcome to the table' })).toBeNull()
  })

  it('goes through its steps with the buttons and the keys', async () => {
    const { user } = renderTable({ seats: 2 })
    act(startTour)
    await user.click(await screen.findByRole('button', { name: 'Next' }))
    expect(screen.getByRole('dialog', { name: 'Your hand' })).toBeInTheDocument()
    expect(screen.getByText(`Step 2 of ${TOUR.length}`)).toBeInTheDocument()
    await user.keyboard('{ArrowRight}')
    expect(screen.getByRole('dialog', { name: 'Your battlefield' })).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Back' }))
    expect(screen.getByRole('dialog', { name: 'Your hand' })).toBeInTheDocument()
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('dialog', { name: 'Your hand' })).toBeNull()
    expect(tourSeen()).toBe(true)
  })

  it.each([
    ['log', 'Log'],
    ['manual', 'Commands'],
  ])('brings up the side tab of the %s step', async (id, tab) => {
    renderTable({ seats: 2, options: { viewport: { width: 1600, height: 1000 } } })
    act(() => tourStore.setState({ step: TOUR.findIndex((step) => step.id === id) }))
    expect(await screen.findByRole('tab', { name: tab, selected: true })).toBeInTheDocument()
  })

  it('ends with Done on the last step', async () => {
    const { user } = renderTable({ seats: 2 })
    act(() => tourStore.setState({ step: TOUR.length - 1 }))
    await user.click(await screen.findByRole('button', { name: 'Done' }))
    expect(tourStore.getState().step).toBeNull()
  })
})
