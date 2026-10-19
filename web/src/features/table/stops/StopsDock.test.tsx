// Goal: the stop toggles of the controls dock reach the same session calls whether the ladder is a row (1600 and up) or opens from the Stops button, the popover keeps the Keep toggle and the side picked while it is shut and does not stop the Control keys, every button of the dock is 32 px high or more (44 under a coarse pointer), and the table menu holds none of it.
import { act, screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { FullControlMode, Step, StopSide } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'

const SIZES: ReadonlyArray<[string, number, number]> = [
  ['a row', 1600, 1000],
  ['a popover on a laptop', 1024, 768],
  ['a popover on a phone', 390, 844],
]

/** A table whose viewer holds priority on their own turn, with spies for the stop and Full Control calls. */
function stopsOf(width: number, height: number) {
  const setStop = vi.fn((_side: StopSide, _step: Step, _on: boolean, _keep: boolean) => true)
  const setFullControl = vi.fn((_mode: FullControlMode) => true)
  const table = renderTable({
    options: { viewport: { width, height } },
    session: { setStop, setFullControl },
    state: (state) => {
      const view = clone(must(state.view))
      view.turn = { ...view.turn, step: 'Main1', active: 0 }
      return { ...state, view, status: 'open' }
    },
  })
  const open = async (): Promise<void> => {
    const button = screen.queryByRole('button', { name: /^Stops/ })
    if (button !== null && button.getAttribute('aria-expanded') === 'false') {
      await table.user.click(button)
      await screen.findByRole('dialog', { name: 'Stops and Full Control' })
    }
  }
  return { ...table, setStop, setFullControl, open }
}

const step = (side: StopSide, name: Step): HTMLElement =>
  must(
    within(document.body)
      .getAllByRole('button', { name: /stop: / })
      .find((button) => button.dataset['side'] === side && button.dataset['step'] === name),
  )
const keep = (): HTMLElement => screen.getByRole('button', { name: 'Keep stops' })

describe.each(SIZES)('the toggles in %s', (_name, width, height) => {
  it('send the same SetStop for a press, a shift press and a press with Keep on', async () => {
    const { user, open, setStop } = stopsOf(width, height)
    await open()
    await user.click(step('Own', 'Main1'))
    expect(setStop).toHaveBeenLastCalledWith('Own', 'Main1', true, false)
    await user.keyboard('{Shift>}')
    await user.click(step('Own', 'Main1'))
    await user.keyboard('{/Shift}')
    expect(setStop).toHaveBeenLastCalledWith('Own', 'Main1', true, true)
    await user.click(keep())
    expect(keep()).toHaveAttribute('aria-pressed', 'true')
    await user.click(step('Own', 'End'))
    expect(setStop).toHaveBeenLastCalledWith('Own', 'End', true, true)
    expect(setStop).toHaveBeenCalledTimes(3)
  })

  it('reach the other side after the switch, with the same arguments', async () => {
    const { user, open, setStop } = stopsOf(width, height)
    await open()
    await user.click(screen.getByRole('button', { name: /^Theirs/ }))
    await user.click(step('Opponent', 'Draw'))
    expect(setStop).toHaveBeenCalledExactlyOnceWith('Opponent', 'Draw', true, false)
  })

  it('send FullControl for the toggle and for each choice of its menu', async () => {
    const { user, open, setFullControl } = stopsOf(width, height)
    await open()
    await user.click(screen.getByRole('button', { name: 'Full Control: off' }))
    expect(setFullControl).toHaveBeenLastCalledWith('Held')
    await user.click(screen.getByRole('button', { name: 'Full Control options' }))
    await user.click(await screen.findByRole('menuitemradio', { name: /Next window only/ }))
    expect(setFullControl).toHaveBeenLastCalledWith('NextWindow')
  })
})

describe('the popover', () => {
  it('keeps the Keep toggle and the side picked while it is shut', async () => {
    const { user, open } = stopsOf(1024, 768)
    await open()
    await user.click(keep())
    await user.click(screen.getByRole('button', { name: /^Theirs/ }))
    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Stops and Full Control' })).toBeNull())
    expect(document.querySelectorAll('[data-slot="stop-ladder"]')).toHaveLength(1)
    await open()
    expect(keep()).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('button', { name: /^Theirs/ })).toHaveAttribute('aria-pressed', 'true')
  })

  it('closes on a press outside and leaves the stops as they were', async () => {
    const { user, open, setStop } = stopsOf(1024, 768)
    await open()
    await user.click(document.body)
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Stops and Full Control' })).toBeNull())
    expect(setStop).not.toHaveBeenCalled()
  })

  it('does not stop the Control keys, which ask for Full Control with it shut', async () => {
    const { user, setFullControl } = stopsOf(1024, 768)
    act(() => (document.activeElement as HTMLElement | null)?.blur())
    await user.keyboard('{Control}')
    expect(setFullControl).toHaveBeenLastCalledWith('NextWindow')
    await user.keyboard('{Control>}{Shift}{/Control}')
    expect(setFullControl).toHaveBeenLastCalledWith('Held')
    expect(setFullControl).toHaveBeenCalledTimes(2)
  })

  it('opens upward from the dock and says the same words as the row, in a dialog named for its content', async () => {
    const { open } = stopsOf(390, 844)
    await open()
    const panel = screen.getByRole('dialog', { name: 'Stops and Full Control' })
    expect(within(panel).getByRole('group', { name: 'Stops' })).toBeInTheDocument()
    expect(within(panel).getByRole('group', { name: 'Full Control' })).toBeInTheDocument()
    expect(panel.closest('[data-side]')).toHaveAttribute('data-side', 'top')
  })
})

describe('the targets of the dock', () => {
  /** Whether a button's classes make it 32 px high or more, and 44 under a coarse pointer unless it is already that. */
  const target = (button: HTMLElement): boolean => {
    const classes = button.className.split(/\s+/)
    const has = (name: string): boolean => classes.includes(name)
    const big = has('h-11') || has('size-11')
    const desktop = big || has('h-8') || has('size-8') || has('h-9') || has('size-9')
    const coarse = big || has('pointer-coarse:h-11') || has('pointer-coarse:size-11')
    return desktop && coarse
  }

  it.each(SIZES)(
    'are 32 px on desktop and 44 under a coarse pointer, in %s',
    async (_name, width, height) => {
      const { open } = stopsOf(width, height)
      await open()
      const buttons = [
        ...document.querySelectorAll<HTMLElement>(
          '[data-region="controls"] button, [data-slot="popover-content"] button',
        ),
      ].filter((button) => button.closest('[hidden]') === null)
      expect(buttons.length).toBeGreaterThan(10)
      expect(
        buttons
          .filter((button) => !target(button))
          .map((button) => button.getAttribute('aria-label') ?? button.textContent),
      ).toEqual([])
    },
  )
})
