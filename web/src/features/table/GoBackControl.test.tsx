// Goal: the controls dock offers "Back to <step>" only for the seat's own offer while the game runs and the connection is up, sends GoBack on a press or on Backspace with nothing focused (never from a field or with a modifier), asks a seat waited on with Allow and Decline and has the asker wait with Cancel at two and four seats, and words every refusal and ending in a line that goes by itself; the log names the step and whose turn it is.
import { act, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { GoBackRequest, PlayState } from '@/play/store'
import type { GoBackOffer, GoBackRefusal, Step } from '@/protocol/generated'
import { renderTable } from '@/test-utils/tableHarness'
import { GO_BACK_REFUSAL_WORDS, goBackAsk, goBackLabel, waitingFor, wentBackLine } from './goBackText'
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

describe('a go-back request over other people’s passes', () => {
  const requestOf = (player: number, waiting: number[]): GoBackRequest => ({
    offer: { ...offerOf('Main1', player), active: player },
    waiting,
    deadlineMs: 15_000,
    at: Date.now(),
  })

  function renderRequest(change: Partial<PlayState>, seats: number) {
    const voteGoBack = vi.fn(() => true)
    const cancelGoBack = vi.fn(() => true)
    const dismissGoBackNotice = vi.fn()
    const table = renderTable({
      seats,
      session: { voteGoBack, cancelGoBack, dismissGoBackNotice },
      state: (state) => ({
        ...state,
        phase: 'playing',
        status: 'open',
        goBackOffer: offerOf('Main1'),
        ...change,
      }),
    })
    return { ...table, voteGoBack, cancelGoBack, dismissGoBackNotice }
  }

  it.each([2, 4])('asks a seat waited on, with Allow and Decline, at %i seats', async (seats) => {
    const { voteGoBack, user } = renderRequest({ goBackRequest: requestOf(1, [0]) }, seats)
    const group = screen.getByRole('group', { name: 'Go-back request' })
    expect(group).toHaveTextContent(/wants to go back to the precombat main phase/)
    expect(group).toHaveTextContent('15 s left')
    await user.click(within(group).getByRole('button', { name: 'Allow' }))
    await user.click(within(group).getByRole('button', { name: 'Decline' }))
    expect(voteGoBack.mock.calls).toEqual([[true], [false]])
    expect(
      screen.queryByRole('button', { name: /^Back to/ }),
      'no Back button while a request is open',
    ).toBeNull()
  })

  it.each([2, 4])('has the asker wait on the others with Cancel, at %i seats', async (seats) => {
    const waiting = Array.from({ length: seats - 1 }, (_, at) => at + 1)
    const { cancelGoBack, user, container } = renderRequest({ goBackRequest: requestOf(0, waiting) }, seats)
    const line = container.querySelector('[data-slot="go-back-waiting"]')
    expect(line).toHaveTextContent(/^Waiting for .+…/)
    expect(screen.queryByRole('group', { name: 'Go-back request' })).toBeNull()
    await user.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(cancelGoBack).toHaveBeenCalledTimes(1)
  })

  it('tells a seat that already answered whom the table waits on', () => {
    const { container } = renderRequest({ goBackRequest: requestOf(1, [2]) }, 4)
    expect(container.querySelector('[data-slot="go-back-waiting"]')).toHaveTextContent(
      /wants to go back to the precombat main phase\. Waiting for .+…/,
    )
    expect(screen.queryByRole('button', { name: 'Allow' })).toBeNull()
  })

  it.each([
    [{ type: 'Approved' }, /^Allowed: .+ goes back to the precombat main phase\.$/],
    [{ type: 'Declined', by: 1 }, /^Declined by .+: .+ does not go back\.$/],
    [{ type: 'TimedOut', by: 1 }, /^No answer from .+: .+ does not go back\.$/],
    [{ type: 'Withdrawn' }, /^Withdrawn: .+ does not go back\.$/],
  ] as const)('says how it ended (%o) and the line goes by itself', (outcome, words) => {
    vi.useFakeTimers()
    const { dismissGoBackNotice } = renderRequest({ goBackNotice: { offer: offerOf('Main1'), outcome } }, 2)
    expect(screen.getByRole('status')).toHaveTextContent(words)
    act(() => {
      vi.advanceTimersByTime(TAKE_BACK_NOTICE_MS)
    })
    expect(dismissGoBackNotice).toHaveBeenCalledTimes(1)
  })
})

describe('the words', () => {
  const seat = (at: number): string => ['Ana', 'Bo', 'Cy', 'Di'][at] ?? '?'

  it.each([
    [[1], 'Waiting for Bo…'],
    [[1, 2], 'Waiting for Bo and Cy…'],
    [[1, 2, 3], 'Waiting for Bo, Cy and Di…'],
  ])('waits on %o: %s', (waiting, line) => {
    expect(waitingFor(waiting, seat)).toBe(line)
  })

  it('asks in the asker’s turn and in another’s', () => {
    expect(goBackAsk({ player: 0, step: 'Main1', active: 0, at: 3 }, seat)).toBe(
      'Ana wants to go back to the precombat main phase',
    )
    expect(goBackAsk({ player: 0, step: 'End', active: 1, at: 3 }, seat)).toBe(
      "Ana wants to go back to Bo's end step",
    )
  })

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
