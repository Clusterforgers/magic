// Goal: the Full Control menu ends with the pace settings: "End my turn by hand" shows what the server holds and asks for the other value, and the pinned stops are saved as the default of new games or reset, with the count in words.
import { screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flowPrefs, resetFlowPrefs, saveDefaultStops } from '@/play/flowPrefs'
import type { PlayState } from '@/play/store'
import type { Stop } from '@/protocol/generated'
import { renderTable } from '@/test-utils/tableHarness'

const trigger = (): HTMLElement => screen.getByRole('button', { name: 'Full Control options' })
const pinned: Stop = { side: 'Own', step: 'Main1', keep: true }
const oneTurn: Stop = { side: 'Opponent', step: 'End', keep: false }
const withState =
  (patch: Partial<PlayState>) =>
  (state: PlayState): PlayState => ({ ...state, ...patch })

beforeEach(() => {
  window.localStorage.clear()
  resetFlowPrefs()
})

describe('end my turn by hand', () => {
  it.each([true, false])(
    'held %s: checked as the server says and a click asks for the other',
    async (held) => {
      const setEndTurnByHand = vi.fn((_on: boolean) => true)
      const { user } = renderTable({
        session: { setEndTurnByHand },
        state: withState({ endTurnByHand: held }),
      })
      await user.click(trigger())
      const item = await screen.findByRole('menuitemcheckbox', { name: /End my turn by hand/ })
      expect(item).toHaveAttribute('aria-checked', String(held))
      await user.click(item)
      expect(setEndTurnByHand).toHaveBeenCalledExactlyOnceWith(!held)
    },
  )
})

describe('default stops', () => {
  it('saves the pinned stops only, and says how many are saved', async () => {
    const { user } = renderTable({ state: withState({ stops: [pinned, oneTurn] }) })
    await user.click(trigger())
    expect(await screen.findByText('No default stops')).toBeInTheDocument()
    await user.click(await screen.findByRole('menuitem', { name: 'Save pinned stops as default' }))
    expect(flowPrefs().defaultStops).toEqual([pinned])
    await user.click(trigger())
    expect(await screen.findByText('Default: 1 pinned stop')).toBeInTheDocument()
  })

  it.each([
    ['no pinned stop to save', [oneTurn], 'Save pinned stops as default'],
    ['no default to reset', [pinned], 'Reset default stops'],
  ] as const)('is disabled with %s', async (_, stops, name) => {
    const { user } = renderTable({ state: withState({ stops: [...stops] }) })
    await user.click(trigger())
    expect(await screen.findByRole('menuitem', { name })).toHaveAttribute('aria-disabled', 'true')
  })

  it('reset forgets the saved default', async () => {
    saveDefaultStops([pinned])
    const { user } = renderTable()
    await user.click(trigger())
    await user.click(await screen.findByRole('menuitem', { name: 'Reset default stops' }))
    expect(flowPrefs().defaultStops).toEqual([])
  })
})
