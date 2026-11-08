// Goal: a signed-in account's pace preferences come from the server and are saved there on every change, never into local storage; an account with none takes the browser's, a failed load keeps the browser's, and signing out returns to local storage.
import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { PrefsApi } from '@/net/prefs'
import type { UserPrefs } from '@/protocol/generated'
import {
  AUTO_CHOICES_KEY,
  DEFAULT_STOPS_KEY,
  END_TURN_KEY,
  flowPrefs,
  resetFlowPrefs,
  saveDefaultStops,
  saveAutoChoices,
  saveEndTurnByHand,
} from './flowPrefs'
import { usePrefsSync } from './usePrefsSync'

const upkeep = { side: 'Own', step: 'Upkeep', keep: true } as const
const auto = { order_triggers: true, assign_damage: false, smart_stops: true }
const held: UserPrefs = { default_stops: [upkeep], end_turn_by_hand: true, auto_choices: auto }
const none: UserPrefs = {
  default_stops: [],
  end_turn_by_hand: false,
  auto_choices: { order_triggers: false, assign_damage: false, smart_stops: false },
}

function fakeApi(load: () => Promise<UserPrefs>) {
  const saved: UserPrefs[] = []
  const api: PrefsApi = {
    load,
    save: (prefs) => {
      saved.push(prefs)
      return Promise.resolve(prefs)
    },
  }
  return { api, saved }
}

beforeEach(() => {
  window.localStorage.clear()
  resetFlowPrefs()
})

describe('usePrefsSync', () => {
  it('uses the account preferences, saves changes to the server only, and returns to storage on sign-out', async () => {
    const { api, saved } = fakeApi(() => Promise.resolve(held))
    const { rerender } = renderHook(({ on }) => usePrefsSync(on, api), { initialProps: { on: true } })
    await waitFor(() => expect(flowPrefs().endTurnByHand).toBe(true))
    expect(flowPrefs().defaultStops).toEqual([upkeep])

    act(() => saveEndTurnByHand(false))
    await waitFor(() =>
      expect(saved).toEqual([{ default_stops: [upkeep], end_turn_by_hand: false, auto_choices: auto }]),
    )
    expect(window.localStorage.getItem(END_TURN_KEY)).toBeNull()

    act(() => saveDefaultStops([]))
    await waitFor(() => expect(saved).toHaveLength(2))
    expect(window.localStorage.getItem(DEFAULT_STOPS_KEY)).toBeNull()

    rerender({ on: false })
    expect(flowPrefs()).toEqual({ defaultStops: [], endTurnByHand: false, autoChoices: none.auto_choices })
  })

  it('saves a changed auto choice to the server and keeps it out of storage', async () => {
    const { api, saved } = fakeApi(() => Promise.resolve(held))
    renderHook(() => usePrefsSync(true, api))
    await waitFor(() => expect(flowPrefs().autoChoices).toEqual(auto))
    act(() => saveAutoChoices({ order_triggers: true, assign_damage: true, smart_stops: true }))
    await waitFor(() => expect(saved).toHaveLength(1))
    expect(saved[0]?.auto_choices).toEqual({ order_triggers: true, assign_damage: true, smart_stops: true })
    expect(window.localStorage.getItem(AUTO_CHOICES_KEY)).toBeNull()
  })

  it('gives an account with no preferences the ones this browser kept', async () => {
    window.localStorage.setItem(END_TURN_KEY, 'on')
    const { api, saved } = fakeApi(() => Promise.resolve(none))
    renderHook(() => usePrefsSync(true, api))
    await waitFor(() =>
      expect(saved).toEqual([{ default_stops: [], end_turn_by_hand: true, auto_choices: none.auto_choices }]),
    )
    expect(flowPrefs().endTurnByHand).toBe(true)
  })

  it('keeps the browser preferences when the load fails, and asks for nothing as a guest', async () => {
    window.localStorage.setItem(END_TURN_KEY, 'on')
    const failing = fakeApi(() => Promise.reject(new Error('down')))
    renderHook(() => usePrefsSync(true, failing.api))
    await act(() => Promise.resolve())
    expect(flowPrefs().endTurnByHand).toBe(true)

    const load = vi.fn(() => Promise.resolve(held))
    renderHook(() => usePrefsSync(false, fakeApi(load).api))
    expect(load).not.toHaveBeenCalled()
  })
})
