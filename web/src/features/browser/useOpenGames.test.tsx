// Goal: the open-games hook asks at once and again 5 s after each answer with the listing it holds, keeps the same object (and renders nothing) on a 304, pauses while the tab is hidden and asks the moment it is visible, keeps the rows through a failure, waits out a 429, never doubles a request, drops an answer that was overtaken and stops when the page goes.
import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { LobbyError } from '@/net/lobbyErrors'
import { fakeLobbies, lobbyRows } from '@/test-utils/lobbies'
import { POLL_MS, useOpenGames } from './useOpenGames'

let hidden = false

beforeEach(() => {
  hidden = false
  vi.useFakeTimers()
  vi.spyOn(document, 'visibilityState', 'get').mockImplementation(() => (hidden ? 'hidden' : 'visible'))
})

afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
})

const settle = (ms = 0) => act(async () => void (await vi.advanceTimersByTimeAsync(ms)))
const setHidden = (value: boolean) =>
  act(() => {
    hidden = value
    document.dispatchEvent(new Event('visibilitychange'))
  })

async function open(rows = lobbyRows(2)) {
  const lobbies = fakeLobbies(rows)
  const renders = { count: 0 }
  const view = renderHook(() => {
    renders.count += 1
    return useOpenGames(lobbies.api)
  })
  await settle()
  return { lobbies, renders, ...view }
}

describe('polling', () => {
  it('asks at once, then 5 s after each answer, naming the listing it holds', async () => {
    const { lobbies, result } = await open()
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(1)
    expect(lobbies.listLobbies.mock.calls[0]?.[0]).toBeUndefined()
    const first = result.current.listing
    expect(first?.games).toHaveLength(2)

    await settle(POLL_MS - 1)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(1)
    await settle(1)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(2)
    expect(lobbies.listLobbies.mock.calls[1]?.[0]).toBe(first)
    await settle(POLL_MS)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(3)
  })

  it('keeps the same listing and renders nothing when the server answers 304', async () => {
    const { lobbies, renders, result } = await open()
    const first = result.current.listing
    // React may call the component once more for the first state that equals the old one, never for the next ones.
    await settle(POLL_MS)
    const drawn = renders.count
    await settle(POLL_MS * 3)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(5)
    expect(result.current.listing).toBe(first)
    expect(renders.count).toBe(drawn)
  })

  it('takes new rows when the server has them', async () => {
    const { lobbies, result } = await open()
    const first = result.current.listing
    lobbies.set(lobbyRows(5))
    await settle(POLL_MS)
    expect(result.current.listing).not.toBe(first)
    expect(result.current.listing?.games).toHaveLength(5)
  })

  it('never has two requests out at once', async () => {
    const lobbies = fakeLobbies(lobbyRows(1))
    const release = lobbies.hold()
    renderHook(() => useOpenGames(lobbies.api))
    await settle(POLL_MS * 6)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(1)
    release()
    await settle()
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(1)
    await settle(POLL_MS)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(2)
  })
})

describe('a hidden tab', () => {
  it('pauses, and asks again the moment the tab is visible', async () => {
    const { lobbies } = await open()
    await setHidden(true)
    await settle(POLL_MS * 10)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(1)
    await setHidden(false)
    await settle()
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(2)
    await settle(POLL_MS)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(3)
  })

  it('asks nothing when the page opens in a background tab until it is shown', async () => {
    hidden = true
    const lobbies = fakeLobbies(lobbyRows(1))
    const { result } = renderHook(() => useOpenGames(lobbies.api))
    await settle(POLL_MS * 3)
    expect(lobbies.listLobbies).not.toHaveBeenCalled()
    expect(result.current.listing).toBeNull()
    await setHidden(false)
    await settle()
    expect(result.current.listing?.games).toHaveLength(1)
  })

  it('drops an answer that comes back after the tab was hidden', async () => {
    const lobbies = fakeLobbies(lobbyRows(1))
    const release = lobbies.hold()
    const { result } = renderHook(() => useOpenGames(lobbies.api))
    await settle()
    await setHidden(true)
    release()
    await settle(POLL_MS * 2)
    expect(result.current.listing).toBeNull()
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(1)
  })
})

describe('failures', () => {
  it('keeps the rows, shows the error, and clears it when a later answer arrives', async () => {
    const { lobbies, result } = await open()
    const first = result.current.listing
    lobbies.fail(new LobbyError('network', 0, 'down'))
    await settle(POLL_MS)
    expect(result.current.error).toBeInstanceOf(LobbyError)
    expect(result.current.listing).toBe(first)
    lobbies.heal()
    await settle(POLL_MS)
    expect(result.current.error).toBeNull()
    expect(result.current.listing).toBe(first)
  })

  it('has no listing when the first request fails, and goes on asking', async () => {
    const lobbies = fakeLobbies(lobbyRows(1))
    lobbies.fail(new LobbyError('lobbies_unavailable', 503, 'busy'))
    const { result } = renderHook(() => useOpenGames(lobbies.api))
    await settle()
    expect(result.current.listing).toBeNull()
    expect(result.current.error).toBeInstanceOf(LobbyError)
    lobbies.heal()
    await settle(POLL_MS)
    expect(result.current.listing?.games).toHaveLength(1)
    expect(result.current.error).toBeNull()
  })

  it('waits as long as the server asked after a 429, and no longer', async () => {
    const lobbies = fakeLobbies(lobbyRows(1))
    lobbies.fail(new LobbyError('rate_limited', 429, 'slow down', 20))
    renderHook(() => useOpenGames(lobbies.api))
    await settle()
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(1)
    await settle(19_999)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(1)
    await settle(1)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(2)
  })

  it('does not poll faster than 5 s when the server asks for less', async () => {
    const lobbies = fakeLobbies(lobbyRows(1))
    lobbies.fail(new LobbyError('rate_limited', 429, 'slow down', 1))
    renderHook(() => useOpenGames(lobbies.api))
    await settle(POLL_MS - 1)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(1)
    await settle(1)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(2)
  })
})

describe('refresh', () => {
  it('asks now, shows it is working until the answer, and polls on from there', async () => {
    const { lobbies, result } = await open()
    await settle(POLL_MS - 100)
    const release = lobbies.hold()
    act(() => result.current.refresh())
    expect(result.current.refreshing).toBe(true)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(2)
    release()
    await settle()
    expect(result.current.refreshing).toBe(false)
    await settle(POLL_MS - 1)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(2)
    await settle(1)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(3)
  })

  it('lets the newest request win when an older one comes back late', async () => {
    const lobbies = fakeLobbies(lobbyRows(1))
    const release = lobbies.hold()
    const { result } = renderHook(() => useOpenGames(lobbies.api))
    await settle()
    lobbies.set(lobbyRows(3))
    act(() => result.current.refresh())
    await settle()
    expect(result.current.listing?.games).toHaveLength(3)
    release()
    await settle()
    expect(result.current.listing?.games).toHaveLength(3)
  })
})

describe('leaving the page', () => {
  it('stops asking, and ignores an answer that is still on its way', async () => {
    const lobbies = fakeLobbies(lobbyRows(1))
    const release = lobbies.hold()
    const view = renderHook(() => useOpenGames(lobbies.api))
    await settle()
    view.unmount()
    release()
    await settle(POLL_MS * 5)
    expect(lobbies.listLobbies).toHaveBeenCalledTimes(1)
    expect(vi.getTimerCount()).toBe(0)
  })
})
