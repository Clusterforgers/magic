// Goal: the step ladder is the stop ladder: each mark of a step that gives priority is a button that toggles the stop on the side on show (Mine or Theirs, following the turn until picked by hand), a held stop is an outline and a dot (a pin when it lasts) apart from the filled current step, the Keep toggle and shift make a press lasting, one tab stop with the arrow keys, nothing is predicted before the server answers, and the ladder is drawn once: in the top bar from 1024 px, in the stops popover below.
import { act, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ConnectionStatus } from '@/net/socket'
import type { Session } from '@/play/session'
import { STOP_STEPS } from '@/play/stops'
import type { StopStep } from '@/play/stops'
import type { PlayState } from '@/play/store'
import type { Step, Stop, StopSide } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { welcome } from '@/test-utils/socketSetup'
import { renderLiveTable, renderTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'
import type { Viewport } from '../layout'
import { stopName } from './stopModel'
import type { StopState } from './stopModel'

const SIDES: StopSide[] = ['Own', 'Opponent']
const TEN: StopStep[] = STOP_STEPS.filter((step) => step !== 'FirstStrikeDamage')

interface Setup {
  stops?: Stop[]
  step?: Step
  /** The seat whose turn it is; the viewer is seat 0, so 0 shows Mine and 1 shows Theirs. */
  active?: number
  status?: ConnectionStatus
  session?: Partial<Session>
  viewport?: Viewport
}

/** A table whose viewer is seat 0, in `step` of `active`'s turn, holding `stops`. */
function ladder({ stops = [], step = 'Main1', active = 1, status = 'open', session, viewport }: Setup = {}) {
  return renderTable({
    session,
    ...(viewport === undefined ? {} : { options: { viewport } }),
    state: (state) => {
      const view = clone(must(state.view))
      view.turn = { ...view.turn, step, active }
      return { ...state, view, stops, status }
    },
  })
}

const asked = () => vi.fn((_side: StopSide, _step: Step, _on: boolean, _keep: boolean) => true)
const stop = (step: Step): HTMLElement =>
  must(document.querySelector<HTMLElement>(`[data-slot="stop"][data-step="${step}"]`))
const buttons = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('[data-slot="stop"]')]
const lit = (): string[] =>
  buttons()
    .filter((button) => button.getAttribute('aria-pressed') === 'true')
    .map((button) => `${button.dataset['side']}:${button.dataset['step']}:${button.dataset['keep']}`)
const keepToggle = (): HTMLElement => screen.getByRole('button', { name: 'Keep stops' })
const segment = (side: StopSide): HTMLElement =>
  must(document.querySelector<HTMLElement>(`[data-slot="stop-side"] [data-side="${side}"]`))
const held = (side: StopSide, step: Step, state: StopState): Stop[] =>
  state === 'off' ? [] : [{ side, step, keep: state === 'lasting' }]

describe('the ladder', () => {
  it('has a button for each of the ten steps in turn order, in one group named for the side on show', () => {
    ladder()
    expect(buttons().map((button) => button.dataset['step'])).toEqual(TEN)
    const group = screen.getByRole('group', {
      name: "Stops on opponents' turns, an opponent's turn is under way",
    })
    expect(group).toHaveAttribute('data-slot', 'step-ladder')
    expect(within(group).getAllByRole('button')).toEqual(buttons())
    expect(document.querySelectorAll('[data-slot="step-ladder"]')).toHaveLength(1)
  })

  it('is one ladder, not a mark row and a button row: no second row of step buttons', () => {
    ladder()
    expect(document.querySelectorAll('[data-slot="stop-row"]')).toHaveLength(0)
    expect(document.querySelectorAll('[data-slot="stop-ladder"] [data-slot="stop"]')).toHaveLength(0)
    expect(document.querySelectorAll('[data-region="bar"] [data-slot="stop"]')).toHaveLength(10)
  })

  it('lights nothing while the server holds no stop, and draws no untap and no cleanup', () => {
    ladder()
    expect(lit()).toEqual([])
    expect(document.querySelector('[data-step="Untap"], [data-step="Cleanup"]')).toBeNull()
  })

  it('adds first-strike damage in the step that has it, and while a stop is held there', () => {
    ladder({ step: 'FirstStrikeDamage' })
    expect(buttons()).toHaveLength(11)
    expect(stop('FirstStrikeDamage')).toHaveAccessibleName(
      stopName('Opponent', 'FirstStrikeDamage', 'off', false),
    )
  })

  it.each(SIDES)('keeps first-strike damage while a stop is held there on the %s side', (side) => {
    ladder({ active: side === 'Own' ? 0 : 1, stops: [{ side, step: 'FirstStrikeDamage', keep: true }] })
    expect(buttons()).toHaveLength(11)
    expect(lit()).toEqual([`${side}:FirstStrikeDamage:lasting`])
  })

  it('marks the current step apart from a held stop: the fill is the step, the outline and dot are the stop', () => {
    ladder({ step: 'Main1', stops: held('Opponent', 'End', 'turn') })
    expect(stop('Main1')).toHaveAttribute('aria-current', 'step')
    expect(stop('Main1')).toHaveAttribute('data-current')
    expect(stop('Main1')).toHaveAttribute('aria-pressed', 'false')
    expect(stop('End')).not.toHaveAttribute('aria-current')
    expect(stop('End')).toHaveAttribute('aria-pressed', 'true')
    expect(stop('End').parentElement?.querySelector('.rounded-full')).not.toBeNull()
    expect(stop('Main1').parentElement?.querySelector('.rounded-full, .lucide-pin')).toBeNull()
  })

  it('shows a lasting stop with a pin and a one-turn stop with a dot', () => {
    ladder({
      stops: [
        { side: 'Opponent', step: 'Draw', keep: true },
        { side: 'Opponent', step: 'End', keep: false },
      ],
    })
    expect(stop('Draw').parentElement?.querySelector('.lucide-pin')).not.toBeNull()
    expect(stop('End').parentElement?.querySelector('.lucide-pin')).toBeNull()
    expect(lit()).toEqual(['Opponent:Draw:lasting', 'Opponent:End:turn'])
  })
})

