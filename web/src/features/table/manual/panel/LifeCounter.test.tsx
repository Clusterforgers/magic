// Goal: the open life counter changes life with quick buttons and the keyboard without a mistake costing a log line: a run of presses is one `Life` action sent after 600 ms of quiet with the running total shown, Undo drops a run not sent or reverts the last change for a few seconds, one change is in flight at a time, and every state is announced.
import { act, createEvent, fireEvent, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ManualReply, ManualResult } from '@/play/manualLedger'
import type { ManualAction } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { CommandsPanel } from './CommandsPanel'
import { QUIET_MS, UNDO_MS } from './lifeRun'

const WIDE = { width: 1600, height: 1000 }
const START = 40

function counterTable(over = false) {
  const sent: ManualAction[] = []
  const replies: ManualReply[] = []
  const table = renderTable({
    seats: 2,
    options: { viewport: WIDE },
    ui: <CommandsPanel />,
    state: over
      ? (state) => {
          const view = clone(must(state.view))
          must(view.players[0]).lost = 'ZeroLife'
          return { ...state, view }
        }
      : undefined,
    session: {
      manual: (action, reply) => {
        sent.push(action)
        if (reply !== undefined) replies.push(reply)
        return true
      },
    },
  })
  const counter = screen
    .getByRole('group', { name: 'You' })
    .querySelector<HTMLElement>('[data-slot="life-counter"]')
  const shown = must(counter?.querySelector<HTMLElement>('[data-slot="life-total"]'))
  const life = (): number => must(table.store.getState().view).players[0]?.life ?? Number.NaN
  /** The server takes the oldest change and the next state carries it, in that order as the session does. */
  const answer = (result: ManualResult = { ok: true }): void => {
    const delta = sent[sent.length - replies.length]
    act(() => {
      replies.shift()?.(result)
      if (result.ok && delta !== undefined && 'Life' in delta) {
        const view = clone(must(table.store.getState().view))
        const me = must(view.players[0])
        me.life += delta.Life.delta
        table.store.setState({ view })
      }
    })
  }
  return { ...table, sent, counter: must(counter), shown, life, answer }
}

const press = (name: string): void => {
  fireEvent.click(screen.getByRole('button', { name }))
}
const wait = (ms: number): void => {
  act(() => {
    vi.advanceTimersByTime(ms)
  })
}
const total = (shown: HTMLElement): number => Number(shown.textContent)

beforeEach(() => {
  vi.useFakeTimers()
})
afterEach(() => {
  vi.useRealTimers()
})

describe('the counter', () => {
  it('sits open at the top of You with the total and the four quick buttons', () => {
    const { counter, shown } = counterTable()
    const you = screen.getByRole('group', { name: 'You' })
    expect(you.firstElementChild?.nextElementSibling).toBe(counter)
    expect(shown).toHaveTextContent(`${START}`)
    const change = within(counter).getByRole('group', { name: 'Change life' })
    expect(
      within(change)
        .getAllByRole('button')
        .map((button) => button.textContent),
    ).toEqual(['-5', '-1', '+1', '+5'])
    expect(
      within(change)
        .getAllByRole('button')
        .map((button) => button.getAttribute('aria-label')),
    ).toEqual(['Lose 5 life', 'Lose 1 life', 'Gain 1 life', 'Gain 5 life'])
  })

  it('keeps today’s row as "More life options" under it, closed', () => {
    counterTable()
    const more = screen.getByRole('button', { name: /^More life options/ })
    expect(more).toHaveAttribute('aria-expanded', 'false')
  })
})

