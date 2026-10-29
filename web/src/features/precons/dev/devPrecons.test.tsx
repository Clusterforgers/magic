// Goal: /dev/precons reads its view from the address (shrugging off nonsense), serves the real page over a fixed list that holds a precon of every check state, newest release first, whose details agree with each deck's counts, and answers "not found" with no fixture in a production build.
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { statusOf } from '../format'
import { FIXTURE_PRECONS, fixtureView, PRECONS_FIXTURE_MARKER } from './fixture'
import { parseDevPreconsSearch } from './preconsSearch'

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

describe('parseDevPreconsSearch', () => {
  it.each([
    [{}, { view: 'list' }],
    [{ view: 'empty' }, { view: 'empty' }],
    [
      { view: 'unavailable', account: 1 },
      { view: 'unavailable', account: true },
    ],
    [
      { view: 'error', account: '1', play: 'refused' },
      { view: 'error', account: true, play: 'refused' },
    ],
    [{ view: 'nonsense', account: 0, play: 'later' }, { view: 'list' }],
  ])('reads %j as %j', (search, expected) => {
    expect(parseDevPreconsSearch(search)).toEqual(expected)
  })
})

describe('the fixture list', () => {
  it('holds a precon of every check state', () => {
    expect(new Set(FIXTURE_PRECONS.map(statusOf))).toEqual(new Set(['whole', 'hand', 'blocked']))
  })

  it('is sorted newest release first, a deck with no date last, as the server sorts it', () => {
    const dates = FIXTURE_PRECONS.map((precon) => precon.release_date ?? '')
    expect(dates).toEqual([...dates].sort().reverse())
  })

  it.each(FIXTURE_PRECONS.map((precon) => [precon.code, precon] as const))(
    'has details for %s that agree with its counts of cards played by hand',
    (code, precon) => {
      const view = fixtureView(code)
      const cards = view?.boards.flatMap((board) => board.cards) ?? []
      const copies = (support: string): number =>
        cards.filter((card) => card.support === support).reduce((sum, card) => sum + card.count, 0)
      expect(view?.precon).toEqual(precon)
      const { counts } = precon.summary
      expect([copies('partial'), copies('manual'), copies('stale'), copies('unplayable')]).toEqual([
        counts.partial,
        counts.manual,
        counts.stale,
        counts.unplayable,
      ])
    },
  )
})

describe('/dev/precons', () => {
  it('draws the real page over the fixture list', async () => {
    await visit('/dev/precons')
    expect(await screen.findByRole('heading', { name: 'Precons' })).toBeInTheDocument()
    expect(await screen.findByRole('table', { name: 'Precons, newest release first' })).toBeInTheDocument()
    expect(document.querySelector(`[data-fixture="${PRECONS_FIXTURE_MARKER}"]`)).not.toBeNull()
  })
})

describe('/dev/precons in a production build', () => {
  it('answers not found and draws no fixture', async () => {
    vi.stubEnv('DEV', false)
    await visit('/dev/precons')
    expect(await screen.findByText('Not Found')).toBeInTheDocument()
    expect(document.querySelector('[data-fixture]')).toBeNull()
    expect(screen.queryByRole('heading', { name: 'Precons' })).toBeNull()
  })
})
