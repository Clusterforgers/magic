// Goal: on the whole table, Space passes once, Enter ends the turn (or passes once under Full Control and in the last window of the turn), a tap of Control asks for the next window and Control with Shift for Full Control held, each only when nothing has the keyboard and no modifier spoils it, with Full Control off, next window or held, and with a Priority decision, another decision or none pending.
import { act, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer, DecisionKind, FullControlMode, Step, YieldUntil } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { KINDS } from '@/test-utils/decisions'
import { renderTable } from '@/test-utils/tableHarness'
import { GameTable } from './GameTable'

type Pending = 'priority' | 'blockers' | 'none'
/** Where the table is: your own first main phase (the primary button says "Next: Combat") or your own end step. */
type Moment = 'main' | 'end'
type Focus = 'nothing' | 'field' | 'button'

interface Case {
  mode?: FullControlMode
  pending?: Pending
  moment?: Moment
  focus?: Focus
}

const KIND: Record<Exclude<Pending, 'none'>, DecisionKind> = {
  priority: KINDS.Priority,
  blockers: KINDS.DeclareBlockers,
}
const STEP: Record<Moment, Step> = { main: 'Main1', end: 'End' }

/** The whole table, with what the session is asked written down in order. */
function mount({ mode = 'Off', pending = 'priority', moment = 'main', focus = 'nothing' }: Case = {}) {
  const asked: string[] = []
  const answer = vi.fn((given: Answer) => {
    const text = JSON.stringify(given)
    asked.push(text === '{"Priority":"Pass"}' ? 'pass' : `answer ${text}`)
    return true
  })
  const yieldUntil = vi.fn((until: YieldUntil) => {
    asked.push(`yield ${until}`)
    return true
  })
  const setFullControl = vi.fn((next: FullControlMode) => {
    asked.push(`full control ${next}`)
    return true
  })
  const table = renderTable({
    session: { answer, yieldUntil, setFullControl },
    ui: (
      <>
        <GameTable />
        <input aria-label="A text field of the page" />
      </>
    ),
    state: (state) => {
      const view = clone(must(state.view))
      view.turn = { ...view.turn, step: STEP[moment], active: 0 }
      view.stack = { count: 0, cards: [] }
      const kind = pending === 'none' ? undefined : KIND[pending]
      return {
        ...state,
        view,
        fullControl: mode,
        pending: kind === undefined ? null : { id: 4, decision: { player: 0, kind } },
      }
    },
  })
  if (focus === 'field') act(() => screen.getByRole('textbox', { name: 'A text field of the page' }).focus())
  if (focus === 'button') act(() => screen.getByRole('button', { name: 'Keep stops' }).focus())
  return { ...table, asked }
}

const SPACE = ' '
const ENTER = '{Enter}'
const CTRL = '{Control}'
const CTRL_SHIFT = '{Control>}{Shift}{/Control}'

describe('Space', () => {
  it.each([
    ['Off', 'main'],
    ['Off', 'end'],
    ['NextWindow', 'main'],
    ['Held', 'main'],
    ['Held', 'end'],
  ] satisfies [FullControlMode, Moment][])('passes once with Full Control %s at %s', async (mode, moment) => {
    const { user, asked } = mount({ mode, moment })
    await user.keyboard(SPACE)
    expect(asked).toEqual(['pass'])
  })

  it.each([
    ['another decision is pending', { pending: 'blockers' }],
    ['no decision is pending', { pending: 'none' }],
  ] satisfies [string, Case][])('does nothing while %s', async (_what, setup) => {
    const { user, asked } = mount(setup)
    await user.keyboard(SPACE)
    expect(asked).toEqual([])
  })

  it('passes once for a key held down', async () => {
    const { user, asked } = mount()
    await user.keyboard('{ >5/}')
    expect(asked).toEqual(['pass'])
  })
})

describe('Enter', () => {
  it.each([
    ['Off', 'main'],
    ['Off', 'end'],
  ] satisfies [FullControlMode, Moment][])(
    'ends the turn with Full Control %s at the %s of the turn, as far as it has a button for it',
    async (mode, moment) => {
      const { user, asked } = mount({ mode, moment })
      await user.keyboard(ENTER)
      // In the last window of the turn the primary button says "End turn" and no second one is drawn: Enter presses it.
      expect(asked).toEqual([moment === 'end' ? 'pass' : 'yield EndOfTurn'])
    },
  )

  it.each([
    ['NextWindow', 'main'],
    ['NextWindow', 'end'],
    ['Held', 'main'],
    ['Held', 'end'],
  ] satisfies [FullControlMode, Moment][])(
    'passes once, like Space, with Full Control %s at %s',
    async (mode, moment) => {
      const { user, asked } = mount({ mode, moment })
      await user.keyboard(ENTER)
      expect(asked).toEqual(['pass'])
    },
  )

  it.each([
    ['another decision is pending', { pending: 'blockers' }],
    ['no decision is pending', { pending: 'none' }],
  ] satisfies [string, Case][])('does nothing while %s', async (_what, setup) => {
    const { user, asked } = mount(setup)
    await user.keyboard(ENTER)
    expect(asked).toEqual([])
  })

  it('asks once for a key held down', async () => {
    const { user, asked } = mount()
    await user.keyboard('{Enter>5/}')
    expect(asked).toEqual(['yield EndOfTurn'])
  })
})

