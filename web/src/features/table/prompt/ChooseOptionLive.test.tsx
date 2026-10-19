/** Goal: over a real session, an option decision from the server draws its prompt with focus on the first option, a click or a number key sends one Answer that echoes the decision id and the index, the table clears the prompt at once, a decision for another seat draws nothing, and a decision on a clock counts down in its own words and goes with the prompt at zero. */
import { act, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ChoiceOption, ObjectId, ServerMessage } from '@/protocol/generated'
import { creaturesOf } from '@/test-utils/combatTable'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'

const SEATS = 2
const fixture = buildFixture({ seats: SEATS })
const [source] = creaturesOf(fixture, 0) as [ObjectId]
const COLORS: ChoiceOption[] = [
  { Color: 'White' },
  { Color: 'Blue' },
  { Color: 'Black' },
  { Color: 'Red' },
  { Color: 'Green' },
]

const choice = (
  id: number,
  { player = 0, deadline }: { player?: number; deadline?: number } = {},
): ServerMessage => ({
  type: 'Decision',
  id,
  decision: { player, kind: { ChooseOption: { source, options: COLORS, text: 'Choose a color.' } } },
  ...(deadline === undefined ? {} : { deadline_ms: deadline }),
})

function mount() {
  const h = startSession()
  h.join(0, SEATS)
  act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
  const table = renderLiveTable(h.session, fixtureLookup(fixture))
  const send = (message: ServerMessage): void => act(() => h.deliver(message))
  const prompt = (): HTMLElement | null => table.container.querySelector('[data-decision="ChooseOption"]')
  return { h, ...table, send, prompt }
}

describe('an option decision from the server', () => {
  it('draws the prompt with focus on the first option, and a click sends one Answer with the decision id and the index', async () => {
    const { h, send, user, prompt } = mount()
    send(choice(51))
    expect(prompt()).not.toBeNull()
    const options = within(screen.getByRole('group', { name: 'Options' })).getAllByRole('button')
    expect(options).toHaveLength(5)
    expect(options[0]).toHaveFocus()
    await user.click(screen.getByRole('button', { name: 'Black' }))
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 51, answer: { ChooseOption: 2 } }])
    expect(prompt()).toBeNull()
    expect(h.state().pending).toBeNull()
  })

  it('answers from a number key, once', async () => {
    const { h, send, user } = mount()
    send(choice(52))
    await user.keyboard('44')
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 52, answer: { ChooseOption: 3 } }])
  })

  it('draws no prompt for the option decision of another seat', () => {
    const { send, prompt } = mount()
    send(choice(53, { player: 1 }))
    expect(prompt()).toBeNull()
  })

  it('counts down in the words of an option decision, and the prompt goes at zero when the server has answered', () => {
    const { h, send, prompt, container } = mount()
    send(choice(54, { deadline: 10_000 }))
    const timer = (): string => container.querySelector('[role="timer"]')?.textContent ?? ''
    expect(timer()).toBe('Choosing the first option for you in 10 s')
    act(() => h.timers.advance(7_000))
    expect(timer()).toBe('Choosing the first option for you in 3 s')
    act(() => h.timers.advance(3_000))
    expect(prompt()).toBeNull()
  })
})
