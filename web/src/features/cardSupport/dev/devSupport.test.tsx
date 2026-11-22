// Goal: /dev/support reads its view from the address (shrugging off nonsense), serves the real page signed in over a fixture whose counts agree with its missing cards, and answers "not found" with no fixture in a production build.
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { FIXTURE_MISSING, FIXTURE_OVERVIEW, SUPPORT_FIXTURE_MARKER } from './fixture'
import { parseDevSupportSearch } from './supportSearch'

async function visit(url: string) {
  vi.resetModules()
  const { routeTree } = await import('@/routeTree.gen')
  const router = createRouter({ routeTree, history: createMemoryHistory({ initialEntries: [url] }) })
  render(<RouterProvider router={router} />)
  await act(async () => {
    await router.load()
  })
}

afterEach(() => {
  cleanup()
  vi.unstubAllEnvs()
})

describe('parseDevSupportSearch', () => {
  it.each([
    [{}, 'full'],
    [{ view: 'empty' }, 'empty'],
    [{ view: 'error' }, 'error'],
    [{ view: 'loading' }, 'loading'],
    [{ view: 'nonsense' }, 'full'],
  ])('reads %j as %s', (search, view) => {
    expect(parseDevSupportSearch(search)).toEqual({ view })
  })
})

describe('the fixture', () => {
  it('counts the states its missing cards hold and as many missing as it lists', () => {
    const snapshot = FIXTURE_OVERVIEW.snapshot
    const states = snapshot?.states
    expect(snapshot?.missing).toBe(FIXTURE_MISSING.cards.length)
    expect(snapshot && states && snapshot.automatic + snapshot.missing).toBe(snapshot?.cards)
    for (const state of ['waiting', 'partial', 'manual', 'other'] as const) {
      expect(states?.[state]).toBe(FIXTURE_MISSING.cards.filter((card) => card.state === state).length)
    }
  })
})

describe('/dev/support', () => {
  it('draws the real page over the fixture', async () => {
    await visit('/dev/support')
    expect(await screen.findByRole('heading', { level: 1, name: 'Card support' })).toBeInTheDocument()
    expect(await screen.findByRole('region', { name: 'What we are missing' })).toBeInTheDocument()
    expect(document.querySelector(`[data-fixture="${SUPPORT_FIXTURE_MARKER}"]`)).not.toBeNull()
  })
})

describe('/dev/support in a production build', () => {
  it('answers not found and draws no fixture', async () => {
    vi.stubEnv('DEV', false)
    await visit('/dev/support')
    expect(await screen.findByText('Not Found')).toBeInTheDocument()
    expect(document.querySelector('[data-fixture]')).toBeNull()
  })
})