describe('Control', () => {
  it.each([
    ['Off', 'full control NextWindow'],
    ['NextWindow', 'full control Off'],
  ] satisfies [FullControlMode, string][])('tapped alone, in %s, asks: %s', async (mode, asks) => {
    for (const pending of ['priority', 'blockers', 'none'] satisfies Pending[]) {
      const { user, asked, unmount } = mount({ mode, pending })
      await user.keyboard(CTRL)
      expect(asked, `with ${pending} pending`).toEqual([asks])
      unmount()
    }
  })

  it('tapped alone while Full Control is held asks for nothing, since Control and Shift let it go', async () => {
    const { user, asked } = mount({ mode: 'Held' })
    await user.keyboard(CTRL)
    expect(asked).toEqual([])
  })

  it.each([
    ['Off', 'full control Held'],
    ['NextWindow', 'full control Held'],
    ['Held', 'full control Off'],
  ] satisfies [FullControlMode, string][])(
    'with Shift, in %s, asks: %s, whatever is pending',
    async (mode, asks) => {
      for (const pending of ['priority', 'blockers', 'none'] satisfies Pending[]) {
        const { user, asked, unmount } = mount({ mode, pending })
        await user.keyboard(CTRL_SHIFT)
        expect(asked, `with ${pending} pending`).toEqual([asks])
        unmount()
      }
    },
  )

  it('does not pass or end the turn, so Full Control can be asked for with a prompt open', async () => {
    const { user, asked } = mount()
    await user.keyboard(`${CTRL}${CTRL_SHIFT}`)
    expect(asked).toEqual(['full control NextWindow', 'full control Held'])
  })
})

describe('with a modifier held', () => {
  it.each([
    ['Shift+Space', '{Shift>} {/Shift}'],
    ['Shift+Enter', '{Shift>}{Enter}{/Shift}'],
    ['Control+Space', '{Control>} {/Control}'],
    ['Control+Enter', '{Control>}{Enter}{/Control}'],
    ['Alt+Space', '{Alt>} {/Alt}'],
    ['Alt+Enter', '{Alt>}{Enter}{/Alt}'],
    ['Meta+Space', '{Meta>} {/Meta}'],
    ['Meta+Enter', '{Meta>}{Enter}{/Meta}'],
    ['Control+Shift+Space', '{Control>}{Shift>} {/Shift}{/Control}'],
    ['Control+Shift+Enter', '{Control>}{Shift>}{Enter}{/Shift}{/Control}'],
    ['Control+A', '{Control>}a{/Control}'],
    ['Control+Shift+I', '{Control>}{Shift>}i{/Shift}{/Control}'],
    ['Alt+Control', '{Alt>}{Control}{/Alt}'],
    ['Meta+Control', '{Meta>}{Control}{/Meta}'],
    ['Shift alone', '{Shift}'],
    ['Alt alone', '{Alt}'],
    ['Meta alone', '{Meta}'],
  ])('%s does nothing at all', async (_name, keys) => {
    const { user, asked } = mount()
    await user.keyboard(keys)
    expect(asked).toEqual([])
  })
})

describe('what has the keyboard', () => {
  it.each([
    ['Space', SPACE],
    ['Enter', ENTER],
    ['Control', CTRL],
    ['Control with Shift', CTRL_SHIFT],
  ])('leaves %s to a text field, so a line can be typed', async (_name, keys) => {
    const { user, asked } = mount({ focus: 'field' })
    await user.keyboard(keys)
    expect(asked).toEqual([])
  })

  it.each([
    ['Control', CTRL],
    ['Control with Shift', CTRL_SHIFT],
  ])('leaves %s to a button that has focus, as Space and Enter do', async (_name, keys) => {
    const { user, asked } = mount({ focus: 'button' })
    await user.keyboard(keys)
    expect(asked).toEqual([])
  })

  it('leaves Space and Enter to a button that has focus, which the browser presses', async () => {
    const { user, asked } = mount({ focus: 'button' })
    await user.keyboard(`${SPACE}${ENTER}`)
    expect(asked).toEqual([])
    expect(screen.getByRole('button', { name: 'Keep stops' })).toHaveFocus()
  })

  it('takes the keys again once the field is left', async () => {
    const { user, asked } = mount({ focus: 'field' })
    await user.keyboard(`${SPACE}${CTRL}`)
    expect(asked).toEqual([])
    act(() => screen.getByRole('textbox', { name: 'A text field of the page' }).blur())
    await user.keyboard(CTRL)
    expect(asked).toEqual(['full control NextWindow'])
  })
})

describe('when the server changes the mode', () => {
  it('turns Enter from an end-of-turn yield into a pass as soon as the server turns Full Control on', async () => {
    const { user, asked, update, store } = mount()
    update(() => ({ fullControl: 'Held', pending: { id: 9, decision: { player: 0, kind: KINDS.Priority } } }))
    await user.keyboard(ENTER)
    expect(asked).toEqual(['pass'])
    expect(store.getState().fullControl).toBe('Held')
  })
})
