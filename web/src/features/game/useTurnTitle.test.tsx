// Goal: the tab title gets "Your turn · " in front for exactly as long as it is the viewer's turn, a backgrounded tab with a decision waiting shows "● Your move" (flashing against the other title once a second, static under reduced motion) and goes quiet when the tab returns or the decision is answered, and nothing changes while the hook is disabled.
import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fixtureState } from '@/features/table/dev/devGame'
import { buildFixture } from '@/features/table/dev/fixture'
import { createPlayStore } from '@/play/store'
import type { PlayState, PlayStore } from '@/play/store'
import { clone, must } from '@/test-utils/batches'
import { FLASH_MS, useTurnTitle, YOUR_MOVE_TITLE, YOUR_TURN_PREFIX } from './useTurnTitle'

const MINE = `${YOUR_TURN_PREFIX}Game`

/** A store of a two-seat game in which seat 0 (the viewer) is the active seat or not, with or without a decision. */
function storeOf(options: { mine: boolean; deciding: boolean }): PlayStore {
  const fixture = buildFixture({ seats: 2, active: options.mine ? 0 : 1, permanents: 2 })
  const store = createPlayStore()
  store.setState(fixtureState(fixture, options.deciding ? undefined : null))
  return store
}

/** The hook over a new store, as a page that shows `title` would call it. */
function mount(options: { mine: boolean; deciding: boolean }, title: string, enabled: boolean) {
  const store = storeOf(options)
  return { store, ...renderHook(() => useTurnTitle(store, title, enabled)) }
}

const passTo = (store: PlayStore, active: number): void =>
  act(() =>
    store.setState((state: PlayState) => {
      const view = clone(must(state.view))
      view.turn = { ...view.turn, active }
      return { view }
    }),
  )

const answer = (store: PlayStore): void => act(() => store.setState({ pending: null }))

let hidden = false
function setHidden(next: boolean): void {
  hidden = next
  act(() => {
    document.dispatchEvent(new Event('visibilitychange'))
  })
}

// A media query list the test can flip, standing in for the operating system setting.
function stubReducedMotion(initial: boolean) {
  let matches = initial
  const listeners = new Set<() => void>()
  vi.stubGlobal('matchMedia', (query: string) => ({
    media: query,
    get matches() {
      return matches
    },
    addEventListener: (_type: string, listener: () => void) => listeners.add(listener),
    removeEventListener: (_type: string, listener: () => void) => listeners.delete(listener),
  }))
  return (next: boolean): void => {
    matches = next
    act(() => listeners.forEach((listener) => listener()))
  }
}

const tick = (ms: number): void => {
  act(() => {
    vi.advanceTimersByTime(ms)
  })
}

beforeEach(() => {
  hidden = false
  vi.useFakeTimers()
  vi.spyOn(document, 'visibilityState', 'get').mockImplementation(() => (hidden ? 'hidden' : 'visible'))
})

afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('the turn prefix', () => {
  it('leaves the title alone on someone else’s turn', () => {
    const { result } = mount({ mine: false, deciding: true }, 'Game', true)
    expect(result.current).toBe('Game')
  })

  it('puts "Your turn · " in front on the viewer’s turn', () => {
    const { result } = mount({ mine: true, deciding: true }, 'Game', true)
    expect(result.current).toBe(MINE)
  })

  it('follows the turn: on when it comes round, off when it passes, on again', () => {
    const store = storeOf({ mine: false, deciding: false })
    const { result } = renderHook(() => useTurnTitle(store, 'Game', true))
    expect(result.current).toBe('Game')
    passTo(store, 0)
    expect(result.current).toBe(MINE)
    passTo(store, 1)
    expect(result.current).toBe('Game')
    passTo(store, 0)
    expect(result.current).toBe(MINE)
  })

  it('keeps the prefix while the viewer decides, answers and waits within their own turn', () => {
    const store = storeOf({ mine: true, deciding: true })
    const { result } = renderHook(() => useTurnTitle(store, 'Game', true))
    answer(store)
    expect(result.current).toBe(MINE)
  })

  it('is off for a screen that is not the table, even on the viewer’s turn', () => {
    const { result } = mount({ mine: true, deciding: true }, 'Game over', false)
    expect(result.current).toBe('Game over')
  })

  it.each([
    ['the opening hands', { stage: 'Mulligan' as const }],
    ['a finished game', { stage: 'Finished' as const }],
  ])('is off in %s', (_when, change) => {
    const store = storeOf({ mine: true, deciding: false })
    const { result } = renderHook(() => useTurnTitle(store, 'Game', true))
    act(() => store.setState((state) => ({ view: { ...must(state.view), ...change } })))
    expect(result.current).toBe('Game')
  })
})

