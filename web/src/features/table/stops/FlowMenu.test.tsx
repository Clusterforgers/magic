// Goal: the Full Control menu ends with the pace settings: "End my turn by hand" shows what the server holds and asks for the other value, the auto choices show what the server holds and ask for the one toggled, and the pinned stops are saved as the default of new games or reset, with the count in words.
import { screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flowPrefs, resetFlowPrefs, saveDefaultStops } from '@/play/flowPrefs'
import type { PlayState } from '@/play/store'
import type { AutoChoices, Stop } from '@/protocol/generated'
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

describe('auto choices', () => {
  const none: AutoChoices = { order_triggers: false, assign_damage: false, smart_stops: false }
  const all: AutoChoices = { order_triggers: true, assign_damage: true, smart_stops: true }
  const flag = {
    'Order my triggers by timestamp': 'order_triggers',
    'Assign combat damage for me': 'assign_damage',
    'Stop where my permanents can act': 'smart_stops',
  } as const
  it.each([
    ['Order my triggers by timestamp', none, { ...none, order_triggers: true }],
    ['Assign combat damage for me', none, { ...none, assign_damage: true }],
    ['Stop where my permanents can act', none, { ...none, smart_stops: true }],
    ['Order my triggers by timestamp', all, { ...all, order_triggers: false }],
    ['Assign combat damage for me', all, { ...all, assign_damage: false }],
    ['Stop where my permanents can act', all, { ...all, smart_stops: false }],
  ] as const)('%s from %o asks for %o', async (name, held, asked) => {
    const setAutoChoices = vi.fn((_choices: AutoChoices) => true)
    const { user } = renderTable({
      session: { setAutoChoices },
      state: withState({ autoChoices: held }),
    })
    await user.click(trigger())
    const item = await screen.findByRole('menuitemcheckbox', { name })
    const on = held[flag[name]]
    expect(item).toHaveAttribute('aria-checked', String(on))
    await user.click(item)
    expect(setAutoChoices).toHaveBeenCalledExactlyOnceWith(asked)
  })
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
