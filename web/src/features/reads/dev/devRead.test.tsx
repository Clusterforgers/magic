// Goal: /dev/read reads its view from the address (shrugging off nonsense), serves the real review screen over the fixture cards in each view (a deck, a precon, the account's decks, nothing left, a failed list, the table's dialog), and answers "not found" with no fixture in a production build.
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { READS_FIXTURE_MARKER } from './fixture'
import { DEV_READ_VIEWS, parseDevReadSearch } from './readSearch'

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

describe('parseDevReadSearch', () => {
  it.each([
    [{}, 'deck'],
    [{ view: 'stale' }, 'stale'],
    [{ view: 'nonsense' }, 'deck'],
    [{ view: 3 }, 'deck'],
  ])('reads %j as the %s view', (search, view) => {
    expect(parseDevReadSearch(search).view).toBe(view)
  })

  it('knows every view the route draws', () => {
    expect(DEV_READ_VIEWS).toEqual([
      'deck',
      'precon',
      'all',
      'mine',
      'done',
      'loading',
      'error',
      'stale',
      'dialog',
    ])
  })
})

describe('/dev/read', () => {
  it.each([
    ['deck', 'Review Izzet Spellcraft'],
    ['precon', 'Review Izzet Spellcraft'],
    ['mine', 'Review cards'],
    ['done', 'Nothing to review'],
    ['error', 'The cards could not be loaded'],
    ['dialog', 'Read the rules of Tidecaller of the Long Shoal'],
  ])('draws the %s view', async (view, words) => {
    await visit(`/dev/read?view=${view}`)
    expect(await screen.findByText(words)).toBeInTheDocument()
    expect(document.querySelector(`[data-fixture="${READS_FIXTURE_MARKER}"]`)).not.toBeNull()
  })
})

describe('/dev/read in a production build', () => {
  it('answers not found and draws no fixture', async () => {
    vi.stubEnv('DEV', false)
    await visit('/dev/read')
    expect(await screen.findByText('Not Found')).toBeInTheDocument()
    expect(document.querySelector('[data-fixture]')).toBeNull()
  })
})
