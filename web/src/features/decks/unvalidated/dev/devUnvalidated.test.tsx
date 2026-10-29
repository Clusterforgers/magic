// Goal: /dev/unvalidated reads its view from the address (shrugging off nonsense), serves the real page over the fixture decks, shows each view the page has (every deck, one deck, a deck that runs whole, no decks, a wait, a failed list, a server without decks, one failed check and unreadable card texts), and answers "not found" with no fixture in a production build.
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { CARDS, FIXTURE_DECKS, UNVALIDATED_FIXTURE_MARKER, detailOf, reportLine, spanOf } from '../fixture'
import { DEV_VIEWS, parseDevUnvalidatedSearch } from './unvalidatedSearch'

/** The real route tree, loaded fresh so the route sees the `import.meta.env.DEV` the test has set. */
async function visit(url: string) {
  vi.resetModules()
  const { routeTree } = await import('@/routeTree.gen')
  const router = createRouter({ routeTree, history: createMemoryHistory({ initialEntries: [url] }) })
  render(<RouterProvider router={router} />)
  await act(async () => {
    await router.load()
  })
  return router
}

afterEach(() => {
  cleanup()
  vi.unstubAllEnvs()
})

describe('parseDevUnvalidatedSearch', () => {
  it.each([
    [{}, 'all'],
    [{ view: 'deck' }, 'deck'],
    [{ view: 'texts' }, 'texts'],
    [{ view: 'nonsense' }, 'all'],
    [{ view: 3 }, 'all'],
    [{ view: null }, 'all'],
  ])('reads %j as the %s view', (search, view) => {
    expect(parseDevUnvalidatedSearch(search).view).toBe(view)
  })

  it('knows every view the route draws', () => {
    expect(DEV_VIEWS).toEqual([
      'all',
      'deck',
      'clean',
      'none',
      'loading',
      'error',
      'unavailable',
      'failed',
      'texts',
    ])
  })
})

describe('the fixture', () => {
  it('has a clause of every card that lies in its text, and a card of every state but Verified', () => {
    const states = new Set<string>()
    for (const key of Object.keys(CARDS)) {
      const spec = CARDS[key]!
      states.add(spec.support)
      for (const reason of detailOf(key).reasons) {
        const text = spec.faces[reason.face] ?? ''
        expect(reason.span.end, spec.name).toBeLessThanOrEqual(Array.from(text).length)
        expect(reason.span.end, spec.name).toBeGreaterThan(reason.span.start)
      }
    }
    expect(states).toEqual(new Set(['manual', 'partial', 'stale', 'unplayable']))
    expect(reportLine('delver').reasons.some((reason) => reason.face === 1)).toBe(true)
  })

  it('counts a span in Unicode scalar values, as the server does', () => {
    expect(spanOf('a🙂b c', 'b c')).toEqual({ start: 2, end: 5 })
    expect(() => spanOf('abc', 'z')).toThrow()
  })

  it('has a deck that runs whole beside the decks that do not, each list entry agreeing with its report', () => {
    expect(FIXTURE_DECKS.map((deck) => deck.item.state)).toEqual([
      'manual_play',
      'verified',
      'manual_play',
      'manual_play',
    ])
    for (const deck of FIXTURE_DECKS) {
      expect(deck.item.summary.counts).toEqual(deck.report.summary.counts)
    }
  })
})

describe('/dev/unvalidated', () => {
  it('draws the real page over the fixture decks', async () => {
    await visit('/dev/unvalidated')
    expect(await screen.findByRole('heading', { level: 1, name: 'Not validated cards' })).toBeInTheDocument()
    expect(await screen.findByRole('list', { name: 'Cards not validated' })).toBeInTheDocument()
    expect(document.querySelector(`[data-fixture="${UNVALIDATED_FIXTURE_MARKER}"]`)).not.toBeNull()
    expect(document.querySelector('li[data-card="oracle-rhystic-study"]')).not.toBeNull()
  })

  it.each([
    ['deck', /^Cards of Izzet Spells that the game does not run whole/],
    ['clean', 'The game runs every card'],
    ['none', 'You have no decks yet'],
    ['unavailable', 'This server keeps no decks'],
    ['error', /Could not reach the server/],
    ['failed', 'Could not check 1 deck'],
  ])('draws the %s view', async (view, words) => {
    await visit(`/dev/unvalidated?view=${view}`)
    expect(await screen.findByText(words)).toBeInTheDocument()
  })

  it('waits for ever in the loading view', async () => {
    await visit('/dev/unvalidated?view=loading')
    expect(await screen.findByText('Checking your decks…')).toBeInTheDocument()
  })

  it('says the texts could not be read in the texts view', async () => {
    await visit('/dev/unvalidated?view=texts')
    expect(await screen.findByText(/could not be read/)).toBeInTheDocument()
  })
})

describe('/dev/unvalidated in a production build', () => {
  it('answers not found and draws no fixture', async () => {
    vi.stubEnv('DEV', false)
    await visit('/dev/unvalidated')
    expect(await screen.findByText('Not Found')).toBeInTheDocument()
    expect(document.querySelector('[data-fixture]')).toBeNull()
    expect(document.querySelector('li[data-card]')).toBeNull()
  })
})
