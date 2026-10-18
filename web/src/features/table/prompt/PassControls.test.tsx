// Goal: the Priority prompt passes through one primary button whose words say what passing does now, an "End turn" beside it and a small menu, each asking the session for the right yield exactly once per prompt; Space or Enter presses the primary button only when nothing else has the keyboard; and nothing says "pass priority" except the description a screen reader gets.
import { act, screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer, FullControlMode, PriorityOptions, Step, YieldUntil } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { primaryButton } from '@/test-utils/passControls'
import { renderTable } from '@/test-utils/tableHarness'
import { GameTable } from '../GameTable'
import { buildFixture } from '../dev/fixture'

const fixture = buildFixture({ seats: 2 })
const hand = must(fixture.view.players[0]?.hand.cards)
const landInHand = must(
  hand.find((id) => fixture.view.objects.find((object) => object.id === id)?.types.includes('Land')),
)

const options = (over: Partial<PriorityOptions> = {}): PriorityOptions => ({
  playable_lands: [],
  castable: [],
  mana_abilities: [],
  can_act: false,
  can_pass: true,
  ...over,
})

interface Situation {
  /** The step the table is in; the declare-blockers step of the fixture by default. */
  step?: Step
  /** Whose turn: the viewer's (seat 0) or the opponent's. */
  own?: boolean
  /** Whether the fixture's three spells are on the stack. */
  stacked?: boolean
  given?: PriorityOptions
  /** What the session answers to a request for a yield. */
  accepts?: boolean
  /** The Full Control mode the server holds. */
  fullControl?: FullControlMode
}

