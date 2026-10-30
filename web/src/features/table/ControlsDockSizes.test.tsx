// Goal: at every window width the controls dock holds at most five controls, with the stop ladder and Full Control as one row from 1600 up and behind one Stops button below, the primary pass button last and 44 px high in the corner, its words naming what passing does for the prompt on screen (whole, with key hints, from a 1150 px main area; short without them below it), every control reachable by Tab in the order of reading, and Space, Enter and Control meaning what they always meant.
import { act, screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer, FullControlMode, Step, YieldUntil } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { primaryButton } from '@/test-utils/passControls'
import { renderTable } from '@/test-utils/tableHarness'

const ROW = ['stop-ladder', 'full-control', 'secondary-action', 'more-passing', 'primary-action']
const BUTTON = ['stops-button', 'secondary-action', 'more-passing', 'primary-action']
const PHONE = ['stops-button', 'more-passing', 'primary-action']

/** Window widths with the controls each shows, in the order of reading. 1599 is one pixel short of the row. */
const SIZES: ReadonlyArray<[number, number, string[]]> = [
  [1920, 1080, ROW],
  [1600, 1000, ROW],
  [1599, 1000, BUTTON],
  [1440, 900, BUTTON],
  [1280, 720, BUTTON],
  [1024, 768, BUTTON],
  [390, 844, PHONE],
]

interface Situation {
  width?: number
  height?: number
  /** Whether it is the viewer's turn (seat 0). */
  own?: boolean
  step?: Step
  /** Whether the fixture's three spells are on the stack. */
  stacked?: boolean
  /** Whether a Priority decision is on screen; the stops are there without one. */
  prompt?: boolean
  fullControl?: FullControlMode
}

/** A table whose viewer faces priority, or nothing, in a window, with spies for what the session is asked. */
function dockOf({
  width = 1600,
  height = 720,
  own = false,
  step = 'DeclareBlockers',
  stacked = true,
  prompt = true,
  fullControl = 'Off',
}: Situation = {}) {
  const answer = vi.fn((_answer: Answer) => true)
  const yieldUntil = vi.fn((_until: YieldUntil) => true)
  const setFullControl = vi.fn((_mode: FullControlMode) => true)
  const table = renderTable({
    options: { viewport: { width, height } },
    session: { answer, yieldUntil, setFullControl },
    state: (state) => {
      const view = clone(must(state.view))
      view.turn = { ...view.turn, step, active: own ? 0 : 1 }
      if (!stacked) view.stack = { count: 0, cards: [] }
      return {
        ...state,
        view,
        fullControl,
        status: 'open',
        pending: prompt
          ? {
              id: 4,
              decision: {
                player: 0,
                kind: {
                  Priority: {
                    playable_lands: [],
                    castable: [],
                    mana_abilities: [],
                    can_act: false,
                    can_pass: true,
                  },
                },
              },
            }
          : null,
      }
    },
  })
  return { ...table, answer, yieldUntil, setFullControl }
}

const dock = (): HTMLElement => must(document.querySelector<HTMLElement>('[data-region="controls"]'))
const slotOf = (element: HTMLElement): string => element.dataset['slot'] ?? ''
const visible = (element: HTMLElement): boolean => element.closest('[hidden], [aria-hidden="true"]') === null

/** The outermost group (a ladder, Full Control) around `button` inside the dock, or the button itself. */
function controlOf(button: HTMLElement): HTMLElement {
  let found = button
  for (let at = button.parentElement; at !== null && at !== dock(); at = at.parentElement) {
    if (at.getAttribute('role') === 'group') found = at
  }
  return found
}

/** The controls of the dock as a person counts them: each button, and each group of buttons that works as one, once. */
function controls(): HTMLElement[] {
  const found: HTMLElement[] = []
  for (const button of dock().querySelectorAll<HTMLElement>('button')) {
    const control = controlOf(button)
    if (visible(button) && !found.includes(control)) found.push(control)
  }
  return found
}

describe.each(SIZES)('the dock at %i x %i', (width, height, expected) => {
  it('holds these controls, in the order of reading, and never more than five', () => {
    dockOf({ width, height })
    const found = controls()
    expect(found.map(slotOf)).toEqual(expected)
    expect(found.length).toBeLessThanOrEqual(5)
  })

  it('ends with the primary button, the one in the corner, 44 px high or more', () => {
    dockOf({ width, height })
    const last = must(controls().at(-1))
    expect(last).toBe(primaryButton())
    expect(last).toHaveClass('h-11')
    expect(dock()).toContainElement(last)
  })

  it('has the ladder once in the page, in the dock as a row or behind the Stops button, never as both', () => {
    dockOf({ width, height })
    expect(document.querySelectorAll('[data-slot="stop-ladder"]')).toHaveLength(1)
    const row = expected.includes('stop-ladder')
    expect(dock().querySelector('[data-slot="stop-ladder"]') !== null).toBe(row)
    expect(dock().querySelector('[data-slot="stops-button"]') !== null).toBe(!row)
    expect(document.querySelectorAll('[data-slot="full-control"]')).toHaveLength(1)
  })
})

