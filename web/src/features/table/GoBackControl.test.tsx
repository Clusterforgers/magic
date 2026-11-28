// Goal: the controls dock offers "Back to <step>" only for the seat's own offer while the game runs and the connection is up, sends GoBack on a press or on Backspace with nothing focused (never from a field or with a modifier), and words every refusal in a line that goes by itself; the log names the step and whose turn it is.
import { act, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { PlayState } from '@/play/store'
import type { GoBackOffer, GoBackRefusal, Step } from '@/protocol/generated'
import { renderTable } from '@/test-utils/tableHarness'
import { GO_BACK_REFUSAL_WORDS, goBackLabel, wentBackLine } from './goBackText'
import { TAKE_BACK_NOTICE_MS } from './takeBackText'

const offerOf = (step: Step, player = 0): GoBackOffer => ({ player, step, active: 0, at: 12 })

/** A table whose seat 0 holds the given go-back state, with spies on the session's go-back calls. */
function renderGoBack(change: Partial<PlayState>, seats = 2) {
  const goBack = vi.fn(() => true)
  const dismissGoBackRefusal = vi.fn()
  const table = renderTable({
    seats,
    session: { goBack, dismissGoBackRefusal },
    state: (state) => ({ ...state, phase: 'playing', status: 'open', ...change }),
  })
  const dock = (): HTMLElement => {
    const region = table.container.querySelector<HTMLElement>('[data-region="controls"]')
    if (region === null) throw new Error('no controls dock')
    return region
  }
  return { ...table, goBack, dismissGoBackRefusal, dock }
}

afterEach(() => {
  vi.useRealTimers()
})

describe('the Back button', () => {
  it.each([
    ['Main1', 'Back to Precombat main', 2],
    ['BeginCombat', 'Back to Beginning of combat', 4],
    ['End', 'Back to End step', 4],
  ] as const)('names the step %s and sends GoBack once', async (step, words, seats) => {
    const { goBack, dock, user } = renderGoBack({ goBackOffer: offerOf(step) }, seats)
    const button = within(dock()).getByRole('button', { name: words })
    expect(button).toHaveAttribute('aria-keyshortcuts', 'Backspace')
    await user.click(button)
    expect(goBack).toHaveBeenCalledTimes(1)
  })

  it('is not there without an offer, for another seat, once the game is over or while offline', () => {
    const { update } = renderGoBack({ goBackOffer: null })
    const button = () => screen.queryByRole('button', { name: /^Back to/ })
    expect(button()).toBeNull()
    update(() => ({ goBackOffer: offerOf('Main1', 1) }))
    expect(button()).toBeNull()
    update(() => ({ goBackOffer: offerOf('Main1'), phase: 'over' }))
    expect(button()).toBeNull()
    update(() => ({ phase: 'playing', status: 'reconnecting' }))
    expect(button()).toBeNull()
    update(() => ({ status: 'open' }))
    expect(button()).not.toBeNull()
  })

  it('goes back on Backspace with nothing focused, never from a field, with a modifier or held down', async () => {
    const { goBack, user, container } = renderGoBack({ goBackOffer: offerOf('Main1') })
    await user.keyboard('{Shift>}{Backspace}{/Shift}')
    expect(goBack).not.toHaveBeenCalled()
    const field = document.createElement('input')
    container.append(field)
    field.focus()
    await user.keyboard('{Backspace}')
    expect(goBack).not.toHaveBeenCalled()
    field.blur()
    await user.keyboard('{Backspace}')
    expect(goBack).toHaveBeenCalledTimes(1)
  })

  it('leaves Backspace alone without an offer', async () => {
    const { goBack, user } = renderGoBack({ goBackOffer: null })
    await user.keyboard('{Backspace}')
    expect(goBack).not.toHaveBeenCalled()
  })
})

describe('a refused go-back', () => {
  it.each(Object.entries(GO_BACK_REFUSAL_WORDS) as [GoBackRefusal, string][])(
    'says why (%s) and goes by itself',
    (reason, words) => {
      vi.useFakeTimers()
      const { dismissGoBackRefusal } = renderGoBack({ goBackRefused: reason })
      expect(screen.getByRole('status')).toHaveTextContent(words)
      act(() => {
        vi.advanceTimersByTime(TAKE_BACK_NOTICE_MS)
      })
      expect(dismissGoBackRefusal).toHaveBeenCalledTimes(1)
    },
  )
})

describe('the words', () => {
  const seat = (at: number): string => ['Ana', 'Bo'][at] ?? '?'

  it.each([
    [0, 'Main1', 0, 'Ana went back to the precombat main phase'],
    [1, 'End', 0, "Bo went back to Ana's end step"],
    [0, 'BeginCombat', 1, "Ana went back to Bo's beginning of combat"],
  ] as const)('seat %i back to %s of seat %i: %s', (player, step, active, line) => {
    expect(wentBackLine(player, step, active, seat)).toBe(line)
  })

  it('labels the button with the step', () => {
    expect(goBackLabel('Main2')).toBe('Back to Postcombat main')
  })
})
