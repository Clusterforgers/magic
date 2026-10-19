// Goal: the controls dock is where the pass controls of a Priority prompt are drawn, wherever the prompt itself is: in the dock while the table has one, in place where it has none, never twice, with the prompt still owning them (its state and its answers); the dock says that the server is passing for the seat, and holds the stop ladder as one row where the table says it has room and behind one Stops button where it has not.
import { act, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { useState } from 'react'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { ControlsDock, ControlsSlotProvider, InControlsDock } from './ControlsDock'

/** A button that counts its own presses, so a test can see that the same component, with its state, moved. */
function Counter() {
  const [count, setCount] = useState(0)
  return (
    <button type="button" onClick={() => setCount(count + 1)}>
      Pressed {count}
    </button>
  )
}

const slot = (): HTMLElement => {
  const found = document.querySelector<HTMLElement>('[data-slot="pass-slot"]')
  if (found === null) throw new Error('the dock has no slot')
  return found
}

describe('InControlsDock', () => {
  it('draws where it is called when the table has no dock', () => {
    renderTable({
      seats: 2,
      ui: (
        <div data-testid="origin">
          <InControlsDock>
            <Counter />
          </InControlsDock>
        </div>
      ),
    })
    expect(
      within(screen.getByTestId('origin')).getByRole('button', { name: 'Pressed 0' }),
    ).toBeInTheDocument()
  })

  it('draws in the dock and not where it is called, once the dock is in the page', () => {
    renderTable({
      seats: 2,
      ui: (
        <ControlsSlotProvider>
          <div data-testid="origin">
            <InControlsDock>
              <Counter />
            </InControlsDock>
          </div>
          <ControlsDock stopsRow={false} />
        </ControlsSlotProvider>
      ),
    })
    expect(within(screen.getByTestId('origin')).queryByRole('button')).toBeNull()
    expect(within(slot()).getByRole('button', { name: 'Pressed 0' })).toBeInTheDocument()
    expect(screen.getAllByRole('button', { name: /^Pressed/ })).toHaveLength(1)
  })

  it('keeps the state of what it draws and its place in the page across renders', async () => {
    const { user } = renderTable({
      seats: 2,
      ui: (
        <ControlsSlotProvider>
          <InControlsDock>
            <Counter />
          </InControlsDock>
          <ControlsDock stopsRow={false} />
        </ControlsSlotProvider>
      ),
    })
    await user.click(within(slot()).getByRole('button', { name: 'Pressed 0' }))
    expect(within(slot()).getByRole('button', { name: 'Pressed 1' })).toBeInTheDocument()
  })

  it('draws nothing while the table has a place for a dock that is not there', () => {
    renderTable({
      seats: 2,
      ui: (
        <ControlsSlotProvider>
          <div data-testid="origin">
            <InControlsDock>
              <Counter />
            </InControlsDock>
          </div>
        </ControlsSlotProvider>
      ),
    })
    expect(screen.queryByRole('button')).toBeNull()
  })

  it('takes the controls with it when the dock goes', () => {
    const table = renderTable({ seats: 2, options: { viewport: { width: 1600, height: 1000 } } })
    expect(document.querySelector('[data-slot="primary-action"]')).not.toBeNull()
    table.unmount()
    expect(document.querySelector('[data-slot="primary-action"]')).toBeNull()
  })
})

describe('the dock of the table', () => {
  it('is the region of the controls, with the primary button of the Priority prompt in it', () => {
    renderTable({ seats: 2, options: { viewport: { width: 1600, height: 1000 } } })
    const dock = screen.getByRole('region', { name: 'Pass controls' })
    expect(dock).toHaveAttribute('data-region', 'controls')
    expect(dock.querySelector('[data-slot="primary-action"]')).not.toBeNull()
    expect(dock.querySelector('[data-slot="secondary-action"]')).not.toBeNull()
    expect(dock.querySelector('[data-slot="more-passing"]')).not.toBeNull()
  })

  it('answers through the prompt: a press on the primary button sends the answer of the decision on screen', async () => {
    const answer = (given: unknown): boolean => {
      answered.push(JSON.stringify(given))
      return true
    }
    const answered: string[] = []
    const { user } = renderTable({ seats: 2, session: { answer } })
    await user.click(
      must(document.querySelector<HTMLElement>('[data-region="controls"] [data-slot="primary-action"]')),
    )
    expect(answered).toEqual(['{"Priority":"Pass"}'])
  })

  it('says that the server is passing for the seat, in the dock, in place of the buttons', () => {
    const table = renderTable({ seats: 2, options: { viewport: { width: 1600, height: 1000 } } })
    act(() => table.store.setState({ pending: null, yielding: 'EndOfTurn' }))
    const dock = screen.getByRole('region', { name: 'Pass controls' })
    expect(within(dock).getByText('Passing until the end of turn')).toBeInTheDocument()
    expect(within(dock).getByRole('button', { name: 'Stop passing' })).toBeInTheDocument()
    expect(dock.querySelector('[data-slot="primary-action"]')).toBeNull()
  })

  it('draws the stop ladder as a row when the table says it has room, and one Stops button otherwise', () => {
    const { unmount } = renderTable({
      seats: 2,
      ui: (
        <ControlsSlotProvider>
          <ControlsDock stopsRow />
        </ControlsSlotProvider>
      ),
    })
    expect(document.querySelector('[data-region="controls"] [data-slot="stop-ladder"]')).not.toBeNull()
    expect(document.querySelector('[data-slot="stops-button"]')).toBeNull()
    unmount()
    renderTable({
      seats: 2,
      ui: (
        <ControlsSlotProvider>
          <ControlsDock stopsRow={false} />
        </ControlsSlotProvider>
      ),
    })
    expect(document.querySelector('[data-region="controls"] [data-slot="stop-ladder"]')).toBeNull()
    expect(document.querySelector('[data-region="controls"] [data-slot="stops-button"]')).not.toBeNull()
  })

  it('scrolls inside itself when it holds more than it has room for, with the edge fade, never the page', () => {
    renderTable({ seats: 2 })
    const dock = screen.getByRole('region', { name: 'Pass controls' })
    expect(dock).toHaveClass('fade-scroll', 'min-h-0', 'min-w-0')
  })
})