/** Seat 0 facing priority at `situation`, with spies for the session's answers and yields. */
function renderControls({
  step = 'DeclareBlockers',
  own = false,
  stacked = true,
  given = options(),
  accepts = true,
  fullControl = 'Off',
}: Situation = {}) {
  const answer = vi.fn((_answer: Answer) => true)
  const yieldUntil = vi.fn((_until: YieldUntil) => accepts)
  const cancelYield = vi.fn(() => true)
  const table = renderTable({
    session: { answer, yieldUntil, cancelYield },
    ui: (
      <>
        <GameTable />
        <input aria-label="A text field of the page" />
      </>
    ),
    state: (state) => {
      const view = clone(must(state.view))
      view.turn = { ...view.turn, step, active: own ? 0 : 1 }
      if (!stacked) view.stack = { count: 0, cards: [] }
      return {
        ...state,
        view,
        fullControl,
        pending: { id: 4, decision: { player: 0, kind: { Priority: given } } },
      }
    },
  })
  const primary = (): HTMLElement => primaryButton()
  const secondary = (): HTMLElement | null =>
    table.container.querySelector<HTMLElement>('[data-slot="secondary-action"]')
  const trigger = (): HTMLElement => screen.getByRole('button', { name: 'More ways to pass' })
  const card = (id: number): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`))
  return { ...table, answer, yieldUntil, cancelYield, primary, secondary, trigger, card }
}

describe('the words on the buttons', () => {
  it.each([
    ['over the spells on the stack', {}, 'Resolve Lightning Bolt'],
    ['over a stack on your own turn', { own: true }, 'Resolve Lightning Bolt'],
    [
      'on your own turn in the first main phase',
      { own: true, stacked: false, step: 'Main1' },
      'Next: Combat',
    ],
    [
      'on your own turn at the end of combat',
      { own: true, stacked: false, step: 'EndCombat' },
      'Next: Postcombat main',
    ],
    ['in the last window of your turn', { own: true, stacked: false, step: 'End' }, 'End turn'],
    ['on the opponent’s turn', { stacked: false, step: 'Upkeep' }, 'Continue'],
    ['in the last window of the opponent’s turn', { stacked: false, step: 'End' }, 'Continue'],
  ] satisfies [string, Situation, string][])('%s the primary button says "%s"', (_when, situation, label) => {
    const { primary } = renderControls(situation)
    expect(primary()).toHaveTextContent(label)
    expect(primary()).toHaveAccessibleName(label)
  })

  it('never says "pass priority" where it can be seen or used as a name', () => {
    const { container, primary } = renderControls({ own: true, stacked: false, step: 'Main1' })
    const line = must(container.querySelector('[data-slot="decision-line"]'))
    expect(line.textContent).not.toMatch(/pass priority/i)
    expect(primary().getAttribute('title')).not.toMatch(/priority/i)
    expect(screen.queryByRole('button', { name: /pass priority/i })).toBeNull()
  })

  it('says to a screen reader that it passes priority, and what then happens to a spell', () => {
    const stacked = renderControls()
    expect(stacked.primary()).toHaveAttribute(
      'aria-description',
      'Passes priority; if nobody responds, Lightning Bolt resolves.',
    )
    stacked.unmount()
    const empty = renderControls({ stacked: false })
    expect(empty.primary()).toHaveAttribute('aria-description', 'Passes priority.')
  })

  it('documents the keyboard in a visible hint, a tooltip and the key shortcuts', () => {
    const { primary } = renderControls()
    const hint = must(primary().querySelector('kbd'))
    expect(hint).toHaveTextContent('Space')
    expect(hint).toHaveAttribute('aria-hidden', 'true')
    expect(primary()).toHaveAttribute('title', 'Space or Enter')
    expect(primary()).toHaveAttribute('aria-keyshortcuts', 'Space Enter')
  })

  it('has "End turn" beside the primary button, except where the primary button already ends the turn', () => {
    expect(renderControls().secondary()).toHaveTextContent('End turn')
    expect(renderControls({ stacked: false, step: 'Upkeep' }).secondary()).toHaveTextContent('End turn')
    expect(renderControls({ own: true, stacked: false, step: 'Main2' }).secondary()).toHaveTextContent(
      'End turn',
    )
    expect(renderControls({ own: true, stacked: false, step: 'End' }).secondary()).toBeNull()
    // With a spell still on the stack in that window, ending the turn is more than the primary button does.
    expect(renderControls({ own: true, stacked: true, step: 'End' }).secondary()).toHaveTextContent(
      'End turn',
    )
  })

  it('has one prompt: its title, a live sentence and the buttons in a row', () => {
    const { container } = renderControls()
    const line = must(container.querySelector<HTMLElement>('[data-slot="decision-line"]'))
    const actions = must(line.querySelector<HTMLElement>('[data-slot="prompt-actions"]'))
    expect(within(actions).getAllByRole('button')).toHaveLength(3)
    expect(line.querySelectorAll('[aria-live]')).toHaveLength(1)
  })
})

describe('passing', () => {
  it('answers Pass when the primary button is pressed, once', async () => {
    const { primary, answer, yieldUntil, user } = renderControls({ own: true, stacked: false, step: 'Main1' })
    await user.dblClick(primary())
    expect(answer.mock.calls).toEqual([[{ Priority: 'Pass' }]])
    expect(yieldUntil).not.toHaveBeenCalled()
  })

  it('cannot be pressed when passing is not allowed, and neither can the others', async () => {
    const { primary, secondary, trigger, answer, yieldUntil, user } = renderControls({
      given: options({ playable_lands: [landInHand], can_pass: false }),
    })
    expect(primary()).toBeDisabled()
    expect(must(secondary())).toBeDisabled()
    expect(trigger()).toBeDisabled()
    await user.click(primary())
    await user.click(must(secondary()))
    expect(answer).not.toHaveBeenCalled()
    expect(yieldUntil).not.toHaveBeenCalled()
  })
})

describe('"End turn"', () => {
  it('asks the session to pass until the end of the turn, and answers nothing itself', async () => {
    const { secondary, answer, yieldUntil, user } = renderControls()
    await user.click(must(secondary()))
    expect(yieldUntil.mock.calls).toEqual([['EndOfTurn']])
    expect(answer).not.toHaveBeenCalled()
  })

  it('is asked for once per prompt, however often it is pressed, and takes the other buttons with it', async () => {
    const { primary, secondary, trigger, answer, yieldUntil, user } = renderControls()
    await user.dblClick(must(secondary()))
    expect(yieldUntil).toHaveBeenCalledTimes(1)
    expect(primary()).toBeDisabled()
    expect(must(secondary())).toBeDisabled()
    expect(trigger()).toBeDisabled()
    await user.click(primary())
    expect(answer).not.toHaveBeenCalled()
  })

  it('can be asked for again when the server sends a fresh prompt, which means it did not pass', async () => {
    const { secondary, yieldUntil, update, store, user } = renderControls()
    await user.click(must(secondary()))
    expect(must(secondary())).toBeDisabled()
    const pending = must(store.getState().pending)
    update(() => ({ pending: { ...pending, id: pending.id + 1 } }))
    await waitFor(() => expect(must(secondary())).toBeEnabled())
    await user.click(must(secondary()))
    expect(yieldUntil).toHaveBeenCalledTimes(2)
  })

  it('stays available when the session could not send the request', async () => {
    const { secondary, yieldUntil, user } = renderControls({ accepts: false })
    await user.click(must(secondary()))
    expect(yieldUntil).toHaveBeenCalledTimes(1)
    expect(must(secondary())).toBeEnabled()
    await user.click(must(secondary()))
    expect(yieldUntil).toHaveBeenCalledTimes(2)
  })
})

describe('under Full Control, which the server does not let a yield run beside', () => {
  const yieldControls = (): number =>
    document.querySelectorAll('[data-slot="secondary-action"], [data-slot="more-passing"]').length

  it.each(['NextWindow', 'Held'] satisfies FullControlMode[])(
    'offers only the primary button while the mode is %s',
    (fullControl) => {
      const { primary, secondary } = renderControls({ fullControl })
      expect(primary()).toBeEnabled()
      expect(secondary()).toBeNull()
      expect(screen.queryByRole('button', { name: 'More ways to pass' })).toBeNull()
      expect(yieldControls()).toBe(0)
    },
  )

  it('offers the yields again when the mode is off', () => {
    const { secondary, trigger } = renderControls({ fullControl: 'Off' })
    expect(secondary()).toHaveTextContent('End turn')
    expect(trigger()).toBeEnabled()
  })

  it('still passes once with the primary button', async () => {
    const { primary, answer, yieldUntil, user } = renderControls({ fullControl: 'Held' })
    await user.click(primary())
    expect(answer.mock.calls).toEqual([[{ Priority: 'Pass' }]])
    expect(yieldUntil).not.toHaveBeenCalled()
  })

  it('takes the yield buttons away when the server turns it on, and brings them back when it turns it off', () => {
    const { secondary, update } = renderControls()
    expect(yieldControls()).toBe(2)
    update(() => ({ fullControl: 'Held' }))
    expect(secondary()).toBeNull()
    expect(yieldControls()).toBe(0)
    update(() => ({ fullControl: 'Off' }))
    expect(yieldControls()).toBe(2)
  })

  it('keeps the primary button free when the server turns it on', () => {
    const { primary, update } = renderControls({ own: true, stacked: false, step: 'Main1' })
    update(() => ({ fullControl: 'NextWindow' }))
    expect(primary()).toBeEnabled()
    expect(primary()).toHaveTextContent('Next: Combat')
  })
})

describe('the menu', () => {
  const items = async (): Promise<string[]> =>
    (await screen.findAllByRole('menuitem')).map((item) => item.textContent ?? '')

  it('is closed until it is asked for, and has two ways to pass over a stack', async () => {
    const { trigger, user } = renderControls()
    expect(screen.queryByRole('menu')).toBeNull()
    expect(trigger()).toHaveAttribute('aria-haspopup', 'menu')
    await user.click(trigger())
    expect(await items()).toEqual(['Resolve everything on the stack', 'Pass until my next turn'])
  })

  it('leaves out resolving the stack when there is none', async () => {
    const { trigger, user } = renderControls({ stacked: false, own: true, step: 'Main1' })
    await user.click(trigger())
    expect(await items()).toEqual(['Pass until my next turn'])
  })

  it.each([
    ['Resolve everything on the stack', 'Resolve'],
    ['Pass until my next turn', 'MyTurn'],
  ] satisfies [string, YieldUntil][])('"%s" asks for the yield until %s', async (name, until) => {
    const { trigger, yieldUntil, answer, user } = renderControls()
    await user.click(trigger())
    await user.click(await screen.findByRole('menuitem', { name }))
    expect(yieldUntil.mock.calls).toEqual([[until]])
    expect(answer).not.toHaveBeenCalled()
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())
  })

  it('describes each item to a screen reader', async () => {
    const { trigger, user } = renderControls()
    await user.click(trigger())
    expect(await screen.findByRole('menuitem', { name: 'Resolve everything on the stack' })).toHaveAttribute(
      'aria-description',
      expect.stringContaining('until the stack is empty'),
    )
    expect(await screen.findByRole('menuitem', { name: 'Pass until my next turn' })).toHaveAttribute(
      'aria-description',
      expect.stringContaining('until your next turn begins'),
    )
  })

  it('works from the keyboard alone', async () => {
    const { trigger, yieldUntil, user } = renderControls()
    act(() => trigger().focus())
    await user.keyboard('{Enter}')
    await screen.findByRole('menu')
    await user.keyboard('{ArrowDown}{Enter}')
    await waitFor(() => expect(yieldUntil).toHaveBeenCalledTimes(1))
    expect(['Resolve', 'MyTurn']).toContain(yieldUntil.mock.calls[0]?.[0])
  })

  it('asks for nothing when it is dismissed, and gives focus back to the trigger', async () => {
    const { trigger, yieldUntil, user } = renderControls()
    act(() => trigger().focus())
    await user.keyboard('{Enter}')
    await screen.findByRole('menu')
    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())
    expect(yieldUntil).not.toHaveBeenCalled()
    expect(trigger()).toHaveFocus()
  })
})

describe('Space and Enter', () => {
  it.each([
    ['Space', ' '],
    ['Enter', '{Enter}'],
  ])('%s presses the primary button when nothing has the keyboard', async (_name, keys) => {
    const { answer, user } = renderControls({ own: true, stacked: false, step: 'Main1' })
    await user.keyboard(keys)
    expect(answer.mock.calls).toEqual([[{ Priority: 'Pass' }]])
  })

  it('presses it once for a key held down, and again for none once it has answered', async () => {
    const { answer, user } = renderControls()
    await user.keyboard('{Enter>4/}')
    expect(answer).toHaveBeenCalledTimes(1)
  })

  it('leaves the key to a text field, so a chat line can be typed', async () => {
    const { answer, user } = renderControls()
    const field = screen.getByRole('textbox', { name: 'A text field of the page' })
    await user.click(field)
    await user.keyboard('good game{Enter}')
    expect((field as HTMLInputElement).value).toBe('good game')
    expect(answer).not.toHaveBeenCalled()
  })

  it('leaves it to a card that has focus, which plays itself, and passes nothing', async () => {
    const { card, answer, user } = renderControls({
      own: true,
      stacked: false,
      step: 'Main1',
      given: options({ playable_lands: [landInHand], can_act: true }),
    })
    act(() => card(landInHand).focus())
    await user.keyboard('{Enter}')
    expect(answer.mock.calls).toEqual([[{ Priority: { PlayLand: landInHand } }]])
  })

  it('leaves it to the button that has focus', async () => {
    const { trigger, answer, user } = renderControls()
    act(() => trigger().focus())
    await user.keyboard(' ')
    expect(answer).not.toHaveBeenCalled()
    await screen.findByRole('menu')
  })

  it('does nothing when passing is not allowed', async () => {
    const { answer, user } = renderControls({ given: options({ can_pass: false }) })
    await user.keyboard(' {Enter}')
    expect(answer).not.toHaveBeenCalled()
  })

  it('does nothing once a yield was asked for, which the server is about to answer', async () => {
    const { secondary, answer, user } = renderControls()
    await user.click(must(secondary()))
    await user.keyboard(' {Enter}')
    expect(answer).not.toHaveBeenCalled()
  })

  it('does nothing while a modifier is held', async () => {
    const { answer, user } = renderControls()
    await user.keyboard('{Control>} {/Control}{Alt>}{Enter}{/Alt}{Shift>} {/Shift}')
    expect(answer).not.toHaveBeenCalled()
  })

  it('goes with the prompt: no decision, no shortcut', async () => {
    const { answer, update, user } = renderControls()
    update(() => ({ pending: null }))
    await user.keyboard(' {Enter}')
    expect(answer).not.toHaveBeenCalled()
  })
})
