// Goal: the stack stops button sits in the auto-pass group in every layout, names and shows the level the server holds, its menu offers the three levels with what each does and asks the server for the one picked with every other auto choice kept, and it moves only when the server answers; a level is kept across a reload and sent at Welcome.
import { act, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { AutoChoices, StackStops } from '@/protocol/generated'
import type { PlayState } from '@/play/store'
import { flowPrefs, resetFlowPrefs, saveAutoChoices, settingRequests } from '@/play/flowPrefs'
import { isStackStops, stackStopsOf, withStackStops } from '@/play/stackStops'
import { renderTable } from '@/test-utils/tableHarness'
import { STACK_STOPS_WORDS } from './stackStops'

const OFF: AutoChoices = { order_triggers: false, assign_damage: false, smart_stops: false }
const LEVELS: StackStops[] = ['All', 'Opponents', 'Aimed']

function renderMenu(
  auto: AutoChoices,
  change: (state: PlayState) => PlayState = (state) => state,
  width = 1600,
) {
  const setAutoChoices = vi.fn((_choices: AutoChoices) => true)
  const table = renderTable({
    options: { viewport: { width, height: 900 } },
    session: { setAutoChoices },
    state: (state) => change({ ...state, autoChoices: auto, pending: null }),
  })
  const button = (): HTMLElement => screen.getByRole('button', { name: /^Stack stops: / })
  return { ...table, setAutoChoices, button }
}

describe('the stack stops button', () => {
  it.each([1920, 1600, 1280, 1024, 390])('at %i px sits in the group of Auto-skip triggers', (width) => {
    const { button } = renderMenu(OFF, undefined, width)
    const group = button().closest('[data-slot="auto-skip"]')
    expect(group).not.toBeNull()
    expect(group).toContainElement(screen.getByRole('switch', { name: 'Auto-skip triggers' }))
  })

  it.each(LEVELS)('names and marks the level %s the server holds', (level) => {
    const { button } = renderMenu(withStackStops(OFF, level))
    expect(button()).toHaveAccessibleName(`Stack stops: ${STACK_STOPS_WORDS[level].label.toLowerCase()}`)
    expect(button()).toHaveAttribute('data-level', level)
  })

  it.each(LEVELS.flatMap((from) => LEVELS.filter((to) => to !== from).map((to) => [from, to] as const)))(
    'from %s asks the server for %s, keeps the other choices and moves when the server answers',
    async (from, to) => {
      const held: AutoChoices = withStackStops({ ...OFF, skip_triggers: true, auto_pay: true }, from)
      const { button, user, setAutoChoices, store } = renderMenu(held)
      await user.click(button())
      const item = await screen.findByRole('menuitemradio', { name: STACK_STOPS_WORDS[to].label })
      expect(item).toHaveAccessibleDescription(STACK_STOPS_WORDS[to].description)
      expect(screen.getByRole('menuitemradio', { name: STACK_STOPS_WORDS[from].label })).toBeChecked()
      await user.click(item)
      const asked = withStackStops(held, to)
      expect(setAutoChoices).toHaveBeenCalledExactlyOnceWith(asked)
      expect(button()).toHaveAttribute('data-level', from)
      act(() => store.setState({ autoChoices: asked }))
      expect(button()).toHaveAttribute('data-level', to)
    },
  )

  it('cannot be opened while the connection is down', () => {
    const { button } = renderMenu(OFF, (state) => ({ ...state, status: 'closed' }))
    expect(button()).toBeDisabled()
  })

  it('says it is off while Full Control is on', () => {
    const { button } = renderMenu(withStackStops(OFF, 'Aimed'), (state) => ({
      ...state,
      fullControl: 'Held',
    }))
    expect(button()).toHaveAttribute('title', expect.stringContaining('Off while Full Control is on.'))
  })
})

describe('the stack stops level', () => {
  beforeEach(() => {
    window.localStorage.clear()
    resetFlowPrefs()
  })

  it.each(LEVELS)('reads %s back from the choices and leaves All out', (level) => {
    const choices = withStackStops(withStackStops(OFF, 'Aimed'), level)
    expect(stackStopsOf(choices)).toBe(level)
    expect('stack_stops' in choices).toBe(level !== 'All')
    expect(isStackStops(level)).toBe(true)
  })

  it.each(['', 'Mine', 1, null, undefined])('refuses %s as a level', (value) => {
    expect(isStackStops(value)).toBe(false)
  })

  it.each<[StackStops, StackStops, boolean]>([
    ['Opponents', 'All', true],
    ['Aimed', 'All', true],
    ['Aimed', 'Opponents', true],
    ['All', 'Aimed', true],
    ['Aimed', 'Aimed', false],
    ['All', 'All', false],
  ])('saved %s against a server holding %s is sent: %s', (saved, held, sent) => {
    saveAutoChoices(withStackStops(OFF, saved))
    resetFlowPrefs()
    const choices = flowPrefs().autoChoices
    expect(stackStopsOf(choices)).toBe(saved)
    expect('stack_stops' in choices).toBe(saved !== 'All')
    const requests = settingRequests({
      endTurnByHand: false,
      autoChoices: withStackStops(OFF, held),
      autoYields: [],
      fullControl: 'Off',
    })
    expect(requests).toEqual(sent ? [{ type: 'AutoChoices', choices }] : [])
  })
})
