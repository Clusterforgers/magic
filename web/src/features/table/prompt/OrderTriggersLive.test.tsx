/** Goal: over a real session, an order decision from the server draws its prompt with the engine's own order in place, Up and Down rearrange it, Confirm order sends one Answer that echoes the decision id and lists the triggers in the order they go on the stack, the table clears the prompt as soon as it is answered, a decision the server replaces (or repeats after a resync) starts again from its default order, a decision for another seat draws nothing, and a decision on a clock counts down in its own words and goes with the prompt at zero. */
import { act, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectId, ServerMessage, TriggerEntry } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf } from '@/test-utils/combatTable'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'

const SEATS = 2
const fixture = buildFixture({ seats: SEATS })
const sources = creaturesOf(fixture, 0) as [ObjectId, ObjectId, ObjectId, ObjectId]

const entries = (count: number): TriggerEntry[] =>
  sources.slice(0, count).map((id, index) => ({
    source: { id, counter: 0 },
    ability: 0,
    when: [
      'When it enters the battlefield',
      'Whenever it attacks',
      'When it dies',
      'At the beginning of your end step',
    ][index],
  }))

const order = (
  id: number,
  triggers: TriggerEntry[],
  { player = 0, deadline }: { player?: number; deadline?: number } = {},
): ServerMessage => ({
  type: 'Decision',
  id,
  decision: { player, kind: { OrderTriggers: { triggers } } },
  ...(deadline === undefined ? {} : { deadline_ms: deadline }),
})

/** A live table of seat 0 that has been sent the fixture as a Snapshot. */
function mount() {
  const h = startSession()
  h.join(0, SEATS)
  act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
  const table = renderLiveTable(h.session, fixtureLookup(fixture))
  const send = (message: ServerMessage): void => act(() => h.deliver(message))
  const prompt = (): HTMLElement | null => table.container.querySelector('[data-decision="OrderTriggers"]')
  const shown = (): number[] =>
    within(screen.getByRole('list', { name: /^Your triggers/ }))
      .getAllByRole('listitem')
      .map((row) => Number(must(row.querySelector('button')).getAttribute('data-entry')))
  const move = (entry: number, way: 'up' | 'down'): HTMLElement =>
    must(document.querySelector<HTMLElement>(`button[data-entry="${entry}"][data-move="${way}"]`))
  const confirm = (): HTMLElement => screen.getByRole('button', { name: 'Confirm order' })
  return { h, ...table, send, prompt, shown, move, confirm }
}

describe('an order decision from the server', () => {
  it('draws the prompt in the default order, and Confirm order sends one Answer with the decision id and the order', async () => {
    const { h, send, user, prompt, shown, move, confirm } = mount()
    send(order(41, entries(3)))
    expect(prompt()).not.toBeNull()
    expect(shown()).toEqual([2, 1, 0])
    await user.click(move(0, 'up'))
    expect(shown()).toEqual([2, 0, 1])
    await user.click(confirm())
    expect(h.sentOf('Answer')).toEqual([
      { type: 'Answer', decision: 41, answer: { OrderTriggers: [1, 0, 2] } },
    ])
    expect(prompt()).toBeNull()
    expect(h.state().pending).toBeNull()
  })

  it('answers the order the triggers came in when the list is not touched', async () => {
    const { h, send, user, confirm } = mount()
    send(order(42, entries(4)))
    await user.click(confirm())
    expect(h.sentOf('Answer')).toEqual([
      { type: 'Answer', decision: 42, answer: { OrderTriggers: [0, 1, 2, 3] } },
    ])
  })

  it.each([
    [
      'a double click on Confirm order',
      async (user: ReturnType<typeof mount>['user']) =>
        user.dblClick(screen.getByRole('button', { name: 'Confirm order' })),
    ],
    [
      'Enter twice on Confirm order',
      async (user: ReturnType<typeof mount>['user']) => {
        act(() => screen.getByRole('button', { name: 'Confirm order' }).focus())
        await user.keyboard('{Enter}{Enter}')
      },
    ],
  ])('sends one answer for %s', async (_how, gesture) => {
    const { h, send, user } = mount()
    send(order(43, entries(2)))
    await gesture(user)
    expect(h.sentOf('Answer')).toHaveLength(1)
    expect(h.answeredIds()).toEqual([43])
  })

  it('starts again from the default order when the server replaces the decision', async () => {
    const { send, user, shown, move } = mount()
    send(order(44, entries(3)))
    await user.click(move(0, 'up'))
    await user.click(move(0, 'up'))
    expect(shown()).toEqual([0, 2, 1])
    send(order(45, entries(2)))
    expect(shown()).toEqual([1, 0])
    expect(screen.getByRole('button', { name: 'Default order' })).toBeDisabled()
  })

  it('can be answered again when the server repeats it after a new snapshot', async () => {
    const { h, send, user, confirm } = mount()
    send(order(46, entries(2)))
    await user.click(confirm())
    act(() => h.deliver({ type: 'Snapshot', seq: 2, state: fixture.view }))
    send(order(46, entries(2)))
    await user.click(confirm())
    expect(h.answeredIds()).toEqual([46, 46])
  })

  it('draws no prompt for the order decision of another seat', () => {
    const { send, prompt } = mount()
    send(order(47, entries(2), { player: 1 }))
    expect(prompt()).toBeNull()
  })

  it('marks no card on the table, since none of them can be chosen', () => {
    const { send, container } = mount()
    send(order(48, entries(2)))
    expect(container.querySelectorAll('[data-highlight]').length).toBe(0)
  })

  it('counts down in the words of an order decision, and the prompt goes at zero when the server has answered', () => {
    const { h, send, prompt, container } = mount()
    send(order(49, entries(2), { deadline: 10_000 }))
    const timer = (): string => container.querySelector('[role="timer"]')?.textContent ?? ''
    expect(timer()).toBe('Ordering the triggers for you in 10 s')
    act(() => h.timers.advance(7_000))
    expect(timer()).toBe('Ordering the triggers for you in 3 s')
    act(() => h.timers.advance(3_000))
    expect(prompt()).toBeNull()
  })
})
