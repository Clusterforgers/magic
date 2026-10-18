// Goal: /dev/table serves the real table over the fixture in development, reads seats, mode and focus from the address (and shrugs off nonsense), and answers "not found" with no fixture when the build is a production one.
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { FIXTURE_MARKER } from './dev/fixture'

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

const panels = (): number[] =>
  [...document.querySelectorAll('[data-seat]')].map((element) => Number(element.getAttribute('data-seat')))

afterEach(() => {
  cleanup()
  vi.unstubAllEnvs()
})

describe('/dev/table in development', () => {
  it('shows a four-seat grid table when the address says nothing', async () => {
    await visit('/dev/table')
    // The first visit pays for importing the whole route tree again, which can pass 1 s under a loaded runner.
    await screen.findByRole('region', { name: 'Game table' }, { timeout: 8000 })
    expect(panels()).toEqual([1, 2, 3, 0])
    expect(document.querySelector('[data-layout]')).toHaveAttribute('data-layout', 'grid')
    expect(document.querySelector(`[data-fixture="${FIXTURE_MARKER}"]`)).not.toBeNull()
  }, 10_000)

  it.each([
    ['?seats=2', 'grid', [1, 0]],
    ['?seats=12', 'focus', [1, 0]],
    ['?seats=12&mode=grid', 'grid', [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0]],
    ['?seats=4&mode=focus&focus=3', 'focus', [3, 0]],
    ['?seats=6&focus=5', 'focus', [5, 0]],
  ])('reads %s', async (search, mode, seats) => {
    await visit(`/dev/table${search}`)
    await screen.findByRole('region', { name: 'Game table' })
    expect(document.querySelector('[data-layout]')).toHaveAttribute('data-layout', mode)
    expect(panels()).toEqual(seats)
  })

  it.each(['?seats=99', '?seats=1', '?seats=abc', '?mode=sideways&seats=3'])(
    'ignores the nonsense in %s',
    async (search) => {
      await visit(`/dev/table${search}`)
      await screen.findByRole('region', { name: 'Game table' })
      expect(panels().length).toBeGreaterThanOrEqual(2)
      expect(panels().length).toBeLessThanOrEqual(4)
    },
  )

  it('is the real table, with its toolbar, banner and drawer', async () => {
    await visit('/dev/table?seats=2')
    await screen.findByRole('region', { name: 'Game table' })
    expect(screen.getByRole('toolbar', { name: 'Table layout' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Turn 5 · Bo · Declare blockers' })).toBeInTheDocument()
    expect(screen.getByRole('complementary', { name: 'Game log and chat' })).toBeInTheDocument()
  })
})

describe('/dev/table with a prompt', () => {
  it('shows no decision by default', async () => {
    await visit('/dev/table?seats=2')
    await screen.findByRole('region', { name: 'Game table' })
    expect(document.querySelector('[data-decision]')).toBeNull()
    expect(screen.queryByRole('button', { name: 'Pass priority' })).toBeNull()
  })

  it.each(['?seats=2&prompt=priority', '?seats=4&mode=grid&prompt=priority'])(
    'shows the priority prompt for %s, with legal cards marked',
    async (search) => {
      await visit(`/dev/table${search}`)
      await screen.findByRole('region', { name: 'Game table' })
      expect(document.querySelector('[data-decision="Priority"]')).not.toBeNull()
      expect(screen.getByRole('button', { name: 'Pass priority' })).toBeEnabled()
      expect(document.querySelectorAll('[data-highlight="legal"]').length).toBeGreaterThan(3)
      expect(document.querySelectorAll('[data-highlight="dimmed"]').length).toBeGreaterThan(3)
    },
  )

  it('lets the prompt be answered, which clears it', async () => {
    await visit('/dev/table?seats=2&prompt=priority')
    const pass = await screen.findByRole('button', { name: 'Pass priority' })
    await act(async () => pass.click())
    expect(document.querySelector('[data-decision]')).toBeNull()
    expect(document.querySelectorAll('[data-highlight]')).toHaveLength(0)
  })

  it('ignores a prompt it does not know', async () => {
    await visit('/dev/table?seats=2&prompt=nonsense')
    await screen.findByRole('region', { name: 'Game table' })
    expect(document.querySelector('[data-decision]')).toBeNull()
  })
})

describe('/dev/table in a production build', () => {
  it('answers not found and draws no fixture', async () => {
    vi.stubEnv('DEV', false)
    await visit('/dev/table?seats=4')
    expect(await screen.findByText('Not Found')).toBeInTheDocument()
    expect(document.querySelector('[data-seat]')).toBeNull()
    expect(document.querySelector('[data-fixture]')).toBeNull()
  })
})
