// Goal: the Play page keeps itself current without disturbing anyone: it polls every 5 s while the tab is visible and stops while it is hidden, reuses the rows (the very same elements) on a 304, never moves focus when it updates, sends focus from a removed row to the next Join, leaves it alone when it is somewhere else, and is a table from the M breakpoint up and stacked rows under it, swapping as the window crosses it.
import { act, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installFakeClock, removeFakeClock, settle } from '@/test-utils/fakeClock'
import { fakeLobbies, lobbyRow, lobbyRows } from '@/test-utils/lobbies'
import type { FakeLobbies } from '@/test-utils/lobbies'
import { renderInRouter } from '@/test-utils/router'
import { PlayPage } from './PlayPage'
import { TABLE_FROM } from './OpenGameRows'
import { POLL_MS } from './useOpenGames'

let hidden = false

beforeEach(() => {
  hidden = false
  installFakeClock()
  vi.spyOn(document, 'visibilityState', 'get').mockImplementation(() => (hidden ? 'hidden' : 'visible'))
})

afterEach(() => {
  removeFakeClock()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

const user = () => userEvent.setup({ delay: null })

async function open(lobbies: FakeLobbies) {
  const view = await renderInRouter(<PlayPage api={lobbies.api} />)
  await settle()
  return { ...view, user: user() }
}

const joinOf = (host: string, seats = '2 of 4') =>
  screen.getByRole('button', { name: `Join ${host}'s game, ${seats} seats` })
const row = (id: string) => document.querySelector(`[data-game="${id}"]`)

const ANN = lobbyRow({ game: 'g1', host: 'Ann' })
const BO = lobbyRow({ game: 'g2', host: 'Bo' })
const CY = lobbyRow({ game: 'g3', host: 'Cy' })
const three = [ANN, BO, CY]

describe('polling', () => {
  it('asks every 5 s with the listing it holds, and keeps the very same rows when nothing changed', async () => {
    const lobbies = fakeLobbies(three)
    await open(lobbies)
    const before = three.map((game) => row(game.game))
    expect(before.every((node) => node !== null)).toBe(true)
    await settle(POLL_MS * 3)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(4)
    expect(lobbies.listLobbies.mock.calls[3]?.[0]?.games).toHaveLength(3)
    expect(three.map((game) => row(game.game))).toEqual(before)
    for (const [index, node] of before.entries())
      expect(three.map((game) => row(game.game))[index]).toBe(node)
  })

  it('shows a new game without remaking the rows that were there', async () => {
    const lobbies = fakeLobbies(three)
    await open(lobbies)
    const bo = row('g2')
    lobbies.set([lobbyRow({ game: 'g0', host: 'Zed' }), ...three])
    await settle(POLL_MS)
    expect(screen.getByText('4 open games')).toBeInTheDocument()
    expect(row('g0')).not.toBeNull()
    expect(row('g2')).toBe(bo)
  })

  it('stops while the tab is hidden and asks again the moment it is shown', async () => {
    const lobbies = fakeLobbies(three)
    await open(lobbies)
    act(() => {
      hidden = true
      document.dispatchEvent(new Event('visibilitychange'))
    })
    await settle(POLL_MS * 10)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(1)
    lobbies.set(lobbyRows(2))
    act(() => {
      hidden = false
      document.dispatchEvent(new Event('visibilitychange'))
    })
    await settle()
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(2)
    expect(screen.getByText('2 open games')).toBeInTheDocument()
  })
})

describe('focus while it updates', () => {
  it('stays on the Join that has it, however the list around it changes', async () => {
    const lobbies = fakeLobbies(three)
    await open(lobbies)
    const bo = joinOf('Bo')
    bo.focus()
    lobbies.set([lobbyRow({ game: 'g0', host: 'Zed' }), CY, BO])
    await settle(POLL_MS)
    expect(joinOf('Bo')).toBe(bo)
    expect(bo).toHaveFocus()
  })

  it('stays in the code field, with what was typed, through every update', async () => {
    const lobbies = fakeLobbies(three)
    const { user } = await open(lobbies)
    const field = screen.getByRole('textbox', { name: 'Join with a code' })
    await user.click(field)
    await user.keyboard('k7m2')
    lobbies.set(lobbyRows(1))
    await settle(POLL_MS * 2)
    expect(field).toHaveFocus()
    expect(field).toHaveValue('k7m2')
  })

  it('stays on Refresh through an update that fails and one that succeeds again', async () => {
    const lobbies = fakeLobbies(three)
    await open(lobbies)
    const refresh = screen.getByRole('button', { name: 'Refresh' })
    refresh.focus()
    lobbies.fail(new Error('down'))
    await settle(POLL_MS)
    expect(screen.getByRole('alert')).toBeInTheDocument()
    expect(refresh).toHaveFocus()
    lobbies.heal()
    lobbies.set(lobbyRows(2))
    await settle(POLL_MS)
    expect(refresh).toHaveFocus()
  })

  it('goes to the next Join when the row that had focus is gone', async () => {
    const lobbies = fakeLobbies(three)
    await open(lobbies)
    joinOf('Bo').focus()
    lobbies.set([ANN, CY])
    await settle(POLL_MS)
    expect(screen.queryByRole('button', { name: /Join Bo/ })).not.toBeInTheDocument()
    expect(joinOf('Cy')).toHaveFocus()
  })

  it('goes to the Join before it when the last row is the one that went', async () => {
    const lobbies = fakeLobbies(three)
    await open(lobbies)
    joinOf('Cy').focus()
    lobbies.set(three.slice(0, 2))
    await settle(POLL_MS)
    expect(joinOf('Bo')).toHaveFocus()
  })

  it('skips a full game for the next one that can be joined', async () => {
    const full = lobbyRow({ game: 'g3', host: 'Cy', filled: 4, total: 4 })
    const lobbies = fakeLobbies([ANN, BO, full, lobbyRow({ game: 'g4', host: 'Di' })])
    await open(lobbies)
    joinOf('Bo').focus()
    lobbies.set([ANN, full, lobbyRow({ game: 'g4', host: 'Di' })])
    await settle(POLL_MS)
    expect(joinOf('Di')).toHaveFocus()
  })

  it('goes to Refresh when no row is left', async () => {
    const lobbies = fakeLobbies(three)
    await open(lobbies)
    joinOf('Ann').focus()
    lobbies.set([])
    await settle(POLL_MS)
    expect(screen.getByRole('heading', { name: 'No open games' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Refresh' })).toHaveFocus()
  })

  it('leaves focus alone when it was somewhere else while a row went', async () => {
    const lobbies = fakeLobbies(three)
    await open(lobbies)
    const field = screen.getByRole('textbox', { name: 'Join with a code' })
    field.focus()
    lobbies.set([ANN])
    await settle(POLL_MS)
    expect(field).toHaveFocus()
  })

  it('leaves focus alone when nothing was focused', async () => {
    const lobbies = fakeLobbies(three)
    await open(lobbies)
    lobbies.set([ANN])
    await settle(POLL_MS)
    expect(document.body).toHaveFocus()
  })
})

describe('the layout', () => {
  /** A window whose width crosses the M breakpoint when `setWide` is called. */
  function stubWindow(wide: boolean) {
    const listeners = new Set<() => void>()
    let current = wide
    vi.stubGlobal('matchMedia', (query: string) => ({
      matches: query === TABLE_FROM ? current : false,
      addEventListener: (_type: string, listener: () => void) => void listeners.add(listener),
      removeEventListener: (_type: string, listener: () => void) => void listeners.delete(listener),
    }))
    return (next: boolean) =>
      act(() => {
        current = next
        for (const listener of listeners) listener()
      })
  }

  it('is a table with a caption from the M breakpoint up', async () => {
    stubWindow(true)
    await open(fakeLobbies(three))
    expect(screen.getByRole('table', { name: 'Open games, newest first' })).toBeInTheDocument()
    expect(screen.queryByRole('list', { name: 'Open games, newest first' })).not.toBeInTheDocument()
  })

  it('is stacked rows under it, with the same facts and the same Join names, and no table', async () => {
    stubWindow(false)
    await open(fakeLobbies([lobbyRow({ game: 'g1', host: 'Ann', filled: 4, total: 4 }), BO]))
    expect(screen.queryByRole('table')).not.toBeInTheDocument()
    const list = screen.getByRole('list', { name: 'Open games, newest first' })
    const [ann, bo] = within(list).getAllByRole('listitem')
    expect(bo).toHaveTextContent('Bo')
    expect(bo).toHaveTextContent('Normal · Your own deck · 3 min ago')
    expect(bo).toHaveTextContent('2 of 4')
    expect(
      within(bo as HTMLElement).getByRole('button', { name: "Join Bo's game, 2 of 4 seats" }),
    ).toBeEnabled()
    expect(ann).toHaveTextContent('Full')
    expect(
      within(ann as HTMLElement).getByRole('button', { name: "Join Ann's game, 4 of 4 seats" }),
    ).toHaveAttribute('aria-disabled', 'true')
    expect(within(bo as HTMLElement).getByText('3 min ago').tagName).toBe('TIME')
  })

  it('keeps the scrolling region and the count in both layouts', async () => {
    stubWindow(false)
    await open(fakeLobbies(three))
    expect(screen.getByRole('region', { name: 'Open games' })).toHaveAttribute('tabindex', '0')
    expect(screen.getByRole('status')).toHaveTextContent('3 open games')
  })

  it('swaps as the window crosses the breakpoint, and the Joins keep working', async () => {
    const setWide = stubWindow(true)
    await open(fakeLobbies(three))
    expect(screen.getByRole('table')).toBeInTheDocument()
    await setWide(false)
    expect(screen.queryByRole('table')).not.toBeInTheDocument()
    expect(screen.getAllByRole('listitem')).toHaveLength(3)
    await setWide(true)
    expect(screen.getByRole('table')).toBeInTheDocument()
    expect(screen.getAllByRole('button', { name: /^Join .* game/ })).toHaveLength(3)
  })

  it('moves focus to the next Join in the stacked layout too', async () => {
    stubWindow(false)
    const lobbies = fakeLobbies(three)
    await open(lobbies)
    joinOf('Ann').focus()
    lobbies.set(three.slice(1))
    await settle(POLL_MS)
    expect(joinOf('Bo')).toHaveFocus()
  })
})
