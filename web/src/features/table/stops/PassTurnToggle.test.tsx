// Goal: "Pass until my turn" sits beside "Auto-skip triggers" in every layout, shows only the yield the server holds, asks for `MyTurnThrough` when pressed on and cancels it when pressed off, is unavailable while Full Control is on or the connection is down, says what it does in its description, and its yield draws the line that says why the game moves on.
import { act, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { FullControlMode, YieldUntil } from '@/protocol/generated'
import type { PlayState } from '@/play/store'
import { renderTable } from '@/test-utils/tableHarness'

const NAME = 'Pass until my turn'

function renderToggle(
  yielding: YieldUntil | null,
  change: (state: PlayState) => PlayState = (state) => state,
  width = 1600,
) {
  const yieldUntil = vi.fn((_until: YieldUntil) => true)
  const cancelYield = vi.fn(() => true)
  const table = renderTable({
    options: { viewport: { width, height: 900 } },
    session: { yieldUntil, cancelYield },
    state: (state) => change({ ...state, yielding, pending: null }),
  })
  const toggle = (): HTMLElement => screen.getByRole('switch', { name: NAME })
  return { ...table, yieldUntil, cancelYield, toggle }
}

describe('the Pass until my turn switch', () => {
  it.each([1920, 1600, 1280, 1024, 390])('at %i px sits in the group of Auto-skip triggers', (width) => {
    const { toggle } = renderToggle(null, undefined, width)
    const group = toggle().closest('[data-slot="auto-skip"]')
    expect(group).not.toBeNull()
    expect(group).toContainElement(screen.getByRole('switch', { name: 'Auto-skip triggers' }))
  })

  it.each<[YieldUntil | null, boolean]>([
    [null, false],
    ['MyTurnThrough', true],
    ['MyTurn', false],
    ['EndOfTurn', false],
    ['Resolve', false],
  ])('with the server holding %s it is on: %s', (yielding, on) => {
    const { toggle } = renderToggle(yielding)
    expect(toggle()).toHaveAttribute('aria-checked', String(on))
  })

  it('asks for the yield when pressed on and moves only when the server says so', async () => {
    const { toggle, user, yieldUntil, store } = renderToggle(null)
    await user.click(toggle())
    expect(yieldUntil).toHaveBeenCalledExactlyOnceWith('MyTurnThrough')
    expect(toggle()).toHaveAttribute('aria-checked', 'false')
    act(() => store.setState({ yielding: 'MyTurnThrough' }))
    expect(toggle()).toHaveAttribute('aria-checked', 'true')
    act(() => store.setState({ yielding: null }))
    expect(toggle()).toHaveAttribute('aria-checked', 'false')
  })

  it('cancels the yield when pressed off', async () => {
    const { toggle, user, yieldUntil, cancelYield } = renderToggle('MyTurnThrough')
    await user.click(toggle())
    expect(cancelYield).toHaveBeenCalledTimes(1)
    expect(yieldUntil).not.toHaveBeenCalled()
  })

  it('says what it does', () => {
    const { toggle } = renderToggle(null)
    expect(toggle()).toHaveAccessibleDescription('Pass until my turn — only asks when you have a real choice')
  })

  it.each<[FullControlMode, string]>([
    ['Held', 'open'],
    ['NextWindow', 'open'],
    ['Off', 'closed'],
  ])('in Full Control %s with the socket %s it cannot be pressed', async (mode, status) => {
    const { toggle, user, yieldUntil } = renderToggle(null, (state) => ({
      ...state,
      fullControl: mode,
      status: status as PlayState['status'],
    }))
    expect(toggle()).toHaveAttribute('data-disabled')
    await user.click(toggle())
    expect(yieldUntil).not.toHaveBeenCalled()
  })

  it('says it is off while Full Control is on', () => {
    const { toggle } = renderToggle(null, (state) => ({ ...state, fullControl: 'Held' }))
    expect(toggle()).toHaveAccessibleDescription(
      'Pass until my turn — only asks when you have a real choice. Off while Full Control is on.',
    )
  })

  it('draws the line that says why the game moves on, with Stop passing', async () => {
    renderToggle('MyTurnThrough')
    expect(
      await screen.findByText('Passing until your turn; you are asked only real choices'),
    ).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Stop passing' })).toBeEnabled()
  })
})
