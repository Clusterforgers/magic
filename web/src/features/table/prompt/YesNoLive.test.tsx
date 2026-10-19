/** Goal: over a real session, a yes-or-no decision from the server draws its prompt, Yes and No (or Y and N) send one Answer that echoes the decision id and the boolean, the table clears the prompt as soon as it is answered, a new decision starts again with focus on No, a decision for another seat draws nothing, and a decision on a clock counts down in its own words and goes with the prompt at zero. */
import { act, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectId, ServerMessage } from '@/protocol/generated'
import { creaturesOf } from '@/test-utils/combatTable'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'

const SEATS = 2
const fixture = buildFixture({ seats: SEATS })
const [source] = creaturesOf(fixture, 0) as [ObjectId]

const question = (
  id: number,
  { player = 0, deadline, text = 'Draw a card?' }: { player?: number; deadline?: number; text?: string } = {},
): ServerMessage => ({
  type: 'Decision',
  id,
  decision: { player, kind: { YesNo: { source, text } } },
  ...(deadline === undefined ? {} : { deadline_ms: deadline }),
})

/** A live table of seat 0 that has been sent the fixture as a Snapshot. */
function mount() {
  const h = startSession()
  h.join(0, SEATS)
  act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
  const table = renderLiveTable(h.session, fixtureLookup(fixture))
  const send = (message: ServerMessage): void => act(() => h.deliver(message))
  const prompt = (): HTMLElement | null => table.container.querySelector('[data-decision="YesNo"]')
  return { h, ...table, send, prompt }
}

describe('a yes-or-no decision from the server', () => {
  it('draws the prompt with focus on No, and Yes sends one Answer with the decision id', async () => {
    const { h, send, user, prompt } = mount()
    send(question(31))
    expect(prompt()).not.toBeNull()
    expect(screen.getByRole('button', { name: 'No' })).toHaveFocus()
    await user.click(screen.getByRole('button', { name: 'Yes' }))
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 31, answer: { YesNo: true } }])
    expect(prompt()).toBeNull()
    expect(h.state().pending).toBeNull()
  })

  it('answers No from the N key', async () => {
    const { h, send, user } = mount()
    send(question(32))
    await user.keyboard('n')
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 32, answer: { YesNo: false } }])
  })

  it('sends one answer for a double click and for keys pressed twice', async () => {
    const { h, send, user } = mount()
    send(question(33))
    await user.dblClick(screen.getByRole('button', { name: 'Yes' }))
    expect(h.answeredIds()).toEqual([33])
    send(question(34))
    await user.keyboard('yy')
    expect(h.answeredIds()).toEqual([33, 34])
  })

  it('starts again for the next decision, with focus back on No', async () => {
    const { send, user } = mount()
    send(question(35))
    await user.tab({ shift: true })
    expect(screen.getByRole('button', { name: 'Yes' })).toHaveFocus()
    send(question(36, { text: 'Gain 2 life?' }))
    expect(screen.getByRole('button', { name: 'No' })).toHaveFocus()
    expect(document.querySelector('[data-slot="prompt-text"]')).toHaveTextContent('Gain 2 life?')
  })

  it('can be answered again when the server repeats it after a new snapshot', async () => {
    const { h, send, user } = mount()
    send(question(37))
    await user.click(screen.getByRole('button', { name: 'No' }))
    act(() => h.deliver({ type: 'Snapshot', seq: 2, state: fixture.view }))
    send(question(37))
    await user.click(screen.getByRole('button', { name: 'No' }))
    expect(h.answeredIds()).toEqual([37, 37])
  })

  it('draws no prompt for the question of another seat', () => {
    const { send, prompt } = mount()
    send(question(38, { player: 1 }))
    expect(prompt()).toBeNull()
  })

  it('counts down in the words of a question, and the prompt goes at zero when the server has answered', () => {
    const { h, send, prompt, container } = mount()
    send(question(39, { deadline: 10_000 }))
    const timer = (): string => container.querySelector('[role="timer"]')?.textContent ?? ''
    expect(timer()).toBe('Answering no for you in 10 s')
    act(() => h.timers.advance(7_000))
    expect(timer()).toBe('Answering no for you in 3 s')
    act(() => h.timers.advance(3_000))
    expect(prompt()).toBeNull()
  })

  it('has no countdown without a clock', () => {
    const { send, container } = mount()
    send(question(40))
    expect(container.querySelector('[role="timer"]')).toBeNull()
  })
})
