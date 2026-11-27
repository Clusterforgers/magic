// Goal: the table tour's steps name only registry anchors within the coach's word limits, each step lights exactly one drawn anchor of its own on a two-seat and a four-seat table, and the tour starts once by itself, walks with Next, Back, Skip tour and the keys, brings up the side tab a step explains and ends with Done.
import { act, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from '@/features/table/dev/fixture'
import { cardRef } from '@/features/learn/anchors'
import { TEXT_MAX, TITLE_MAX, knownTarget } from '@/features/learn/step'
import { endTour, moveTour, startTour, TOUR_SEEN_KEY, tourSeen, tourStore } from './tourStore'
import { TOUR } from './tourSteps'

const at = (id: string): number => TOUR.findIndex((step) => step.id === id)

describe('the tour steps', () => {
  it.each(TOUR.map((step) => [step.id, step] as const))(
    '%s names registry anchors in few words',
    (_, step) => {
      expect(step.title.length).toBeLessThanOrEqual(TITLE_MAX)
      expect(step.text.length).toBeLessThanOrEqual(TEXT_MAX)
      for (const target of step.at) expect(knownTarget(target)).toBe(true)
    },
  )

  it('lights a part of the table at every step but the welcome', () => {
    expect(TOUR.filter((step) => step.at.length === 0).map((step) => step.id)).toEqual(['welcome'])
  })
})

describe('the lit anchors', () => {
  beforeEach(() => {
    // jsdom lays nothing out: every element is drawn inside the window, so the anchor's presence decides.
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(
      () => new DOMRect(10, 60, 120, 80),
    )
  })
  afterEach(() => vi.restoreAllMocks())

  const cases = [2, 4].flatMap((seats) =>
    TOUR.filter((step) => step.at.length > 0).map((step) => [seats, step.id] as const),
  )
  it.each(cases)('at %i seats the %s step lights one of its own anchors', async (seats, id) => {
    renderTable({
      fixture: buildFixture({ seats, permanents: 2 }),
      options: { viewport: { width: 1600, height: 1000 } },
    })
    act(() => tourStore.setState({ step: at(id) }))
    const step = TOUR[at(id)]
    const root = await screen.findByRole('dialog', { name: step?.title ?? '' })
    const tour = root.closest<HTMLElement>('[data-slot="table-tour"]')
    await waitFor(() => expect(tour?.dataset['coachLit']).toBe(step?.at[0]))
    const lit = step?.at[0] ?? ''
    const ref = cardRef(lit)
    const found =
      ref === null
        ? document.querySelectorAll(`[data-coach="${lit}"]`)
        : document.querySelectorAll(`[data-coach-zone="${ref.zone}"]`)
    expect(found).toHaveLength(1)
    expect(document.querySelectorAll('[data-slot="coach-ring"]')).toHaveLength(1)
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
    expect(screen.getByRole('img', { name: `Step 2 of ${TOUR.length}` })).toBeInTheDocument()
    await user.keyboard('{ArrowRight}')
    expect(screen.getByRole('dialog', { name: 'Your battlefield' })).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Back' }))
    expect(screen.getByRole('dialog', { name: 'Your hand' })).toBeInTheDocument()
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('dialog', { name: 'Your hand' })).toBeNull()
    expect(tourSeen()).toBe(true)
  })

  it('ends at Skip tour', async () => {
    const { user } = renderTable({ seats: 2 })
    act(startTour)
    await user.click(await screen.findByRole('button', { name: 'Skip tour' }))
    expect(tourStore.getState().step).toBeNull()
    expect(tourSeen()).toBe(true)
  })

  it.each([
    ['log', 'Log'],
    ['manual', 'Commands'],
  ])('brings up the side tab of the %s step', async (id, tab) => {
    renderTable({ seats: 2, options: { viewport: { width: 1600, height: 1000 } } })
    act(() => tourStore.setState({ step: at(id) }))
    expect(await screen.findByRole('tab', { name: tab, selected: true })).toBeInTheDocument()
  })

  it('ends with Done on the last step', async () => {
    const { user } = renderTable({ seats: 2 })
    act(() => tourStore.setState({ step: TOUR.length - 1 }))
    await user.click(await screen.findByRole('button', { name: 'Done' }))
    expect(tourStore.getState().step).toBeNull()
  })
})