describe('the side on show', () => {
  it('follows the turn: Mine on your turn, Theirs on another seat’s, and the switch names the side and its count', () => {
    ladder({ active: 0, stops: [{ side: 'Own', step: 'Main1', keep: false }] })
    expect(segment('Own')).toHaveAttribute('aria-pressed', 'true')
    expect(segment('Own')).toHaveAccessibleName('Mine, 1 set')
    expect(segment('Opponent')).toHaveAccessibleName('Theirs, none set')
    expect(
      screen.getByRole('group', { name: 'Stops on your turns, your turn is under way' }),
    ).toBeInTheDocument()
    expect(lit()).toEqual(['Own:Main1:turn'])
  })

  it('shows the other side after a pick, which holds, and sets stops on that side', async () => {
    const setStop = asked()
    const { user } = ladder({ stops: held('Own', 'Draw', 'turn'), session: { setStop } })
    expect(lit()).toEqual([])
    await user.click(segment('Own'))
    expect(segment('Own')).toHaveAttribute('aria-pressed', 'true')
    expect(lit()).toEqual(['Own:Draw:turn'])
    expect(screen.getByRole('group', { name: 'Stops on your turns' })).toBeInTheDocument()
    expect(setStop).not.toHaveBeenCalled()
    await user.click(stop('Upkeep'))
    expect(setStop).toHaveBeenCalledExactlyOnceWith('Own', 'Upkeep', true, false)
  })

  it('goes back to the turn once the turn changes hands', async () => {
    const { user, store } = ladder()
    await user.click(segment('Own'))
    expect(segment('Own')).toHaveAttribute('aria-pressed', 'true')
    act(() => {
      const view = clone(must(store.getState().view))
      view.turn = { ...view.turn, active: 0 }
      store.setState({ view })
    })
    expect(segment('Own')).toHaveAttribute('aria-pressed', 'true')
    act(() => {
      const view = clone(must(store.getState().view))
      view.turn = { ...view.turn, active: 1 }
      store.setState({ view })
    })
    expect(segment('Opponent')).toHaveAttribute('aria-pressed', 'true')
  })
})

describe('pressing a step', () => {
  interface Press {
    state: StopState
    shift: boolean
    keep: boolean
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
      const setStop = asked()
      const { user } = ladder({
        active: side === 'Own' ? 0 : 1,
        stops: held(side, 'Main2', state),
        session: { setStop },
      })
      if (keep) await user.click(keepToggle())
      if (shift) await user.keyboard('{Shift>}')
      await user.click(stop('Main2'))
      if (shift) await user.keyboard('{/Shift}')
      expect(setStop).toHaveBeenCalledExactlyOnceWith(side, 'Main2', ...asks)
    },
  )

  it.each(TEN)('asks for the step it is on: %s', async (step) => {
    const setStop = asked()
    const { user } = ladder({ session: { setStop } })
    await user.click(stop(step))
    expect(setStop).toHaveBeenCalledExactlyOnceWith('Opponent', step, true, false)
  })

  it('shows nothing new until the server says so, then follows its list in either direction', async () => {
    const setStop = asked()
    const { user, store } = ladder({ session: { setStop } })
    await user.click(stop('Draw'))
    expect(lit()).toEqual([])
    act(() => store.setState({ stops: held('Opponent', 'Draw', 'lasting') }))
    expect(lit()).toEqual(['Opponent:Draw:lasting'])
    act(() => store.setState({ stops: [] }))
    expect(lit()).toEqual([])
  })

  it('reads the stops the server holds at the moment of the press, not those of the last render', async () => {
    const setStop = asked()
    const { user, store } = ladder({ session: { setStop } })
    const button = stop('Draw')
    store.setState({ stops: held('Opponent', 'Draw', 'turn') })
    await user.click(button)
    expect(setStop).toHaveBeenCalledExactlyOnceWith('Opponent', 'Draw', false, false)
  })
})

