// Goal: /dev/replay reads its view, game and step from the address (shrugging off nonsense), serves the real replay page over the recorded duel or the dev table's four-seat board at the step asked for, with its toolbar in the pass controls' place, shows each failure the page has, and answers "not found" with no fixture in a production build.
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createCursor } from '../replayModel'
import { COMMANDER_STEPS, commanderReplay, duelReplay, REPLAY_FIXTURE_MARKER } from './fixture'
import { DEV_GAMES, DEV_VIEWS, parseDevReplaySearch } from './replaySearch'
import { TEST_VIEWPORT } from '@/test-utils/tableHarness'

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

beforeEach(() => {
  window.innerWidth = TEST_VIEWPORT.width
  window.innerHeight = TEST_VIEWPORT.height
})

afterEach(() => {
  cleanup()
  vi.unstubAllEnvs()
})

const at = async () => (await screen.findByText(/^\d+ \/ \d+$/)).textContent

describe('parseDevReplaySearch', () => {
  it.each([
    [{}, { view: 'game', game: 'duel', step: 0 }],
    [
      { view: 'missing', game: 'commander', step: '3' },
      { view: 'missing', game: 'commander', step: 3 },
    ],
    [{ step: 'last' }, { view: 'game', game: 'duel', step: 'last' }],
    [{ step: 4 }, { view: 'game', game: 'duel', step: 4 }],
    [
      { view: 'nonsense', game: 'chess', step: -2 },
      { view: 'game', game: 'duel', step: 0 },
    ],
    [{ step: '1.5' }, { view: 'game', game: 'duel', step: 0 }],
    [
      { view: null, game: 3, step: null },
      { view: 'game', game: 'duel', step: 0 },
    ],
  ])('reads %j as %j', (search, parsed) => {
    expect(parseDevReplaySearch(search)).toEqual(parsed)
  })

  it('knows every view and game the route draws', () => {
    expect(DEV_VIEWS).toEqual(['game', 'loading', 'missing', 'signed-out', 'offline', 'broken'])
    expect(DEV_GAMES).toEqual(['duel', 'commander'])
  })
})

describe('the fixture replays', () => {
  it.each([
    ['duel', duelReplay],
    ['commander', commanderReplay],
  ])('the %s replay folds from its first step to its last with a seat name for every player', (_, make) => {
    const replay = make()
    const cursor = createCursor(replay.steps)
    expect(cursor).not.toBeNull()
    const last = cursor?.(replay.steps.length - 1)
    expect(last?.view.players).toHaveLength(replay.names.length)
  })

  it('costs one opponent 3 life a step on the Commander board and never the viewer', () => {
    const replay = commanderReplay()
    expect(replay.steps).toHaveLength(COMMANDER_STEPS)
    const lives = replay.steps.map((step) => step.table.players.map((player) => player.life))
    const lost = lives[0]!.map((life, seat) => life - lives[COMMANDER_STEPS - 1]![seat]!)
    expect(lost[0]).toBe(0)
    expect(lost.reduce((sum, each) => sum + each, 0)).toBe(3 * (COMMANDER_STEPS - 1))
  })
})

describe('/dev/replay', () => {
  it('draws the real replay at its first step with the toolbar where the pass controls stand', async () => {
    await visit('/dev/replay')
    expect(await screen.findByRole('toolbar', { name: 'Replay' })).toBeInTheDocument()
    expect(document.querySelector(`[data-fixture="${REPLAY_FIXTURE_MARKER}"]`)).not.toBeNull()
    expect(await at()).toBe(`1 / ${duelReplay().steps.length}`)
    expect(document.querySelector('[data-area="controls"] [data-slot="replay-controls"]')).not.toBeNull()
    expect(screen.queryByText('Watching: you hold no seat')).toBeNull()
  })

  it.each([
    ['/dev/replay?game=commander&step=last', `${COMMANDER_STEPS} / ${COMMANDER_STEPS}`],
    ['/dev/replay?game=commander&step=2', `3 / ${COMMANDER_STEPS}`],
    ['/dev/replay?game=commander&step=99', `${COMMANDER_STEPS} / ${COMMANDER_STEPS}`],
  ])('opens %s at %s', async (url, step) => {
    await visit(url)
    expect(await at()).toBe(step)
  })

  it.each([
    ['missing', 'No replay for this game'],
    ['signed-out', 'Sign in to watch replays'],
    ['offline', 'The replay could not be loaded'],
    ['broken', 'The replay could not be loaded'],
  ])('draws the %s view', async (view, words) => {
    await visit(`/dev/replay?view=${view}`)
    expect(await screen.findByRole('heading', { level: 1, name: words })).toBeInTheDocument()
  })

  it('waits for ever in the loading view', async () => {
    await visit('/dev/replay?view=loading')
    expect(document.querySelector(`[data-fixture="${REPLAY_FIXTURE_MARKER}"]`)).not.toBeNull()
    expect(screen.queryByRole('toolbar', { name: 'Replay' })).toBeNull()
  })
})

describe('/dev/replay in a production build', () => {
  it('answers not found and draws no fixture', async () => {
    vi.stubEnv('DEV', false)
    await visit('/dev/replay')
    expect(await screen.findByText('Not Found')).toBeInTheDocument()
    expect(document.querySelector('[data-fixture]')).toBeNull()
  })
})