describe('a run of presses', () => {
  it('sends nothing before the quiet and one action after it', () => {
    const { sent, shown } = counterTable()
    press('Lose 5 life')
    expect(total(shown)).toBe(START - 5)
    wait(QUIET_MS - 1)
    expect(sent).toEqual([])
    wait(1)
    expect(sent).toEqual([{ Life: { player: 0, delta: -5 } }])
  })

  it.each<[string, readonly [string, number][], number | null]>([
    [
      'three single losses',
      [
        ['Lose 1 life', 100],
        ['Lose 1 life', 100],
        ['Lose 1 life', 100],
      ],
      -3,
    ],
    [
      'gains and a loss',
      [
        ['Gain 5 life', 200],
        ['Gain 5 life', 200],
        ['Lose 1 life', 500],
      ],
      9,
    ],
    [
      'a press and its opposite',
      [
        ['Lose 5 life', 100],
        ['Gain 5 life', 100],
      ],
      null,
    ],
    [
      'a press every 599 ms',
      [
        ['Lose 1 life', 599],
        ['Lose 1 life', 599],
        ['Lose 1 life', 599],
      ],
      -3,
    ],
  ])('%s is one action after the last press', (_, presses, delta) => {
    const { sent, shown } = counterTable()
    for (const [name, gap] of presses) {
      press(name)
      wait(gap)
    }
    expect(sent).toEqual([])
    expect(total(shown)).toBe(START + (delta ?? 0))
    wait(QUIET_MS)
    expect(sent).toEqual(delta === null ? [] : [{ Life: { player: 0, delta } }])
  })

  it('waits another 600 ms after each press', () => {
    const { sent } = counterTable()
    press('Lose 1 life')
    wait(500)
    press('Lose 1 life')
    wait(500)
    expect(sent).toEqual([])
    wait(100)
    expect(sent).toEqual([{ Life: { player: 0, delta: -2 } }])
  })

  it('shows what is pending, in the seat’s words', () => {
    const { counter } = counterTable()
    press('Lose 5 life')
    press('Gain 1 life')
    expect(counter.querySelector('[data-slot="life-pending"]')).toHaveTextContent(
      `Not sent yet: -4 (${START} → ${START - 4} life)`,
    )
  })

  it('limits a run to what one action carries', () => {
    const { sent, shown } = counterTable()
    for (let i = 0; i < 205; i++) press('Gain 5 life')
    expect(total(shown)).toBe(START + 1000)
    wait(QUIET_MS)
    expect(sent).toEqual([{ Life: { player: 0, delta: 1000 } }])
  })

  it('sends the next run only after the first was answered, and shows the total once', () => {
    const { sent, shown, life, answer } = counterTable()
    press('Lose 5 life')
    wait(QUIET_MS)
    expect(sent).toHaveLength(1)
    expect(screen.getByRole('status')).toHaveTextContent('Sending: Lose 5 life…')
    expect(total(shown)).toBe(START - 5)
    press('Lose 1 life')
    expect(total(shown)).toBe(START - 6)
    wait(5 * QUIET_MS)
    expect(sent).toHaveLength(1)
    answer()
    expect(life()).toBe(START - 5)
    expect(total(shown)).toBe(START - 6)
    wait(QUIET_MS)
    expect(sent).toEqual([{ Life: { player: 0, delta: -5 } }, { Life: { player: 0, delta: -1 } }])
  })

  it('says Done with the log’s change when the server took it', () => {
    const { answer, counter } = counterTable()
    press('Gain 5 life')
    wait(QUIET_MS)
    answer()
    expect(within(counter).getByRole('status')).toHaveTextContent('Done: Gain 5 life.')
  })

  it('shows a refusal in the counter, takes the change off the total and offers no Undo', () => {
    const { answer, counter, shown } = counterTable()
    press('Lose 5 life')
    wait(QUIET_MS)
    answer({ ok: false, reason: 'the game is over' })
    const line = within(counter).getByRole('status')
    expect(line).toHaveTextContent('Refused: the game is over.')
    expect(line).toHaveAttribute('data-state', 'refused')
    expect(total(shown)).toBe(START)
    expect(screen.queryByRole('button', { name: /^Undo/ })).toBeNull()
  })

  it('sends a run that is pending when the panel goes away', () => {
    const { sent, unmount } = counterTable()
    press('Lose 5 life')
    unmount()
    expect(sent).toEqual([{ Life: { player: 0, delta: -5 } }])
  })
})

