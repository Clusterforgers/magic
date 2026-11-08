// Goal: the Full Control menu ends with the pace settings: "End my turn by hand" shows what the server holds and asks for the other value, the auto choices show what the server holds and ask for the one toggled, the abilities the seat always yields to are listed and a click takes one back, and the pinned stops are saved as the default of new games or reset, with the count in words.
import { screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flowPrefs, resetFlowPrefs, saveDefaultStops } from '@/play/flowPrefs'
import type { PlayState } from '@/play/store'
import type { AutoChoices, AutoYield, Stop } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
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
  const none: AutoChoices = { order_triggers: false, assign_damage: false }
  it.each([
    ['Order my triggers by timestamp', none, { ...none, order_triggers: true }],
    ['Assign combat damage for me', none, { ...none, assign_damage: true }],
    [
      'Order my triggers by timestamp',
      { order_triggers: true, assign_damage: true },
      { order_triggers: false, assign_damage: true },
    ],
    [
      'Assign combat damage for me',
      { order_triggers: true, assign_damage: true },
      { order_triggers: true, assign_damage: false },
    ],
  ] as const)('%s from %o asks for %o', async (name, held, asked) => {
    const setAutoChoices = vi.fn((_choices: AutoChoices) => true)
    const { user } = renderTable({
      session: { setAutoChoices },
      state: withState({ autoChoices: held }),
    })
    await user.click(trigger())
    const item = await screen.findByRole('menuitemcheckbox', { name })
    const on = name.startsWith('Order') ? held.order_triggers : held.assign_damage
    expect(item).toHaveAttribute('aria-checked', String(on))
    await user.click(item)
    expect(setAutoChoices).toHaveBeenCalledExactlyOnceWith(asked)
  })
})

describe('always yielding to', () => {
  const warden: AutoYield = {
    name: 'Soul Warden',
    text: 'Whenever another creature enters, you gain 1 life.',
  }
  const ornithopter: AutoYield = { name: 'Ornithopter', text: '' }

  it('lists nothing while the seat yields to no ability', async () => {
    const { user } = renderTable({ state: withState({ autoYields: [] }) })
    await user.click(trigger())
    await screen.findByRole('menuitemcheckbox', { name: /End my turn by hand/ })
    expect(screen.queryByText('Always yielding to')).toBeNull()
  })

  it.each([
    [[warden], 0, 'Stop yielding to Soul Warden: Whenever another creature enters, you gain 1 life.'],
    [[warden, ornithopter], 1, 'Stop yielding to Ornithopter'],
  ] as const)('lists %o and a click on item %i (%s) takes that one back', async (held, at, name) => {
    const clearAutoYield = vi.fn((_ability: AutoYield) => true)
    const { user } = renderTable({
      session: { clearAutoYield },
      state: withState({ autoYields: [...held] }),
    })
    await user.click(trigger())
    expect(await screen.findByText('Always yielding to')).toBeInTheDocument()
    const items = screen.getAllByRole('menuitem', { name: /^Stop yielding to/ })
    expect(items).toHaveLength(held.length)
    expect(items[at]).toHaveAccessibleName(name)
    await user.click(must(items[at]))
    expect(clearAutoYield).toHaveBeenCalledExactlyOnceWith(held[at])
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
