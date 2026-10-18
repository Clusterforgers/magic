// Goal: while the server passes priority for the seat the table says so in one quiet line with a "Stop passing" button that only asks the session to end it; the line shows the server's word for each yield, goes with it, stays beside a decision the seat still has to answer, and cannot be pressed while the connection is down.
import { screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { PlayState } from '@/play/store'
import type { YieldUntil } from '@/protocol/generated'
import { decisionFor } from '@/test-utils/decisions'
import { maybePrimaryButton } from '@/test-utils/passControls'
import { renderTable } from '@/test-utils/tableHarness'

const YIELDS: [YieldUntil, string][] = [
  ['Resolve', 'Passing until the stack is empty'],
  ['EndOfTurn', 'Passing until the end of turn'],
  ['MyTurn', 'Passing until your next turn'],
]

/** A table whose seat is passing (or not), with no decision unless `pending` gives one. */
function renderYielding(
  yielding: YieldUntil | null,
  change: (state: PlayState) => PlayState = (state) => state,
) {
  const cancelYield = vi.fn(() => true)
  const table = renderTable({
    session: { cancelYield },
    state: (state) => change({ ...state, yielding, pending: null }),
  })
  const line = (): HTMLElement | null => table.container.querySelector('[data-slot="yield-indicator"]')
  return { ...table, cancelYield, line }
}

describe('while the server passes for the seat', () => {
  it.each(YIELDS)('%s: says "%s" and offers to stop', async (until, words) => {
    const { line } = renderYielding(until)
    expect(await screen.findByText(words)).toBeInTheDocument()
    expect(line()).toHaveAttribute('data-yield', until)
    expect(screen.getByRole('button', { name: 'Stop passing' })).toBeEnabled()
  })

  it('says it once, politely, as a status', async () => {
    renderYielding('EndOfTurn')
    const status = await screen.findByRole('status')
    expect(status).toHaveTextContent('Passing until the end of turn')
    expect(status).toHaveAttribute('aria-live', 'polite')
    expect(status).toHaveAttribute('aria-atomic', 'true')
  })

  it('shows no prompt of its own and none of the Priority controls', async () => {
    const { container } = renderYielding('MyTurn')
    await screen.findByText('Passing until your next turn')
    expect(container.querySelector('[data-decision]')).toBeNull()
    expect(maybePrimaryButton()).toBeNull()
  })

  it('asks the session to stop, once, and leaves the line to the server', async () => {
    const { cancelYield, store, line, user } = renderYielding('EndOfTurn')
    await user.click(await screen.findByRole('button', { name: 'Stop passing' }))
    expect(cancelYield).toHaveBeenCalledTimes(1)
    expect(store.getState().yielding).toBe('EndOfTurn')
    expect(line()).not.toBeNull()
  })

  it('goes when the server says the yield ended, and comes back with the next one', async () => {
    const { line, update } = renderYielding('EndOfTurn')
    await screen.findByText('Passing until the end of turn')
    update(() => ({ yielding: null }))
    expect(line()).toBeNull()
    expect(screen.queryByText(/Passing until/)).toBeNull()
    update(() => ({ yielding: 'Resolve' }))
    expect(await screen.findByText('Passing until the stack is empty')).toBeInTheDocument()
  })

  it('cannot be pressed while the connection is down, since the server would not hear it', async () => {
    const { cancelYield, user } = renderYielding('EndOfTurn', (state) => ({
      ...state,
      status: 'reconnecting',
    }))
    const stop = await screen.findByRole('button', { name: 'Stop passing' })
    expect(stop).toBeDisabled()
    await user.click(stop)
    expect(cancelYield).not.toHaveBeenCalled()
  })

  it('stays beside a decision the seat must still answer, such as the attack to declare', async () => {
    const { container } = renderYielding('EndOfTurn', (state) => ({
      ...state,
      pending: { id: 9, decision: decisionFor('DeclareAttackers') },
    }))
    await screen.findByText('Passing until the end of turn')
    expect(container.querySelector('[data-decision="DeclareAttackers"]')).not.toBeNull()
    expect(screen.getByRole('button', { name: 'Stop passing' })).toBeInTheDocument()
  })
})

describe('without a yield', () => {
  it('draws nothing', () => {
    const { line } = renderYielding(null)
    expect(line()).toBeNull()
    expect(screen.queryByRole('button', { name: 'Stop passing' })).toBeNull()
  })
})