describe('Undo', () => {
  it('drops a run that was not sent: nothing is sent and the total is back', () => {
    const { sent, shown } = counterTable()
    press('Lose 5 life')
    press('Lose 1 life')
    fireEvent.click(screen.getByRole('button', { name: 'Undo -6' }))
    expect(total(shown)).toBe(START)
    expect(screen.getByRole('button', { name: 'Lose 5 life' })).toHaveFocus()
    wait(10 * QUIET_MS)
    expect(sent).toEqual([])
    expect(screen.queryByRole('button', { name: /^Undo/ })).toBeNull()
  })

  it('reverts a change the server took with the opposite change, once', () => {
    const { sent, shown, answer, life } = counterTable()
    press('Lose 5 life')
    wait(QUIET_MS)
    expect(screen.queryByRole('button', { name: /^Undo/ })).toBeNull()
    answer()
    fireEvent.click(screen.getByRole('button', { name: 'Undo -5' }))
    expect(sent[1]).toEqual({ Life: { player: 0, delta: 5 } })
    expect(total(shown)).toBe(START)
    answer()
    expect(life()).toBe(START)
    expect(total(shown)).toBe(START)
    expect(screen.getByRole('status')).toHaveTextContent('Done: Gain 5 life.')
    expect(screen.queryByRole('button', { name: /^Undo/ })).toBeNull()
  })

  it('is gone after a few seconds, so a late press cannot move life by surprise', () => {
    const { answer } = counterTable()
    press('Lose 5 life')
    wait(QUIET_MS)
    answer()
    wait(UNDO_MS - 1)
    expect(screen.getByRole('button', { name: 'Undo -5' })).toBeInTheDocument()
    wait(1)
    expect(screen.queryByRole('button', { name: /^Undo/ })).toBeNull()
  })

  it('is gone when life changed some other way since', () => {
    const { answer, store } = counterTable()
    press('Lose 5 life')
    wait(QUIET_MS)
    answer()
    act(() => {
      const view = clone(must(store.getState().view))
      must(view.players[0]).life -= 3
      store.setState({ view })
    })
    expect(screen.queryByRole('button', { name: /^Undo/ })).toBeNull()
  })

  it('is replaced by the new run’s own as soon as a button is pressed again', () => {
    const { answer } = counterTable()
    press('Lose 5 life')
    wait(QUIET_MS)
    answer()
    press('Gain 1 life')
    expect(screen.getByRole('button', { name: 'Undo +1' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Undo -5' })).toBeNull()
    expect(screen.queryByText(/^Done/)).toBeNull()
  })
})

describe('the keyboard, from any button of the counter', () => {
  it.each<[string, readonly string[], number]>([
    ['ArrowUp', ['ArrowUp'], 1],
    ['ArrowDown', ['ArrowDown'], -1],
    ['PageUp', ['PageUp'], 5],
    ['PageDown', ['PageDown'], -5],
    ['two PageDowns and an ArrowUp', ['PageDown', 'PageDown', 'ArrowUp'], -9],
  ])('%s moves the pending change by %i', (_, keys, delta) => {
    const { shown, sent } = counterTable()
    const focused = screen.getByRole('button', { name: 'Gain 1 life' })
    focused.focus()
    for (const key of keys) fireEvent.keyDown(focused, { key })
    expect(total(shown)).toBe(START + delta)
    wait(QUIET_MS)
    expect(sent).toEqual([{ Life: { player: 0, delta } }])
  })

  it('forgets the run with Escape', () => {
    const { shown, sent } = counterTable()
    const focused = screen.getByRole('button', { name: 'Lose 1 life' })
    fireEvent.keyDown(focused, { key: 'PageUp' })
    fireEvent.keyDown(focused, { key: 'Escape' })
    wait(QUIET_MS)
    expect(sent).toEqual([])
    expect(total(shown)).toBe(START)
  })

  it('leaves Enter to the button it is pressed on', () => {
    const { sent } = counterTable()
    const focused = screen.getByRole('button', { name: 'Lose 1 life' })
    const event = createEvent.keyDown(focused, { key: 'Enter' })
    fireEvent(focused, event)
    expect(event.defaultPrevented).toBe(false)
    expect(sent).toEqual([])
  })

  it('names the group and explains the keys to a screen reader', () => {
    counterTable()
    const group = screen.getByRole('group', { name: 'Change life' })
    const hint = document.getElementById(must(group.getAttribute('aria-describedby')))
    expect(hint).toHaveTextContent('Page Up and Page Down by 5')
  })

  it('has no spinbutton, which a prompt’s number field would share the table with', () => {
    counterTable()
    expect(screen.queryByRole('spinbutton')).toBeNull()
  })
})

describe('announcements', () => {
  it('says the total politely as it changes, and that it is not sent yet', () => {
    const { counter, answer } = counterTable()
    const live = counter.querySelector('[aria-live="polite"]')
    expect(live).toHaveTextContent(`: ${START} life`)
    press('Lose 5 life')
    expect(live).toHaveTextContent(`: ${START - 5} life, not sent yet`)
    wait(QUIET_MS)
    answer()
    expect(live).toHaveTextContent(`: ${START - 5} life`)
    expect(live).not.toHaveTextContent('not sent yet')
  })
})

describe('a seat that cannot act by hand', () => {
  it('shows the buttons inert and sends nothing', () => {
    const { sent, shown } = counterTable(true)
    const button = screen.getByRole('button', { name: 'Lose 5 life' })
    expect(button).toHaveAttribute('aria-disabled', 'true')
    press('Lose 5 life')
    fireEvent.keyDown(shown, { key: 'PageDown' })
    wait(10 * QUIET_MS)
    expect(sent).toEqual([])
    expect(total(shown)).toBe(START)
  })
})