describe('the Keep toggle', () => {
  it('is off to start with, pressed when on, and describes itself', async () => {
    const { user } = ladder()
    expect(keepToggle()).toHaveAttribute('aria-pressed', 'false')
    expect(keepToggle()).toHaveAccessibleDescription(expect.stringContaining('stays every turn'))
    await user.click(keepToggle())
    expect(keepToggle()).toHaveAttribute('aria-pressed', 'true')
  })

  it('makes the words of every step say that a press keeps the stop, with shift for a one-turn stop', async () => {
    const { user } = ladder()
    expect(stop('Upkeep')).toHaveAccessibleName(
      "Opponents' upkeep stop: off. Press to stop here on one opponent turn",
    )
    await user.click(keepToggle())
    expect(stop('Upkeep')).toHaveAccessibleName(
      "Opponents' upkeep stop: off. Press to stop here on every opponent turn",
    )
    expect(stop('Upkeep')).toHaveAccessibleDescription(
      expect.stringContaining('Shift and press to stop here on one opponent turn'),
    )
  })

  it('sets lasting stops on touch, where there is no shift, and keeps the stops held', async () => {
    const { user } = ladder({ stops: held('Opponent', 'End', 'turn') })
    await user.click(keepToggle())
    expect(lit()).toEqual(['Opponent:End:turn'])
    await user.click(stop('Main1'))
    expect(lit()).toEqual(['Opponent:Main1:lasting', 'Opponent:End:turn'])
  })
})

describe('the keyboard', () => {
  it('reaches the switch, then Keep, then the ladder with one Tab stop (the current step), then Full Control', async () => {
    const { user } = ladder()
    act(() => segment('Own').focus())
    await user.tab()
    expect(segment('Opponent')).toHaveFocus()
    await user.tab()
    expect(keepToggle()).toHaveFocus()
    await user.tab()
    expect(screen.getByRole('button', { name: 'Full Control: off' })).toHaveFocus()
    expect(
      buttons()
        .filter((button) => button.tabIndex === 0)
        .map((b) => b.dataset['step']),
    ).toEqual(['Main1'])
  })

  it('moves along the ladder with the arrow keys, Home and End across the phases, and stops at the ends', async () => {
    const { user } = ladder()
    act(() => stop('Upkeep').focus())
    await user.keyboard('{ArrowRight}')
    expect(stop('Draw')).toHaveFocus()
    await user.keyboard('{ArrowRight}')
    expect(stop('Main1')).toHaveFocus()
    await user.keyboard('{ArrowLeft}{ArrowLeft}{ArrowLeft}')
    expect(stop('Upkeep')).toHaveFocus()
    await user.keyboard('{End}{ArrowRight}')
    expect(stop('End')).toHaveFocus()
    await user.keyboard('{Home}')
    expect(stop('Upkeep')).toHaveFocus()
  })

  it('comes back to the button it left when Tab returns to the ladder', async () => {
    const { user } = ladder()
    act(() => stop('Upkeep').focus())
    await user.keyboard('{ArrowRight}{ArrowRight}')
    await user.tab({ shift: true })
    await user.tab()
    expect(stop('Main1')).toHaveFocus()
    expect(stop('Upkeep').tabIndex).toBe(-1)
  })

  it.each([
    ['Enter', '{Enter}'],
    ['Space', ' '],
  ])('presses a step with %s, and with shift sets a lasting stop', async (_name, keys) => {
    const setStop = asked()
    const { user } = ladder({ session: { setStop } })
    act(() => stop('Draw').focus())
    await user.keyboard(keys)
    await user.keyboard(`{Shift>}${keys}{/Shift}`)
    expect(setStop).toHaveBeenNthCalledWith(1, 'Opponent', 'Draw', true, false)
    expect(setStop).toHaveBeenNthCalledWith(2, 'Opponent', 'Draw', true, true)
  })

  it('does not hand Space and Enter to the primary button while a step has focus', async () => {
    const answer = vi.fn(() => true)
    const setStop = asked()
    const { user } = renderTable({ session: { answer, setStop } })
    act(() => stop('Draw').focus())
    await user.keyboard(' {Enter}')
    expect(answer).not.toHaveBeenCalled()
    expect(setStop).toHaveBeenCalledTimes(2)
  })
})

