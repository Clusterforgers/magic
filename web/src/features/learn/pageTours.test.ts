// Goal: a page tour walks its parts in order across pages, shows a lead to the part's page while another page is open, ends past its last stop with its id (which marks it done), and the Done marks of the Learn page keep the tutorials' order and survive a reload in this browser.
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import {
  endPageTour,
  movePageTour,
  PAGE_TOURS,
  pageTourOf,
  pageTourStore,
  placeOf,
  startPageTour,
} from './pageTours'
import { DONE_KEY, doneStore, markDone, rememberTutorial, TUTORIALS, tutorialOf } from './tutorials'

describe('page tours', () => {
  afterEach(endPageTour)

  it('cover every page tutorial and nothing else', () => {
    const pages = TUTORIALS.filter((tutorial) => tutorial.kind === 'page').map((tutorial) => tutorial.id)
    expect(PAGE_TOURS.map((tour) => tour.id).sort()).toEqual([...pages].sort())
  })

  it.each([
    ['builder', '/decks', '/decks/abc', ['/decks/unvalidated', '/decks/abc/unvalidated']],
    ['lobby', '/new', '/game/abc', ['/', '/games']],
  ] as const)('%s runs its first part on %s and its second on %s', (id, first, second, never) => {
    const tour = pageTourOf(id)!
    expect(tour.parts[0]!.on(first)).toBe(true)
    expect(tour.parts[1]!.on(second)).toBe(true)
    expect(tour.parts[0]!.on(second)).toBe(false)
    for (const path of never) expect(tour.parts.some((part) => part.on(path))).toBe(false)
  })

  it('shows a lead while the part’s page is not open, and the stop once it is', () => {
    expect(placeOf('builder', 0, 0, '/')).toMatchObject({ kind: 'lead', index: 0 })
    expect(placeOf('builder', 0, 0, '/decks')).toMatchObject({ kind: 'stop', index: 0, last: false })
    const total = pageTourOf('builder')!.parts.reduce((sum, part) => sum + part.stops.length, 0)
    expect(placeOf('builder', 1, 3, '/decks/a')).toMatchObject({ kind: 'stop', index: total - 1, last: true })
  })

  it('moves across parts, back across them, and ends with its id past the last stop', () => {
    startPageTour('builder')
    const tour = pageTourOf('builder')!
    const first = tour.parts[0]!.stops.length
    for (let step = 1; step < first; step += 1) expect(movePageTour(1)).toBeNull()
    expect(pageTourStore.getState()).toMatchObject({ part: 0, stop: first - 1 })
    expect(movePageTour(1)).toBeNull()
    expect(pageTourStore.getState()).toMatchObject({ part: 1, stop: 0 })
    movePageTour(-1)
    expect(pageTourStore.getState()).toMatchObject({ part: 0, stop: first - 1 })
    movePageTour(1)
    let ended = null
    for (let step = 0; step < tour.parts[1]!.stops.length; step += 1) ended = movePageTour(1)
    expect(ended).toBe('builder')
    expect(pageTourStore.getState().tour).toBeNull()
  })
})

describe('done marks', () => {
  beforeEach(() => {
    window.localStorage.removeItem(DONE_KEY)
    doneStore.setState({ done: [] })
  })

  it('keep the order of the tutorials and are stored', () => {
    markDone('lexicon')
    markDone('game')
    markDone('game')
    expect(doneStore.getState().done).toEqual(['game', 'lexicon'])
    expect(JSON.parse(window.localStorage.getItem(DONE_KEY) ?? '[]')).toEqual(['game', 'lexicon'])
  })

  it('name the tutorial a game of this tab plays, and nothing for another game', () => {
    rememberTutorial('g1', 'manual')
    expect(tutorialOf('g1')).toBe('manual')
    expect(tutorialOf('g2')).toBeNull()
  })
})
