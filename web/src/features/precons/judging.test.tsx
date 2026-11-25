// Goal: a precon list the server is still judging (`judging`) is shown at once and read again every JUDGING_RETRY_MS until a current one comes, by the Precons page and by the deck choosers alike; a failed re-read keeps the list shown.
import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { usePreconChoices } from '@/features/game/usePreconChoices'
import { PreconError } from '@/net/precons'
import type { PreconList } from '@/protocol/generated'
import { preconItem } from './fakePrecons'
import { JUDGING_RETRY_MS, usePrecons } from './usePrecons'

const BEFORE = preconItem({ code: 'c18-before', verified: false })
const AFTER = preconItem({ code: 'c18-before', verified: true })

/** A precon client whose list answers with the next of `answers`, then the last one again. */
function preconsOf(...answers: (PreconList | Error)[]) {
  let at = 0
  const list = vi.fn(async () => {
    const answer = answers[Math.min(at, answers.length - 1)] ?? { precons: [], judging: false }
    at += 1
    if (answer instanceof Error) throw answer
    return answer
  })
  return { list }
}

/** The precons a hook shows, whichever hook it is. */
type Shown = () => { status: string; precons?: unknown }

const HOOKS: [string, (api: ReturnType<typeof preconsOf>) => Shown][] = [
  [
    'the Precons page',
    (api) => {
      const { result } = renderHook(() => usePrecons(api))
      return () => result.current.state
    },
  ],
  [
    'a deck chooser',
    (api) => {
      const { result } = renderHook(() => usePreconChoices(api))
      return () => result.current.state
    },
  ],
]

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true })
})

afterEach(() => {
  vi.useRealTimers()
})

describe.each(HOOKS)('%s', (_, mount) => {
  it('shows a list still judged at once and reads again until it is current, then stops', async () => {
    const api = preconsOf(
      { precons: [BEFORE], judging: true },
      { precons: [BEFORE], judging: true },
      { precons: [AFTER], judging: false },
    )
    const shown = mount(api)
    await waitFor(() => expect(shown()).toEqual({ status: 'ready', precons: [BEFORE] }))
    expect(api.list).toHaveBeenCalledTimes(1)

    await act(() => vi.advanceTimersByTimeAsync(JUDGING_RETRY_MS))
    await waitFor(() => expect(api.list).toHaveBeenCalledTimes(2))
    expect(shown()).toEqual({ status: 'ready', precons: [BEFORE] })

    await act(() => vi.advanceTimersByTimeAsync(JUDGING_RETRY_MS))
    await waitFor(() => expect(shown()).toEqual({ status: 'ready', precons: [AFTER] }))

    await act(() => vi.advanceTimersByTimeAsync(JUDGING_RETRY_MS * 3))
    expect(api.list).toHaveBeenCalledTimes(3)
  })

  it('keeps a list still judged when the re-read fails', async () => {
    const api = preconsOf({ precons: [BEFORE], judging: true }, new PreconError('network', 0, 'offline'))
    const shown = mount(api)
    await waitFor(() => expect(shown()).toEqual({ status: 'ready', precons: [BEFORE] }))
    await act(() => vi.advanceTimersByTimeAsync(JUDGING_RETRY_MS))
    await waitFor(() => expect(api.list).toHaveBeenCalledTimes(2))
    expect(shown()).toEqual({ status: 'ready', precons: [BEFORE] })
  })
})