describe('the flashing title of a backgrounded tab', () => {
  it('stays quiet while the tab is in front, decision or not', () => {
    const { result } = mount({ mine: false, deciding: true }, 'Game', true)
    tick(5 * FLASH_MS)
    expect(result.current).toBe('Game')
  })

  it('stays quiet in the background when nothing is asked of the viewer', () => {
    setHidden(true)
    const { result } = mount({ mine: false, deciding: false }, 'Game', true)
    tick(5 * FLASH_MS)
    expect(result.current).toBe('Game')
  })

  it('flashes "● Your move" against the plain title once a second while hidden with a decision', () => {
    const { result } = mount({ mine: false, deciding: true }, 'Game', true)
    setHidden(true)
    expect(result.current).toBe(YOUR_MOVE_TITLE)
    tick(FLASH_MS - 1)
    expect(result.current).toBe(YOUR_MOVE_TITLE)
    tick(1)
    expect(result.current).toBe('Game')
    tick(FLASH_MS)
    expect(result.current).toBe(YOUR_MOVE_TITLE)
    tick(FLASH_MS)
    expect(result.current).toBe('Game')
  })

  it('flashes against the turn title when it is also the viewer’s turn', () => {
    const { result } = mount({ mine: true, deciding: true }, 'Game', true)
    setHidden(true)
    expect(result.current).toBe(YOUR_MOVE_TITLE)
    tick(FLASH_MS)
    expect(result.current).toBe(MINE)
  })

  it('starts on "● Your move" every time, however the last flash ended', () => {
    const store = storeOf({ mine: false, deciding: true })
    const { result } = renderHook(() => useTurnTitle(store, 'Game', true))
    setHidden(true)
    tick(FLASH_MS)
    expect(result.current).toBe('Game')
    setHidden(false)
    setHidden(true)
    expect(result.current).toBe(YOUR_MOVE_TITLE)
  })

  it('stops when the tab comes back, and no timer is left running', () => {
    const { result } = mount({ mine: false, deciding: true }, 'Game', true)
    setHidden(true)
    expect(result.current).toBe(YOUR_MOVE_TITLE)
    setHidden(false)
    expect(result.current).toBe('Game')
    expect(vi.getTimerCount()).toBe(0)
    tick(5 * FLASH_MS)
    expect(result.current).toBe('Game')
  })

  it('stops when the decision is answered, and starts again for the next one', () => {
    const store = storeOf({ mine: false, deciding: true })
    const { result } = renderHook(() => useTurnTitle(store, 'Game', true))
    setHidden(true)
    answer(store)
    expect(result.current).toBe('Game')
    expect(vi.getTimerCount()).toBe(0)
    act(() =>
      store.setState({
        pending: {
          id: 2,
          decision: {
            player: 0,
            kind: {
              Priority: {
                playable_lands: [],
                castable: [],
                mana_abilities: [],
                can_act: false,
                can_pass: true,
              },
            },
          },
        },
      }),
    )
    expect(result.current).toBe(YOUR_MOVE_TITLE)
  })

  it('stops for good when the hook goes away', () => {
    const { result, unmount } = mount({ mine: false, deciding: true }, 'Game', true)
    setHidden(true)
    expect(result.current).toBe(YOUR_MOVE_TITLE)
    unmount()
    expect(vi.getTimerCount()).toBe(0)
  })

  it('does nothing while disabled', () => {
    setHidden(true)
    const { result } = mount({ mine: true, deciding: true }, 'Lobby', false)
    tick(3 * FLASH_MS)
    expect(result.current).toBe('Lobby')
    expect(vi.getTimerCount()).toBe(0)
  })
})

describe('with reduced motion', () => {
  it('shows "● Your move" and keeps it still, with no timer at all', () => {
    stubReducedMotion(true)
    const { result } = mount({ mine: false, deciding: true }, 'Game', true)
    setHidden(true)
    expect(result.current).toBe(YOUR_MOVE_TITLE)
    expect(vi.getTimerCount()).toBe(0)
    tick(5 * FLASH_MS)
    expect(result.current).toBe(YOUR_MOVE_TITLE)
  })

  it('still marks the viewer’s turn with the prefix, which does not move', () => {
    stubReducedMotion(true)
    const { result } = mount({ mine: true, deciding: false }, 'Game', true)
    expect(result.current).toBe(MINE)
  })

  it('switches over when the setting changes while the tab is flashing', () => {
    const setReduced = stubReducedMotion(false)
    const { result } = mount({ mine: false, deciding: true }, 'Game', true)
    setHidden(true)
    tick(FLASH_MS)
    expect(result.current).toBe('Game')
    setReduced(true)
    expect(result.current).toBe(YOUR_MOVE_TITLE)
    expect(vi.getTimerCount()).toBe(0)
    setReduced(false)
    tick(FLASH_MS)
    expect(result.current).toBe('Game')
  })
})
