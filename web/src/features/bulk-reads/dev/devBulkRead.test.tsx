// Goal: /dev/read-bulk serves the real bulk read screen over the fixture cards, and answers "not found" with no fixture in a production build.
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { BULK_READS_FIXTURE_MARKER } from './fixture'

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

describe('/dev/read-bulk', () => {
  it('draws the bulk screen over the fixture', async () => {
    await visit('/dev/read-bulk')
    expect(await screen.findByRole('heading', { level: 1, name: 'Wording-only reads' })).toBeInTheDocument()
    expect(document.querySelector(`[data-fixture="${BULK_READS_FIXTURE_MARKER}"]`)).not.toBeNull()
  })

  it('answers not found and draws no fixture in a production build', async () => {
    vi.stubEnv('DEV', false)
    await visit('/dev/read-bulk')
    expect(await screen.findByText('Not Found')).toBeInTheDocument()
    expect(document.querySelector('[data-fixture]')).toBeNull()
  })
})