describe('without a connection', () => {
  it.each(['connecting', 'reconnecting', 'stopped'] satisfies ConnectionStatus[])(
    'cannot be used while the socket is %s, and keeps its tab stop',
    async (status) => {
      const setStop = asked()
      const { user } = ladder({ status, session: { setStop } })
      for (const button of buttons()) expect(button).toHaveAttribute('aria-disabled', 'true')
      await user.click(stop('Main1'))
      expect(setStop).not.toHaveBeenCalled()
      expect(stop('Main1').tabIndex).toBe(0)
    },
  )
})

describe('who sees the stops', () => {
  const lost = (state: PlayState): PlayState => {
    const view = structuredClone(must(state.view))
    const me = must(view.players.find((player) => player.seat === state.seat))
    me.lost = 'Conceded'
    return { ...state, view }
  }

  it.each([
    ['a seat that is out', lost],
    ['a game that is over', (state: PlayState): PlayState => ({ ...state, phase: 'over' })],
  ])('has no stop buttons for %s, only the picture of the turn', (_label, change) => {
    renderTable({ state: change })
    expect(buttons()).toHaveLength(0)
    expect(document.querySelector('[data-slot="stop-ladder"]')).toBeNull()
    expect(document.querySelector('[data-slot="step-ladder"]')).toHaveAttribute('aria-hidden', 'true')
  })
})

describe('where the ladder is drawn', () => {
  it.each([
    ['1440', { width: 1440, height: 900 }],
    ['1366', { width: 1366, height: 768 }],
    ['1280', { width: 1280, height: 720 }],
    ['1024', { width: 1024, height: 768 }],
  ])('is in the top bar at %s px, once', (_name, viewport) => {
    ladder({ viewport })
    expect(document.querySelectorAll('[data-slot="step-ladder"]')).toHaveLength(1)
    expect(document.querySelector('[data-region="bar"] [data-slot="step-ladder"]')).not.toBeNull()
    expect(buttons()).toHaveLength(10)
  })

  it.each([
    ['768', { width: 768, height: 1024 }],
    ['390', { width: 390, height: 844 }],
  ])('is in the stops popover, once, at %s px, with the same buttons', async (_name, viewport) => {
    const setStop = asked()
    const { user } = ladder({ viewport, session: { setStop } })
    expect(document.querySelector('[data-region="bar"] [data-slot="step-ladder"]')).toBeNull()
    await user.click(screen.getByRole('button', { name: /^Stops/ }))
    const popover = await screen.findByRole('dialog', { name: 'Stops and Full Control' })
    expect(document.querySelectorAll('[data-slot="step-ladder"]')).toHaveLength(1)
    expect(within(popover).getByRole('group', { name: /^Stops on/ })).toBeInTheDocument()
    expect(within(popover).getAllByRole('button', { name: /stop: / })).toHaveLength(10)
    await user.click(stop('Draw'))
    expect(setStop).toHaveBeenCalledExactlyOnceWith('Opponent', 'Draw', true, false)
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
    await user.click(stop('Main1'))
    const side = must(stop('Main1').dataset['side']) as StopSide
    expect(h.sentOf('SetStop')).toEqual([{ type: 'SetStop', side, step: 'Main1', on: true, keep: false }])
    expect(lit()).toEqual([])
    act(() => h.deliver({ type: 'Stops', stops: [{ side, step: 'Main1', keep: false }] }))
    expect(lit()).toEqual([`${side}:Main1:turn`])
    await user.click(stop('Main1'))
    expect(h.sentOf('SetStop')[1]).toEqual({ type: 'SetStop', side, step: 'Main1', on: false, keep: false })
  })

  it('shows the stops of a Welcome, and what the next Welcome says after a reconnect', () => {
    const { h } = mount([
      { side: 'Own', step: 'Draw', keep: true },
      { side: 'Opponent', step: 'Draw', keep: true },
    ])
    const first = lit()
    expect(first).toHaveLength(1)
    act(() => h.net.last.fail())
    expect(stop('Draw')).toHaveAttribute('aria-disabled', 'true')
    expect(lit()).toEqual(first)
    act(() => h.timers.advance(250))
    act(() => {
      h.open()
      h.deliver({ ...welcome, stops: [] })
    })
    expect(stop('Draw')).not.toHaveAttribute('aria-disabled')
    expect(lit()).toEqual([])
  })

  it('sends nothing while the connection is down', async () => {
    const { h, user } = mount()
    act(() => h.net.last.fail())
    await user.click(stop('Main1'))
    expect(h.sentOf('SetStop')).toEqual([])
  })
})
