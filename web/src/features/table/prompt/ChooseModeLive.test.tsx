/** Goal: over a real session, a mode decision from the server draws its prompt, a mode or Cancel sends one Answer that echoes the decision id, the table clears the prompt at once, a decision for another seat draws nothing, and a decision on a clock counts down in the words of what the server does (cancel a spell that can be backed out of, else take the first mode) and goes with the prompt at zero. */
import { act, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ServerMessage } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'

const SEATS = 2
const fixture = buildFixture({ seats: SEATS })
const spell = must(fixture.view.objects.find((o) => o.zone === 'Stack' && o.controller === 0))

const modes = (
  id: number,
  { player = 0, deadline, cancel = true }: { player?: number; deadline?: number; cancel?: boolean } = {},
): ServerMessage => ({
  type: 'Decision',
  id,
  decision: {
    player,
    kind: {
      ChooseMode: {
        source: spell.id,
        modes: [
          { index: 0, label: 'Deal 2 damage' },
          { index: 2, label: 'Draw a card' },
        ],
        min: 1,
        max: 1,
        repeat: false,
        can_cancel: cancel,
      },
    },
  },
  ...(deadline === undefined ? {} : { deadline_ms: deadline }),
})

function mount() {
  const h = startSession()
  h.join(0, SEATS)
  act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
  const table = renderLiveTable(h.session, fixtureLookup(fixture))
  const send = (message: ServerMessage): void => act(() => h.deliver(message))
  const prompt = (): HTMLElement | null => table.container.querySelector('[data-decision="ChooseMode"]')
  const timer = (): string => table.container.querySelector('[role="timer"]')?.textContent ?? ''
  return { h, ...table, send, prompt, timer }
}

describe('a mode decision from the server', () => {
  it('draws the prompt, and a mode sends one Answer with the decision id and its index', async () => {
    const { h, send, user, prompt } = mount()
    send(modes(61))
    expect(prompt()).not.toBeNull()
    await user.click(screen.getByRole('button', { name: 'Draw a card' }))
    expect(h.sentOf('Answer')).toEqual([
      { type: 'Answer', decision: 61, answer: { ChooseMode: { Modes: [2] } } },
    ])
    expect(prompt()).toBeNull()
    expect(h.state().pending).toBeNull()
  })

  it('sends Cancel for a spell that is taken back', async () => {
    const { h, send, user } = mount()
    send(modes(62))
    await user.click(screen.getByRole('button', { name: 'Cancel the spell' }))
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 62, answer: { ChooseMode: 'Cancel' } }])
  })

  it('answers from a number key, once', async () => {
    const { h, send, user } = mount()
    send(modes(63))
    await user.keyboard('11')
    expect(h.sentOf('Answer')).toEqual([
      { type: 'Answer', decision: 63, answer: { ChooseMode: { Modes: [0] } } },
    ])
  })

  it('draws no prompt for the mode decision of another seat', () => {
    const { send, prompt } = mount()
    send(modes(64, { player: 1 }))
    expect(prompt()).toBeNull()
  })

  it.each([
    ['cancels a spell that can be backed out of', true, 'Cancelling for you'],
    ['takes the first mode of a trigger', false, 'Choosing the first mode for you'],
  ])('on a clock it %s, and the prompt goes at zero', (_what, cancel, words) => {
    const { h, send, prompt, timer } = mount()
    send(modes(65, { deadline: 10_000, cancel }))
    expect(timer()).toBe(`${words} in 10 s`)
    act(() => h.timers.advance(8_000))
    expect(timer()).toBe(`${words} in 2 s`)
    act(() => h.timers.advance(2_000))
    expect(prompt()).toBeNull()
  })
})
