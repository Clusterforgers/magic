// Goal: /dev/table?step= puts the fixture's turn in the named step (any of the thirteen, in any case) so that each phase theme can be looked at, shrugs off every other value so a mistyped address still shows the plain table, overrides the moment a prompt asks for, and opens a table that carries the theme of that step.
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Step } from '@/protocol/generated'
import { TEST_VIEWPORT } from '@/test-utils/tableHarness'
import { fixtureGame } from './dev/devGame'
import { parseDevSearch } from './devSearch'
import { phaseOf } from './turn/phase'
import { STEP_LABEL } from './turnText'

const STEPS = Object.keys(STEP_LABEL) as Step[]

describe('?step=', () => {
  it.each(STEPS)('reads %s as written and in lower case', (step) => {
    expect(parseDevSearch({ seats: 2, step }).step).toBe(step)
    expect(parseDevSearch({ seats: 2, step: step.toLowerCase() }).step).toBe(step)
  })

  it.each([undefined, '', 'main', 'Main 1', 'cleanup ', 'combat', 3, true, null, ['main1'], {}])(
    'leaves out %j',
    (step) => {
      expect(parseDevSearch({ seats: 2, step }).step).toBeUndefined()
    },
  )
})

describe('the fixture game', () => {
  const stepOf = (options: Parameters<typeof fixtureGame>[0]): Step | undefined =>
    fixtureGame(options).session.store.getState().view?.turn.step

  it.each(STEPS)('has the turn in %s when asked', (step) => {
    expect(stepOf({ seats: 2, step })).toBe(step)
  })

  it('keeps the fixture’s own step when none is asked', () => {
    expect(stepOf({ seats: 2 })).toBe('DeclareBlockers')
  })

  it('overrides the moment a prompt asks for, which the theme is looked at from', () => {
    expect(stepOf({ seats: 2, prompt: 'priority-main' })).toBe('Main1')
    expect(stepOf({ seats: 2, prompt: 'priority-main', step: 'End' })).toBe('End')
  })
})

/** The real route tree, loaded fresh so the route sees the `import.meta.env.DEV` the test has set. */
async function visit(url: string) {
  vi.resetModules()
  const { routeTree } = await import('@/routeTree.gen')
  const router = createRouter({ routeTree, history: createMemoryHistory({ initialEntries: [url] }) })
  render(<RouterProvider router={router} />)
  await act(async () => {
    await router.load()
  })
}

const window0 = { width: window.innerWidth, height: window.innerHeight }

beforeEach(() => {
  window.innerWidth = TEST_VIEWPORT.width
  window.innerHeight = TEST_VIEWPORT.height
})

afterEach(() => {
  cleanup()
  window.innerWidth = window0.width
  window.innerHeight = window0.height
})

describe('/dev/table?step=', () => {
  it.each<[string, Step]>([
    ['upkeep', 'Upkeep'],
    ['main2', 'Main2'],
    ['declareattackers', 'DeclareAttackers'],
    ['cleanup', 'Cleanup'],
  ])(
    'opens a table in the phase of %s',
    async (name, step) => {
      await visit(`/dev/table?seats=2&step=${name}`)
      await screen.findByRole('region', { name: 'Game table' }, { timeout: 8000 })
      expect(document.querySelector('[data-slot="table-grid"]')).toHaveAttribute(
        'data-phase',
        phaseOf(step).id,
      )
      expect(document.querySelector('[data-slot="phase-tag"]')).toHaveTextContent(phaseOf(step).name)
    },
    10_000,
  )

  it('shows the fixture’s own phase, combat, when the address names no step', async () => {
    await visit('/dev/table?seats=2')
    await screen.findByRole('region', { name: 'Game table' }, { timeout: 8000 })
    expect(document.querySelector('[data-slot="table-grid"]')).toHaveAttribute('data-phase', 'combat')
  }, 10_000)
})
