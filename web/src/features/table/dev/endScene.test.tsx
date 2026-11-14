// Goal: `/dev/table?teams=` and `?result=` put the fixture in two teams and end it with every kind of result, and the route shows the result page with a way back to the final table.
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { scaled } from '@/test-utils/loadFactor'
import { parseDevSearch } from '../devSearch'
import { endedWith, withTeams } from './endScene'
import { buildFixture } from './fixture'

afterEach(cleanup)

describe('parseDevSearch', () => {
  it.each([
    ['2v2', 'teams'],
    ['teams', 'teams'],
    ['2hg', 'two_headed_giant'],
    ['3v3', undefined],
  ])('reads teams=%s as %s', (value, mode) => {
    expect(parseDevSearch({ teams: value }).teams).toBe(mode)
  })

  it.each(['won', 'lost', 'draw', 'team', 'nope'])('reads result=%s', (value) => {
    expect(parseDevSearch({ result: value }).result).toBe(value === 'nope' ? undefined : value)
  })
})

describe('withTeams', () => {
  it.each(['teams', 'two_headed_giant'] as const)('puts even seats against odd ones in %s', (mode) => {
    const view = withTeams(buildFixture({ seats: 4 }), mode).view
    expect(view.house_rules?.teams).toEqual({ mode, seats: [0, 1, 0, 1] })
  })

  it('gives each Two-Headed Giant team one life total', () => {
    const players = withTeams(buildFixture({ seats: 4 }), 'two_headed_giant').view.players
    expect(players[0]?.life).toBe(players[2]?.life)
    expect(players[1]?.life).toBe(players[3]?.life)
  })
})

describe('endedWith', () => {
  const table = buildFixture({ seats: 4 })
  const losers = (players: { seat: number; lost: unknown }[]) =>
    players.filter((player) => player.lost !== null).map((player) => player.seat)

  it.each([
    ['won', { Winner: 0 }, [1, 2, 3]],
    ['lost', { Winner: 1 }, [0, 2, 3]],
    ['draw', 'Draw', []],
  ] as const)('ends with %s', (kind, result, lost) => {
    const ended = endedWith(table, kind)
    expect(ended.result).toEqual(result)
    expect(losers(ended.players)).toEqual(lost)
  })

  it('lets the viewer’s team win a team game', () => {
    const ended = endedWith(withTeams(table, 'teams'), 'team')
    expect(ended.result).toEqual({ Team: 0 })
    expect(losers(ended.players)).toEqual([1, 3])
  })
})

describe('the route', () => {
  async function visit(url: string) {
    vi.resetModules()
    const { routeTree } = await import('@/routeTree.gen')
    const router = createRouter({ routeTree, history: createMemoryHistory({ initialEntries: [url] }) })
    render(<RouterProvider router={router} />)
    await act(async () => {
      await router.load()
    })
  }

  it('shows the result, then the final table on request', async () => {
    await visit('/dev/table?result=won')
    expect(
      await screen.findByRole('heading', { name: 'You won' }, { timeout: scaled(8000) }),
    ).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Look at the table and log' }))
    expect(await screen.findByRole('region', { name: 'Game table' })).toBeInTheDocument()
  }, 10_000)

  it('names the winning team', async () => {
    await visit('/dev/table?teams=2v2&result=team')
    expect(
      await screen.findByRole('heading', { name: 'Your team won' }, { timeout: scaled(8000) }),
    ).toBeInTheDocument()
  }, 10_000)
})