describe('the Stops button', () => {
  it('opens the same ladder and Full Control as the row has, saying how many stops are set', async () => {
    const { unmount } = dockOf({ width: 1600 })
    const names = (root: ParentNode): string[] =>
      [...root.querySelectorAll<HTMLElement>('button')].map(
        (button) => button.getAttribute('aria-label') ?? button.textContent ?? '',
      )
    const inRow = names(must(dock().querySelector('[data-slot="stops-bar"]')))
    unmount()

    const popover = dockOf({ width: 1024, height: 768 })
    const button = screen.getByRole('button', { name: 'Stops, none set' })
    expect(button).toHaveAttribute('aria-haspopup', 'dialog')
    expect(button).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByRole('dialog', { name: 'Stops and Full Control' })).toBeNull()
    await popover.user.click(button)
    const panel = screen.getByRole('dialog', { name: 'Stops and Full Control' })
    expect(button).toHaveAttribute('aria-expanded', 'true')
    expect(names(must(panel.querySelector('[data-slot="stops-bar"]')))).toEqual(inRow)
  })

  it('counts the stops and says when Full Control is on', () => {
    const { update } = dockOf({ width: 1024, height: 768 })
    update(() => ({
      stops: [
        { side: 'Own', step: 'Main1', keep: false },
        { side: 'Opponent', step: 'End', keep: true },
      ],
      fullControl: 'Held',
    }))
    expect(screen.getByRole('button', { name: 'Stops, 2 set, Full Control held' })).toHaveTextContent(
      'Stops2',
    )
  })
})

describe('the pass button', () => {
  it.each([
    ['a spell on the stack', { stacked: true }, 'Resolve Lightning Bolt', 'Resolve', true],
    [
      'your first main phase',
      { own: true, stacked: false, step: 'Main1' as const },
      'Next: Combat',
      'Next: Combat',
      true,
    ],
    [
      'your declare attackers',
      { own: true, stacked: false, step: 'DeclareAttackers' as const },
      'Next: Declare blockers',
      'Next: Blockers',
      true,
    ],
    [
      'the end step of your turn',
      { own: true, stacked: false, step: 'End' as const },
      'End turn',
      'End turn',
      false,
    ],
    ['an opponent’s empty step', { stacked: false, step: 'Upkeep' as const }, 'Continue', 'Continue', true],
  ])('says what passing does in %s, at every size', (_name, situation, whole, short, endTurn) => {
    for (const [width, height, words] of [
      [1920, 1080, whole],
      [1024, 768, short],
      [390, 844, short],
    ] as const) {
      const { unmount } = dockOf({ ...situation, width, height })
      expect(primaryButton()).toHaveTextContent(words)
      // "End turn" beside the button only where the button does not already say it; a phone keeps it in the menu.
      expect(dock().querySelector('[data-slot="secondary-action"]') !== null).toBe(endTurn && width > 640)
      unmount()
    }
  })

  it.each([
    [1920, 1080, true],
    [1600, 1000, true],
    [1481, 900, true],
    [1480, 900, false],
    [1440, 900, false],
    [1280, 720, false],
    [1024, 768, false],
    [390, 844, false],
  ])('draws the key hints at %i x %i: %s', (width, height, hints) => {
    dockOf({ own: true, stacked: false, step: 'Main1', width, height })
    expect(dock().querySelectorAll('[data-slot="key-cap"]').length > 0).toBe(hints)
    // The keys are still told: by a tooltip and to a screen reader, whatever the window.
    expect(primaryButton()).toHaveAttribute('aria-keyshortcuts', 'Space')
    expect(primaryButton()).toHaveAttribute('title', 'Space')
  })

  it('leaves the spell’s name to the stack strip in a dock without hints, and says it to a screen reader', () => {
    dockOf({ width: 1280, height: 720 })
    expect(primaryButton()).toHaveTextContent(/^Resolve$/)
    expect(primaryButton()).toHaveAccessibleDescription(/Lightning Bolt resolves/)
  })

  it('is described as passing priority, though it never says so in words', () => {
    dockOf({ own: true, stacked: false, step: 'Main1' })
    expect(primaryButton()).toHaveAccessibleDescription('Passes priority.')
    expect(primaryButton()).not.toHaveTextContent(/pass priority/i)
  })

  it('is not there without a Priority decision, which leaves the stops alone in the dock', () => {
    dockOf({ prompt: false })
    expect(controls().map(slotOf)).toEqual(['stop-ladder', 'full-control'])
    expect(document.querySelector('[data-slot="primary-action"]')).toBeNull()
  })

  it('leaves the Stops button alone in the dock of a phone without a decision', () => {
    dockOf({ prompt: false, width: 390, height: 844 })
    expect(controls().map(slotOf)).toEqual(['stops-button'])
  })

  it('keeps "End turn" in the menu on a phone, where it asks for the same yield as the button does elsewhere', async () => {
    const { user, yieldUntil } = dockOf({ width: 390, height: 844, own: true, stacked: false, step: 'Main1' })
    expect(dock().querySelector('[data-slot="secondary-action"]')).toBeNull()
    await user.click(screen.getByRole('button', { name: 'More ways to pass' }))
    const items = (await screen.findAllByRole('menuitem')).map((item) => item.textContent)
    expect(items).toEqual(['End turn', 'Pass until my next turn'])
    await user.click(await screen.findByRole('menuitem', { name: 'End turn' }))
    expect(yieldUntil).toHaveBeenCalledExactlyOnceWith('EndOfTurn')
  })
})

