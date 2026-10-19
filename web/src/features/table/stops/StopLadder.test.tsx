// Goal: the stop ladder draws one row of buttons, a button for each step that gives priority, for the side on show (Mine, your turns, or Theirs, opponents') with a switch that follows the turn and holds a side picked by hand until the turn changes hands, keeps both rows in the page with the other one hidden, lights a button when the server holds a stop (pinned when it lasts), marks the current step and whose turn it is, sends the right SetStop for every state and kind of press (click, shift-click, the Keep toggle, keyboard), shows only what the server says, cannot be used offline, and says every button in words.
import { act, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ConnectionStatus } from '@/net/socket'
import type { Session } from '@/play/session'
import { STOP_STEPS } from '@/play/stops'
import type { StopStep } from '@/play/stops'
import type { PlayState } from '@/play/store'
import type { Stage, Step, Stop, StopSide } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { welcome } from '@/test-utils/socketSetup'
import { renderLiveTable, renderTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'
import { rowName, stopName } from './stopModel'
import type { StopState } from './stopModel'

const SIDES: StopSide[] = ['Own', 'Opponent']
const OTHER: Record<StopSide, StopSide> = { Own: 'Opponent', Opponent: 'Own' }
const STATES: StopState[] = ['off', 'turn', 'lasting']
const TEN: StopStep[] = STOP_STEPS.filter((step) => step !== 'FirstStrikeDamage')

interface Setup {
  stops?: Stop[]
  /** The step the game is in. */
  step?: Step
  /** The seat whose turn it is; the viewer is seat 0. */
  active?: number
  stage?: Stage
  status?: ConnectionStatus
  seats?: number
  session?: Partial<Session>
}

/** A table whose viewer is seat 0, in `step` of `active`'s turn, holding `stops`. */
function ladder({
  stops = [],
  step = 'Main1',
  active = 1,
  stage,
  status = 'open',
  seats,
  session,
}: Setup = {}) {
  return renderTable({
    seats,
    session,
    state: (state) => {
      const view = clone(must(state.view))
      view.turn = { ...view.turn, step, active }
      if (stage !== undefined) view.stage = stage
      return { ...state, view, stops, status }
    },
  })
}

const stop = (side: StopSide, step: Step): HTMLElement =>
  must(document.querySelector<HTMLElement>(`[data-slot="stop"][data-side="${side}"][data-step="${step}"]`))
const row = (side: StopSide): HTMLElement =>
  must(document.querySelector<HTMLElement>(`[data-slot="stop-row"][data-side="${side}"]`))
const buttons = (side: StopSide): HTMLElement[] => [
  ...row(side).querySelectorAll<HTMLElement>('[data-slot="stop"]'),
]
const lit = (): string[] =>
  [...document.querySelectorAll<HTMLElement>('[data-slot="stop"][aria-pressed="true"]')].map(
    (button) => `${button.dataset['side']}:${button.dataset['step']}:${button.dataset['keep']}`,
  )
const keepToggle = (): HTMLElement => screen.getByRole('button', { name: 'Keep stops' })
/** The segment of the Mine or Theirs switch of a side. */
const segment = (side: StopSide): HTMLElement =>
  must(document.querySelector<HTMLElement>(`[data-slot="stop-side"] [data-side="${side}"]`))
/** The row on show: the one of the side the switch has pressed. */
const shownSide = (): StopSide[] =>
  SIDES.filter((side) => segment(side).getAttribute('aria-pressed') === 'true')
const held = (side: StopSide, step: Step, state: StopState): Stop[] =>
  state === 'off' ? [] : [{ side, step, keep: state === 'lasting' }]

describe('the rows', () => {
  it('are a group for your turns and one for opponents’, each with the ten steps in turn order, one on show', () => {
    ladder()
    expect(screen.getByRole('group', { name: 'Stops' })).toBeInTheDocument()
    for (const side of SIDES) {
      expect(buttons(side).map((button) => button.dataset['step'])).toEqual(TEN)
      expect(buttons(side).every((button) => button.dataset['side'] === side)).toBe(true)
    }
    // An opponent's turn is under way, so its row is on show and yours is out of sight and out of the page for a screen reader.
    expect(
      screen.getByRole('group', { name: "Stops on opponents' turns, an opponent's turn is under way" }),
    ).toBe(row('Opponent'))
    expect(row('Opponent')).not.toHaveAttribute('hidden')
    expect(row('Own')).toHaveAttribute('hidden')
    expect(screen.queryByRole('group', { name: 'Stops on your turns' })).toBeNull()
  })

  it('light nothing while the server holds no stop', () => {
    ladder()
    expect(lit()).toEqual([])
    expect(document.querySelectorAll('[data-slot="stop"]')).toHaveLength(20)
  })

  it('are chosen with a switch of Mine and Theirs, which names the side and says how many stops it holds', () => {
    ladder({ stops: [{ side: 'Own', step: 'Main1', keep: false }] })
    const switchGroup = screen.getByRole('group', { name: 'Stops for' })
    expect(within(switchGroup).getAllByRole('button')).toEqual([segment('Own'), segment('Opponent')])
    expect(segment('Own')).toHaveAccessibleName('Mine, 1 set')
    expect(segment('Opponent')).toHaveAccessibleName('Theirs, none set')
    expect(segment('Own')).toHaveTextContent('Mine')
    expect(segment('Opponent')).toHaveTextContent('Theirs')
  })

  it('sit beside the Full Control toggle in one region', () => {
    ladder()
    const region = screen.getByRole('region', { name: 'Stops and Full Control' })
    expect(region).toContainElement(screen.getByRole('group', { name: 'Stops' }))
    expect(region).toContainElement(screen.getByRole('group', { name: 'Full Control' }))
  })

  it('add first-strike damage in the step that has it, and only then', () => {
    ladder({ step: 'FirstStrikeDamage' })
    for (const side of SIDES) {
      expect(buttons(side)).toHaveLength(11)
      expect(stop(side, 'FirstStrikeDamage')).toHaveAccessibleName(
        stopName(side, 'FirstStrikeDamage', 'off', false),
      )
    }
  })

  it.each(SIDES)(
    'keep first-strike damage while a stop is held there on the %s side, so it can be cleared',
    (side) => {
      ladder({ stops: [{ side, step: 'FirstStrikeDamage', keep: true }] })
      for (const each of SIDES) expect(buttons(each)).toHaveLength(11)
      expect(lit()).toEqual([`${side}:FirstStrikeDamage:lasting`])
    },
  )

  it('draw no untap and no cleanup', () => {
    ladder()
    expect(document.querySelector('[data-step="Untap"], [data-step="Cleanup"]')).toBeNull()
  })
})

describe('every stop, in every state', () => {
  const CASES = SIDES.flatMap((side) =>
    TEN.flatMap((step) => STATES.map((state): [StopSide, StopStep, StopState] => [side, step, state])),
  )

  it.each(CASES)('draws %s %s as %s', (side, step, state) => {
    ladder({ stops: held(side, step, state) })
    const button = stop(side, step)
    expect(button).toHaveAccessibleName(stopName(side, step, state, false))
    expect(button).toHaveAttribute('aria-pressed', String(state !== 'off'))
    if (state === 'off') expect(button).not.toHaveAttribute('data-keep')
    else expect(button).toHaveAttribute('data-keep', state)
    // A lasting stop carries a pin, so it does not rest on colour; a one-turn stop and a stop that is off carry none.
    expect(button.querySelector('.lucide-pin') !== null).toBe(state === 'lasting')
    expect(lit()).toHaveLength(state === 'off' ? 0 : 1)
  })

  it('draws both sides of one step independently', () => {
    ladder({
      stops: [
        { side: 'Own', step: 'End', keep: false },
        { side: 'Opponent', step: 'End', keep: true },
      ],
    })
    expect(lit()).toEqual(['Own:End:turn', 'Opponent:End:lasting'])
  })

  it('draws all the stops the server holds, however many', () => {
    const everything = SIDES.flatMap((side) =>
      STOP_STEPS.map((step, n): Stop => ({ side, step, keep: n % 2 === 0 })),
    )
    ladder({ stops: everything })
    expect(lit()).toHaveLength(22)
    expect(document.querySelectorAll('[data-slot="stop"] .lucide-pin')).toHaveLength(12)
  })

  it('follows the server when its list changes, in either direction', () => {
    const { update } = ladder()
    update(() => ({ stops: [{ side: 'Own', step: 'Draw', keep: false }] }))
    expect(lit()).toEqual(['Own:Draw:turn'])
    update(() => ({ stops: [{ side: 'Own', step: 'Draw', keep: true }] }))
    expect(lit()).toEqual(['Own:Draw:lasting'])
    update(() => ({ stops: [] }))
    expect(lit()).toEqual([])
  })
})

describe('the current step and whose turn it is', () => {
  const CASES = TEN.flatMap((step) => SIDES.map((side): [StopStep, StopSide] => [step, side]))

  it.each(CASES)('marks %s on the %s side', (step, side) => {
    ladder({ step, active: side === 'Own' ? 0 : 1 })
    const current = [...document.querySelectorAll('[aria-current="step"]')]
    expect(current).toEqual([stop(side, step)])
    expect(stop(side, step)).toHaveAttribute('data-current', 'true')
    expect(row(side)).toHaveAttribute('data-turn', 'true')
    expect(row(side)).toHaveAccessibleName(rowName(side, true))
    expect(row(OTHER[side])).not.toHaveAttribute('data-turn')
    // The other row is out of sight, so a screen reader is not told its name; the page still holds it.
    expect(row(OTHER[side])).toHaveAttribute('aria-label', rowName(OTHER[side], false))
    // The row of the turn is the one on show, and its segment of the switch is marked.
    expect(shownSide()).toEqual([side])
    expect(segment(side)).toHaveAttribute('data-turn', 'true')
    expect(segment(OTHER[side])).not.toHaveAttribute('data-turn')
  })

  it('marks first-strike damage when that is the step', () => {
    ladder({ step: 'FirstStrikeDamage', active: 0 })
    expect([...document.querySelectorAll('[aria-current="step"]')]).toEqual([
      stop('Own', 'FirstStrikeDamage'),
    ])
  })

  it.each(['Untap', 'Cleanup'] satisfies Step[])(
    'marks no step in %s, which is not on the ladder, but still whose turn it is',
    (step) => {
      ladder({ step, active: 0 })
      expect(document.querySelector('[aria-current]')).toBeNull()
      expect(row('Own')).toHaveAttribute('data-turn', 'true')
    },
  )

  it.each(['PreGame', 'Mulligan', 'Finished'] satisfies Stage[])(
    'marks nothing in the %s stage, which has no turns',
    (stage) => {
      ladder({ stage, active: 0 })
      expect(document.querySelector('[aria-current]')).toBeNull()
      expect(document.querySelector('[data-turn]')).toBeNull()
    },
  )

  it.each([2, 3, 4])('calls the turn of seat %i, whoever it is, the opponents’', (active) => {
    ladder({ seats: 5, active, step: 'DeclareAttackers' })
    expect(row('Opponent')).toHaveAttribute('data-turn', 'true')
    expect(row('Own')).not.toHaveAttribute('data-turn')
    expect([...document.querySelectorAll('[aria-current="step"]')]).toEqual([
      stop('Opponent', 'DeclareAttackers'),
    ])
  })

  it('moves the marker as the game moves on', () => {
    const { update } = ladder({ step: 'Upkeep', active: 1 })
    expect(stop('Opponent', 'Upkeep')).toHaveAttribute('aria-current', 'step')
    update((state) => {
      const view = clone(must(state.view))
      view.turn = { ...view.turn, step: 'Draw' }
      return { view }
    })
    expect(stop('Opponent', 'Upkeep')).not.toHaveAttribute('aria-current')
    expect(stop('Opponent', 'Draw')).toHaveAttribute('aria-current', 'step')
    update((state) => {
      const view = clone(must(state.view))
      view.turn = { ...view.turn, step: 'Upkeep', active: 0 }
      return { view }
    })
    expect(row('Opponent')).not.toHaveAttribute('data-turn')
    expect(stop('Own', 'Upkeep')).toHaveAttribute('aria-current', 'step')
  })
})

describe('pressing a stop', () => {
  interface Press {
    state: StopState
    shift: boolean
    keep: boolean
    /** What the session is asked, after the side and step. */
    asks: [on: boolean, keep: boolean]
  }
  const PRESSES: Press[] = [
    { state: 'off', shift: false, keep: false, asks: [true, false] },
    { state: 'off', shift: true, keep: false, asks: [true, true] },
    { state: 'off', shift: false, keep: true, asks: [true, true] },
    { state: 'off', shift: true, keep: true, asks: [true, false] },
    { state: 'turn', shift: false, keep: false, asks: [false, false] },
    { state: 'turn', shift: true, keep: false, asks: [true, true] },
    { state: 'turn', shift: false, keep: true, asks: [true, true] },
    { state: 'turn', shift: true, keep: true, asks: [false, false] },
    { state: 'lasting', shift: false, keep: false, asks: [false, true] },
    { state: 'lasting', shift: true, keep: false, asks: [false, true] },
    { state: 'lasting', shift: false, keep: true, asks: [false, true] },
    { state: 'lasting', shift: true, keep: true, asks: [false, true] },
  ]
  const CASES = SIDES.flatMap((side) =>
    PRESSES.map(
      ({ state, shift, keep, asks }): [StopSide, StopState, boolean, boolean, [boolean, boolean]] => [
        side,
        state,
        shift,
        keep,
        asks,
      ],
    ),
  )

  it.each(CASES)(
    'on %s with the stop %s, shift %s and Keep %s asks %j',
    async (side, state, shift, keep, asks) => {
      const setStop = vi.fn((_side: StopSide, _step: Step, _on: boolean, _keep: boolean) => true)
      const { user } = ladder({ stops: held(side, 'Main2', state), session: { setStop } })
      if (keep) await user.click(keepToggle())
      if (shift) await user.keyboard('{Shift>}')
      await user.click(stop(side, 'Main2'))
      if (shift) await user.keyboard('{/Shift}')
      expect(setStop).toHaveBeenCalledExactlyOnceWith(side, 'Main2', ...asks)
    },
  )

  it.each(TEN)('asks for the step it is on: %s', async (step) => {
    const setStop = vi.fn((_side: StopSide, _step: Step, _on: boolean, _keep: boolean) => true)
    const { user } = ladder({ session: { setStop } })
    await user.click(stop('Opponent', step))
    expect(setStop).toHaveBeenCalledExactlyOnceWith('Opponent', step, true, false)
  })

  it('shows nothing new until the server says so', async () => {
    const setStop = vi.fn((_side: StopSide, _step: Step, _on: boolean, _keep: boolean) => true)
    const { user, store } = ladder({ session: { setStop } })
    await user.click(stop('Own', 'Draw'))
    expect(store.getState().stops).toEqual([])
    expect(lit()).toEqual([])
    expect(stop('Own', 'Draw')).toHaveAttribute('aria-pressed', 'false')
  })

  it('reads the stops the server holds at the moment of the press, not those of the last render', async () => {
    const setStop = vi.fn((_side: StopSide, _step: Step, _on: boolean, _keep: boolean) => true)
    const { user, store } = ladder({ session: { setStop } })
    const button = stop('Own', 'Draw')
    // The server's word arrives and the page has not redrawn when the press lands.
    store.setState({ stops: [{ side: 'Own', step: 'Draw', keep: false }] })
    await user.click(button)
    expect(setStop).toHaveBeenCalledExactlyOnceWith('Own', 'Draw', false, false)
  })

  it('sets a one-turn stop, clears it, and sets a lasting one, each shown when the server answers', async () => {
    const { user } = ladder()
    await user.click(stop('Own', 'Draw'))
    expect(lit()).toEqual(['Own:Draw:turn'])
    await user.click(stop('Own', 'Draw'))
    expect(lit()).toEqual([])
    await user.keyboard('{Shift>}')
    await user.click(stop('Own', 'Draw'))
    await user.keyboard('{/Shift}')
    expect(lit()).toEqual(['Own:Draw:lasting'])
    expect(stop('Own', 'Draw').querySelector('.lucide-pin')).not.toBeNull()
  })

  it('makes a one-turn stop lasting with shift, and clears a lasting one with a plain press', async () => {
    const { user } = ladder({ stops: [{ side: 'Opponent', step: 'End', keep: false }] })
    await user.keyboard('{Shift>}')
    await user.click(stop('Opponent', 'End'))
    await user.keyboard('{/Shift}')
    expect(lit()).toEqual(['Opponent:End:lasting'])
    await user.click(stop('Opponent', 'End'))
    expect(lit()).toEqual([])
  })

  it('keeps each side and step apart', async () => {
    const { user } = ladder()
    await user.click(stop('Own', 'Main1'))
    await user.click(stop('Opponent', 'Main1'))
    await user.click(stop('Own', 'End'))
    expect(lit()).toEqual(['Own:Main1:turn', 'Own:End:turn', 'Opponent:Main1:turn'])
    await user.click(stop('Opponent', 'Main1'))
    expect(lit()).toEqual(['Own:Main1:turn', 'Own:End:turn'])
  })
})

describe('the Keep toggle', () => {
  it('is off to start with, pressed when on, and describes itself', async () => {
    const { user } = ladder()
    expect(keepToggle()).toHaveAttribute('aria-pressed', 'false')
    expect(keepToggle()).toHaveAccessibleDescription(expect.stringContaining('stays every turn'))
    await user.click(keepToggle())
    expect(keepToggle()).toHaveAttribute('aria-pressed', 'true')
    await user.click(keepToggle())
    expect(keepToggle()).toHaveAttribute('aria-pressed', 'false')
  })

  it('makes the words of every button say that a press keeps the stop', async () => {
    const { user } = ladder()
    expect(stop('Own', 'Upkeep')).toHaveAccessibleName('Your upkeep stop: off. Press to stop here this turn')
    await user.click(keepToggle())
    expect(stop('Own', 'Upkeep')).toHaveAccessibleName('Your upkeep stop: off. Press to stop here every turn')
    expect(stop('Opponent', 'End')).toHaveAccessibleName(
      "Opponents' end step stop: off. Press to stop here on every opponent turn",
    )
    expect(stop('Own', 'Upkeep')).toHaveAccessibleDescription(
      expect.stringContaining('Shift and press to stop here this turn'),
    )
  })

  it('sets lasting stops on touch, where there is no shift', async () => {
    const { user } = ladder()
    await user.click(keepToggle())
    await user.click(stop('Own', 'Main1'))
    expect(lit()).toEqual(['Own:Main1:lasting'])
  })

  it('does not change the stops held, only what the next press does', async () => {
    const { user } = ladder({ stops: [{ side: 'Own', step: 'Main1', keep: false }] })
    await user.click(keepToggle())
    expect(lit()).toEqual(['Own:Main1:turn'])
  })
})

describe('the words of a button', () => {
  it('say, for your upkeep, what the player was promised', () => {
    ladder({ active: 0 })
    expect(screen.getByRole('button', { name: 'Your upkeep stop: off. Press to stop here this turn' })).toBe(
      stop('Own', 'Upkeep'),
    )
  })

  it('tell a screen reader about shift in the description, and show the step in the tooltip', () => {
    ladder()
    const button = stop('Own', 'Main1')
    expect(button).toHaveAccessibleDescription('Shift and press to stop here every turn.')
    expect(button).toHaveAttribute('title', 'Precombat main. Shift and press to stop here every turn.')
  })

  it('name the step in the tooltip when there is nothing more to say', () => {
    ladder({ stops: [{ side: 'Own', step: 'Main1', keep: true }] })
    expect(stop('Own', 'Main1')).toHaveAttribute('title', 'Precombat main')
  })

  it('have a different name for every one of the twenty buttons', () => {
    ladder()
    const names = [...document.querySelectorAll('[data-slot="stop"]')].map((button) =>
      button.getAttribute('aria-label'),
    )
    expect(new Set(names).size).toBe(20)
  })

  it('hide the pictures from a screen reader, which hears the words', () => {
    ladder({ stops: [{ side: 'Own', step: 'Main1', keep: true }] })
    for (const button of document.querySelectorAll('[data-slot="stop"]')) {
      for (const svg of button.querySelectorAll('svg')) expect(svg).toHaveAttribute('aria-hidden', 'true')
    }
  })
})

describe('the keyboard', () => {
  it('reaches the switch, then Keep, then the row on show with one Tab stop, then Full Control and its menu', async () => {
    const { user } = ladder()
    act(() => segment('Own').focus())
    await user.tab()
    expect(segment('Opponent')).toHaveFocus()
    await user.tab()
    expect(keepToggle()).toHaveFocus()
    // The opponent's turn is under way, so its row is entered at the step the game is in; the row out of sight has no stop.
    await user.tab()
    expect(stop('Opponent', 'Main1')).toHaveFocus()
    await user.tab()
    expect(screen.getByRole('button', { name: 'Full Control: off' })).toHaveFocus()
    await user.tab()
    expect(screen.getByRole('button', { name: 'Full Control options' })).toHaveFocus()
  })

  it('sits in the controls dock, after the hand and before the side tabs, in the order of reading', () => {
    ladder()
    const dock = must(document.querySelector<HTMLElement>('[data-region="controls"]'))
    expect(dock).toContainElement(segment('Own'))
    const hand = must(document.querySelector<HTMLElement>('[data-region="hand"]'))
    const side = must(document.querySelector<HTMLElement>('aside[data-side]'))
    expect(hand.compareDocumentPosition(dock) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(dock.compareDocumentPosition(side) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  })

  it('has just one button of a row in the tab order, the current step on that side’s turn', () => {
    ladder({ step: 'DeclareBlockers', active: 0 })
    const tabbable = (side: StopSide): string[] =>
      buttons(side)
        .filter((button) => button.tabIndex === 0)
        .map((button) => button.dataset['step'] ?? '')
    expect(tabbable('Own')).toEqual(['DeclareBlockers'])
    expect(tabbable('Opponent')).toEqual(['Upkeep'])
  })

  it('moves along a row with the arrow keys, Home and End, and stops at the ends', async () => {
    const { user } = ladder({ active: 0 })
    act(() => stop('Own', 'Upkeep').focus())
    await user.keyboard('{ArrowRight}')
    expect(stop('Own', 'Draw')).toHaveFocus()
    await user.keyboard('{ArrowDown}')
    expect(stop('Own', 'Main1')).toHaveFocus()
    await user.keyboard('{ArrowLeft}{ArrowLeft}{ArrowLeft}')
    expect(stop('Own', 'Upkeep')).toHaveFocus()
    await user.keyboard('{End}')
    expect(stop('Own', 'End')).toHaveFocus()
    await user.keyboard('{ArrowRight}')
    expect(stop('Own', 'End')).toHaveFocus()
    await user.keyboard('{Home}')
    expect(stop('Own', 'Upkeep')).toHaveFocus()
  })

  it('keeps the arrow keys inside the row they are pressed in', async () => {
    const { user } = ladder()
    act(() => stop('Opponent', 'End').focus())
    await user.keyboard('{End}{ArrowRight}')
    expect(stop('Opponent', 'End')).toHaveFocus()
    await user.keyboard('{Home}')
    expect(stop('Opponent', 'Upkeep')).toHaveFocus()
  })

  it('comes back to the button it left when Tab returns to the row', async () => {
    const { user } = ladder({ active: 0 })
    act(() => stop('Own', 'Upkeep').focus())
    await user.keyboard('{ArrowRight}{ArrowRight}')
    expect(stop('Own', 'Main1')).toHaveFocus()
    await user.tab({ shift: true })
    expect(keepToggle()).toHaveFocus()
    await user.tab()
    expect(stop('Own', 'Main1')).toHaveFocus()
    expect(stop('Own', 'Main1').tabIndex).toBe(0)
    expect(stop('Own', 'Upkeep').tabIndex).toBe(-1)
  })

  it.each([
    ['Enter', '{Enter}'],
    ['Space', ' '],
  ])('presses a stop with %s', async (_name, keys) => {
    const setStop = vi.fn((_side: StopSide, _step: Step, _on: boolean, _keep: boolean) => true)
    const { user } = ladder({ active: 0, session: { setStop } })
    act(() => stop('Own', 'Draw').focus())
    await user.keyboard(keys)
    expect(setStop).toHaveBeenCalledExactlyOnceWith('Own', 'Draw', true, false)
  })

  it.each([
    ['Enter', '{Enter}'],
    ['Space', ' '],
  ])('sets a lasting stop with shift and %s', async (_name, keys) => {
    const setStop = vi.fn((_side: StopSide, _step: Step, _on: boolean, _keep: boolean) => true)
    const { user } = ladder({ session: { setStop } })
    act(() => stop('Opponent', 'End').focus())
    await user.keyboard(`{Shift>}${keys}{/Shift}`)
    expect(setStop).toHaveBeenCalledExactlyOnceWith('Opponent', 'End', true, true)
  })

  it('does not hand Space and Enter to the primary button while a stop has focus', async () => {
    const answer = vi.fn(() => true)
    const setStop = vi.fn((_side: StopSide, _step: Step, _on: boolean, _keep: boolean) => true)
    const { user } = renderTable({ session: { answer, setStop } })
    act(() => stop('Opponent', 'Draw').focus())
    await user.keyboard(' {Enter}')
    expect(answer).not.toHaveBeenCalled()
    expect(setStop).toHaveBeenCalledTimes(2)
  })
})

describe('without a connection', () => {
  it.each(['connecting', 'reconnecting', 'stopped'] satisfies ConnectionStatus[])(
    'cannot be used while the socket is %s',
    async (status) => {
      const setStop = vi.fn((_side: StopSide, _step: Step, _on: boolean, _keep: boolean) => true)
      const { user } = ladder({ status, session: { setStop } })
      for (const button of document.querySelectorAll('[data-slot="stop"]')) {
        expect(button).toHaveAttribute('aria-disabled', 'true')
      }
      await user.click(stop('Own', 'Main1'))
      await user.click(stop('Opponent', 'End'))
      expect(setStop).not.toHaveBeenCalled()
    },
  )

  it('keeps the buttons in the tab order, so focus is not lost when the connection drops', () => {
    ladder({ status: 'reconnecting' })
    expect(stop('Own', 'Upkeep').tabIndex).toBe(0)
  })

  it('can be used while the socket is open', () => {
    ladder()
    for (const button of document.querySelectorAll('[data-slot="stop"]')) {
      expect(button).not.toHaveAttribute('aria-disabled')
    }
  })
})

describe('who sees it', () => {
  const lost = (state: PlayState): PlayState => {
    const view = structuredClone(must(state.view))
    const me = must(view.players.find((player) => player.seat === state.seat))
    me.lost = 'Conceded'
    return { ...state, view }
  }

  it.each([
    ['a seat that is out', lost],
    ['a game that is over', (state: PlayState): PlayState => ({ ...state, phase: 'over' })],
    ['a table with no view yet', (state: PlayState): PlayState => ({ ...state, view: null })],
  ])('is not drawn for %s', (_label, change) => {
    renderTable({ state: change })
    expect(document.querySelector('[data-slot="stop-ladder"]')).toBeNull()
    expect(screen.queryByRole('region', { name: 'Stops and Full Control' })).toBeNull()
  })
})

describe('the Mine or Theirs switch', () => {
  it.each([
    ['an opponent’s turn', { active: 1 }, 'Opponent'],
    ['your own turn', { active: 0 }, 'Own'],
    ['a table not yet in its turns', { stage: 'Mulligan' as const }, 'Own'],
  ] satisfies [string, Setup, StopSide][])('starts on the side of the turn: %s', (_name, setup, side) => {
    ladder(setup)
    expect(shownSide()).toEqual([side])
    expect(row(side)).not.toHaveAttribute('hidden')
    expect(row(OTHER[side])).toHaveAttribute('hidden')
  })

  it('shows the other row when pressed, and says which side is on show', async () => {
    const { user } = ladder({ active: 1 })
    await user.click(segment('Own'))
    expect(shownSide()).toEqual(['Own'])
    expect(segment('Own')).toHaveAttribute('aria-pressed', 'true')
    expect(segment('Opponent')).toHaveAttribute('aria-pressed', 'false')
    expect(screen.getByRole('group', { name: 'Stops on your turns' })).toBe(row('Own'))
    expect(row('Opponent')).toHaveAttribute('hidden')
  })

  it('holds a side picked by hand while the turn stays where it was, and follows the turn again once it changes hands', async () => {
    const { user, update } = ladder({ active: 1 })
    await user.click(segment('Own'))
    // Another step of the same turn: the pick stands.
    update((state) => ({
      view: state.view && { ...state.view, turn: { ...state.view.turn, step: 'Main2' } },
    }))
    expect(shownSide()).toEqual(['Own'])
    // The turn passes to the viewer: the row follows it.
    update((state) => ({ view: state.view && { ...state.view, turn: { ...state.view.turn, active: 0 } } }))
    expect(shownSide()).toEqual(['Own'])
    update((state) => ({ view: state.view && { ...state.view, turn: { ...state.view.turn, active: 1 } } }))
    expect(shownSide()).toEqual(['Opponent'])
  })

  it('counts the stops of the side out of sight on its segment, and the names count every stop', async () => {
    const stops = [
      { side: 'Own', step: 'Main1', keep: false },
      { side: 'Opponent', step: 'End', keep: true },
      { side: 'Opponent', step: 'Upkeep', keep: false },
    ] satisfies Stop[]
    const { user } = ladder({ stops, active: 0 })
    expect(segment('Own')).toHaveTextContent('Mine')
    expect(segment('Own')).not.toHaveTextContent('1')
    expect(segment('Opponent')).toHaveTextContent('Theirs2')
    expect(segment('Own')).toHaveAccessibleName('Mine, 1 set')
    expect(segment('Opponent')).toHaveAccessibleName('Theirs, 2 set')
    await user.click(segment('Opponent'))
    expect(segment('Opponent')).not.toHaveTextContent('2')
    expect(segment('Own')).toHaveTextContent('Mine1')
  })

  it('does not press a stop or ask the server for anything', async () => {
    const setStop = vi.fn((_side: StopSide, _step: Step, _on: boolean, _keep: boolean) => true)
    const { user } = ladder({ session: { setStop } })
    await user.click(segment('Own'))
    await user.click(segment('Opponent'))
    expect(setStop).not.toHaveBeenCalled()
  })

  it('keeps the Keep toggle and the row it shows across a switch', async () => {
    const setStop = vi.fn((_side: StopSide, _step: Step, _on: boolean, _keep: boolean) => true)
    const { user } = ladder({ active: 0, session: { setStop } })
    await user.click(keepToggle())
    await user.click(segment('Opponent'))
    await user.click(stop('Opponent', 'End'))
    expect(setStop).toHaveBeenCalledExactlyOnceWith('Opponent', 'End', true, true)
  })
})

describe('over a live session', () => {
  function mount(stops: Stop[] = []) {
    const h = startSession()
    h.session.connect()
    h.net.last.open()
    const fixture = buildFixture({ seats: 2 })
    h.deliver({ ...welcome, stops })
    act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
    return { h, ...renderLiveTable(h.session, fixtureLookup(fixture)) }
  }

  it('sends one SetStop frame for a press, and lights the stop when the server lists it', async () => {
    const { h, user } = mount()
    await user.click(stop('Own', 'Main1'))
    expect(h.sentOf('SetStop')).toEqual([
      { type: 'SetStop', side: 'Own', step: 'Main1', on: true, keep: false },
    ])
    expect(lit()).toEqual([])
    act(() => h.deliver({ type: 'Stops', stops: [{ side: 'Own', step: 'Main1', keep: false }] }))
    expect(lit()).toEqual(['Own:Main1:turn'])
  })

  it('sends shift as keep, and a second press as a clear', async () => {
    const { h, user } = mount()
    await user.keyboard('{Shift>}')
    await user.click(stop('Opponent', 'End'))
    await user.keyboard('{/Shift}')
    act(() => h.deliver({ type: 'Stops', stops: [{ side: 'Opponent', step: 'End', keep: true }] }))
    await user.click(stop('Opponent', 'End'))
    expect(h.sentOf('SetStop')).toEqual([
      { type: 'SetStop', side: 'Opponent', step: 'End', on: true, keep: true },
      { type: 'SetStop', side: 'Opponent', step: 'End', on: false, keep: true },
    ])
  })

  it('shows a one-turn stop going once the server says its step has passed, and a lasting one staying', () => {
    const { h } = mount([
      { side: 'Own', step: 'Upkeep', keep: false },
      { side: 'Own', step: 'End', keep: true },
    ])
    expect(lit()).toEqual(['Own:Upkeep:turn', 'Own:End:lasting'])
    act(() => h.deliver({ type: 'Stops', stops: [{ side: 'Own', step: 'End', keep: true }] }))
    expect(lit()).toEqual(['Own:End:lasting'])
  })

  it('shows the stops of a Welcome, and what the next Welcome says after a reconnect', () => {
    const { h } = mount([{ side: 'Own', step: 'Draw', keep: true }])
    expect(lit()).toEqual(['Own:Draw:lasting'])
    act(() => h.net.last.fail())
    expect(stop('Own', 'Draw')).toHaveAttribute('aria-disabled', 'true')
    expect(lit()).toEqual(['Own:Draw:lasting'])
    act(() => h.timers.advance(250))
    act(() => {
      h.open()
      h.deliver({ ...welcome, stops: [{ side: 'Opponent', step: 'Main2', keep: false }] })
    })
    expect(stop('Own', 'Draw')).not.toHaveAttribute('aria-disabled')
    expect(lit()).toEqual(['Opponent:Main2:turn'])
  })

  it('sends nothing while the connection is down', async () => {
    const { h, user } = mount()
    act(() => h.net.last.fail())
    await user.click(stop('Own', 'Main1'))
    expect(h.sentOf('SetStop')).toEqual([])
  })
})
