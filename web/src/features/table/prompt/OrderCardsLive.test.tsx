/** Goal: over a real session, an order-of-cards decision from the server draws its prompt, Confirm order sends one Answer that echoes the decision id and the cards as ordered, the table clears the prompt as soon as it is answered, a decision for another seat draws nothing, and a decision on a clock counts down in the words of keeping the order and goes with the prompt at zero. */
import { act, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectId, ServerMessage } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf } from '@/test-utils/combatTable'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'

const SEATS = 2
const fixture = buildFixture({ seats: SEATS })
const hand = must(fixture.view.players[0]?.hand.cards)
const [source] = creaturesOf(fixture, 1) as [ObjectId]
const cards = hand.slice(0, 3)

const asked = (
  id: number,
  { player = 0, deadline }: { player?: number; deadline?: number } = {},
): ServerMessage => ({
  type: 'Decision',
  id,
  decision: {
    player,
    kind: { OrderCards: { source, cards, text: 'Put the cards on the bottom in any order.' } },
  },
  ...(deadline === undefined ? {} : { deadline_ms: deadline }),
})

function mount() {
  const h = startSession()
  h.join(0, SEATS)
  act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
  const table = renderLiveTable(h.session, fixtureLookup(fixture))
  const send = (message: ServerMessage): void => act(() => h.deliver(message))
  const prompt = (): HTMLElement | null => table.container.querySelector('[data-decision="OrderCards"]')
  const timer = (): string => table.container.querySelector('[role="timer"]')?.textContent ?? ''
  return { h, ...table, send, prompt, timer }
}

describe('an order of cards from the server', () => {
  it('draws the prompt, and Confirm order sends the cards as ordered with the decision id', async () => {
    const { h, send, user, prompt } = mount()
    send(asked(51))
    expect(prompt()).not.toBeNull()
    const [first] = cards as [ObjectId]
    await user.click(
      must(document.querySelector<HTMLElement>(`button[data-card="${first}"][data-move="down"]`)),
    )
    await user.dblClick(screen.getByRole('button', { name: 'Confirm order' }))
    expect(h.sentOf('Answer')).toEqual([
      { type: 'Answer', decision: 51, answer: { OrderCards: [cards[1], cards[0], cards[2]] } },
    ])
    expect(prompt()).toBeNull()
    expect(h.state().pending).toBeNull()
  })

  it('starts the next decision in its own order', async () => {
    const { send, user } = mount()
    send(asked(52))
    await user.click(
      must(document.querySelector<HTMLElement>(`button[data-card="${cards[2]}"][data-move="up"]`)),
    )
    send(asked(53))
    expect(screen.getByRole('button', { name: 'Original order' })).toBeDisabled()
  })

  it('draws no prompt for the order of another seat', () => {
    const { send, prompt } = mount()
    send(asked(54, { player: 1 }))
    expect(prompt()).toBeNull()
  })

  it('counts down in the words of keeping the order, and the prompt goes at zero', () => {
    const { h, send, prompt, timer } = mount()
    send(asked(55, { deadline: 10_000 }))
    expect(timer()).toBe('Keeping the order for you in 10 s')
    act(() => h.timers.advance(10_000))
    expect(prompt()).toBeNull()
  })

  it('has no countdown without a clock', () => {
    const { send, container } = mount()
    send(asked(56))
    expect(container.querySelector('[role="timer"]')).toBeNull()
  })
})