describe('the keyboard', () => {
  /** The controls Tab visits from `start` until focus leaves the dock, named by slot or label. */
  async function walk(user: ReturnType<typeof dockOf>['user'], start: HTMLElement): Promise<string[]> {
    act(() => start.focus())
    const seen = [label(start)]
    for (let step = 0; step < 30; step += 1) {
      await user.tab()
      const at = document.activeElement
      if (!(at instanceof HTMLElement) || !dock().contains(at)) break
      seen.push(label(at))
    }
    return seen
  }
  const label = (element: HTMLElement): string =>
    element.dataset['slot'] === 'button' || element.dataset['slot'] === 'stop'
      ? (element.getAttribute('aria-label') ?? '')
      : (element.dataset['slot'] ?? '')

  it('reaches every control of the row in the order of reading, the step row as one stop', async () => {
    const { user } = dockOf({ width: 1600, own: true, stacked: false, step: 'Main1' })
    const first = must(dock().querySelector<HTMLElement>('[data-slot="stop-side"] button'))
    expect(await walk(user, first)).toEqual([
      'Mine, none set',
      'Theirs, none set',
      'keep-stops',
      'Your precombat main stop: off. Press to stop here this turn',
      'full-control-toggle',
      'full-control-menu',
      'secondary-action',
      'more-passing',
      'primary-action',
    ])
  })

  it('reaches the Stops button, then the pass controls, below 1600, and the popover from the button alone', async () => {
    const { user } = dockOf({ width: 1024, height: 768, own: true, stacked: false, step: 'Main1' })
    const stops = screen.getByRole('button', { name: /^Stops/ })
    expect(await walk(user, stops)).toEqual([
      'stops-button',
      'secondary-action',
      'more-passing',
      'primary-action',
    ])

    act(() => stops.focus())
    await user.keyboard('{Enter}')
    const panel = screen.getByRole('dialog', { name: 'Stops and Full Control' })
    await waitFor(() => expect(panel).toContainElement(document.activeElement as HTMLElement))
    await user.keyboard('{Escape}')
    await waitFor(() => expect(stops).toHaveFocus())
    expect(screen.queryByRole('dialog', { name: 'Stops and Full Control' })).toBeNull()
  })

  it.each([
    [1600, 1000],
    [390, 844],
  ])('keeps Space, Enter and Control as they were at %i px, with focus on nothing', async (width, height) => {
    const { user, answer, yieldUntil, setFullControl } = dockOf({
      width,
      height,
      own: true,
      stacked: false,
      step: 'Main1',
    })
    act(() => (document.activeElement as HTMLElement | null)?.blur())
    await user.keyboard(' ')
    expect(answer).toHaveBeenCalledExactlyOnceWith({ Priority: 'Pass' })
    await user.keyboard('{Enter}')
    expect(yieldUntil).toHaveBeenCalledExactlyOnceWith('EndOfTurn')
    await user.keyboard('{Control}')
    expect(setFullControl).toHaveBeenCalledExactlyOnceWith('NextWindow')
  })

  it('leaves the keys to a stop that has focus, as to any button', async () => {
    const { user, answer } = dockOf({ width: 1600, own: true, stacked: false, step: 'Main1' })
    act(() => must(within(dock()).getAllByRole('button', { name: /stop: / })[0]).focus())
    await user.keyboard(' ')
    expect(answer).not.toHaveBeenCalled()
  })
})
