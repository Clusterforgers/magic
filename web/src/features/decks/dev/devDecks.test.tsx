// Goal: /dev/decks reads its view from the address (shrugging off nonsense), serves the real Decks page signed in over the fixture decks and groups (a folded group, a deck in two groups and one in none, every marker, an illegal deck, partners and a long name), shows each state the page has (no groups, no decks, a wait, a failed list, a server without decks), and answers "not found" with no fixture in a production build.
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { act, cleanup, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { membership } from '../groups/grouping'
import { DEV_VIEWS, parseDevDecksSearch } from './decksSearch'
import { DECKS_FIXTURE_MARKER, FIXTURE_DECK_LIST, FIXTURE_GROUPS, LONG_DECK_NAME } from './fixture'

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

describe('parseDevDecksSearch', () => {
  it.each([
    [{}, 'groups'],
    [{ view: 'list' }, 'list'],
    [{ view: 'empty' }, 'empty'],
    [{ view: 'nonsense' }, 'groups'],
    [{ view: 3 }, 'groups'],
    [{ view: null }, 'groups'],
  ])('reads %j as the %s view', (search, view) => {
    expect(parseDevDecksSearch(search).view).toBe(view)
  })

  it('knows every view the route draws', () => {
    expect(DEV_VIEWS).toEqual(['groups', 'list', 'empty', 'loading', 'error', 'unavailable'])
  })
})

describe('the fixture', () => {
  it('holds every deck marker, an illegal deck, partners and a deck with nothing in it', () => {
    expect(new Set(FIXTURE_DECK_LIST.map((deck) => deck.state))).toEqual(
      new Set(['verified', 'manual_play', 'read', 'unchecked']),
    )
    expect(FIXTURE_DECK_LIST.some((deck) => !deck.summary.legal)).toBe(true)
    expect(FIXTURE_DECK_LIST.some((deck) => deck.commanders.length === 2)).toBe(true)
    expect(FIXTURE_DECK_LIST.some((deck) => deck.summary.counts.total === 0)).toBe(true)
  })

  it('names only fixture decks in its groups, puts one deck in two and leaves one in none', () => {
    const ids = new Set(FIXTURE_DECK_LIST.map((deck) => deck.id))
    const groupsOf = membership(FIXTURE_GROUPS)
    for (const group of FIXTURE_GROUPS) for (const deck of group.decks) expect(ids).toContain(deck)
    expect([...groupsOf.values()].some((groups) => groups.length > 1)).toBe(true)
    expect([...ids].some((id) => !groupsOf.has(id))).toBe(true)
    expect(FIXTURE_GROUPS.some((group) => group.collapsed)).toBe(true)
  })
})

describe('/dev/decks', () => {
  it('draws the real page with the groups in order, Ungrouped last, the folded group folded', async () => {
    await visit('/dev/decks')
    expect(await screen.findByRole('heading', { level: 1, name: 'Decks' })).toBeInTheDocument()
    expect(await screen.findByRole('list', { name: 'Decks in Fun' })).toBeInTheDocument()
    expect(document.querySelector(`[data-fixture="${DECKS_FIXTURE_MARKER}"]`)).not.toBeNull()
    const headings = screen.getAllByRole('heading', { level: 2 }).map((heading) => heading.textContent)
    expect(headings.map((text) => text?.replace(/\d+ decks?$/, '').trim())).toEqual([
      'Fun',
      'Hardcore',
      'Dungeon',
      'Ungrouped',
    ])
    expect(screen.queryByRole('list', { name: 'Decks in Hardcore' })).toBeNull()
    const fun = within(screen.getByRole('list', { name: 'Decks in Fun' }))
    expect(fun.getByText(LONG_DECK_NAME)).toBeInTheDocument()
    expect(
      within(screen.getByRole('list', { name: 'Decks in Ungrouped' })).getByText('New deck'),
    ).toBeVisible()
  })

  it('draws one list without groups in the list view, offering the three preset groups', async () => {
    await visit('/dev/decks?view=list')
    const list = await screen.findByRole('list', { name: 'Your decks' })
    expect(list.querySelectorAll(':scope > li')).toHaveLength(FIXTURE_DECK_LIST.length)
    expect(await screen.findByRole('button', { name: /Fun, Hardcore and Dungeon/ })).toBeInTheDocument()
  })

  it.each([
    ['empty', 'You have no decks yet'],
    ['loading', 'Loading your decks…'],
    ['error', 'Could not load your decks'],
    ['unavailable', 'This server keeps no decks'],
  ])('draws the %s view', async (view, words) => {
    await visit(`/dev/decks?view=${view}`)
    expect(await screen.findByText(words)).toBeInTheDocument()
  })
})

describe('/dev/decks in a production build', () => {
  it('answers not found and draws no fixture', async () => {
    vi.stubEnv('DEV', false)
    await visit('/dev/decks')
    expect(await screen.findByText('Not Found')).toBeInTheDocument()
    expect(document.querySelector('[data-fixture]')).toBeNull()
  })
})
