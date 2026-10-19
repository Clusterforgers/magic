/** Goal: over a real session, a card-choice decision from the server draws its prompt with focus on the first card, Confirm sends one Answer that echoes the decision id and the cards in the order chosen, the table clears the prompt as soon as it is answered, a new decision starts with an empty choice, a decision for another seat draws nothing, and a decision on a clock counts down in the words of what the server would choose and goes with the prompt at zero. */
import { act, screen, within } from '@testing-library/react'
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

interface Asked {
  player?: number
  deadline?: number
  min?: number
  max?: number
  from?: ObjectId[]
  text?: string
}

const asked = (
  id: number,
  { player = 0, deadline, min = 1, max = 1, from = hand.slice(0, 3), text = 'Discard a card.' }: Asked = {},
): ServerMessage => ({
  type: 'Decision',
  id,
  decision: { player, kind: { ChooseCards: { source, from, min, max, text } } },
  ...(deadline === undefined ? {} : { deadline_ms: deadline }),
})

/** A live table of seat 0 that has been sent the fixture as a Snapshot. */
function mount() {
  const h = startSession()
  h.join(0, SEATS)
  act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
  const table = renderLiveTable(h.session, fixtureLookup(fixture))
  const send = (message: ServerMessage): void => act(() => h.deliver(message))
  const prompt = (): HTMLElement | null => table.container.querySelector('[data-decision="ChooseCards"]')
  const cards = (): HTMLElement[] =>
    within(screen.getByRole('group', { name: 'Cards to choose from' })).getAllByRole('button')
  const timer = (): string => table.container.querySelector('[role="timer"]')?.textContent ?? ''
  return { h, ...table, send, prompt, cards, timer }
}

describe('a card choice from the server', () => {
  it('draws the prompt with focus on the first card, and Confirm sends the cards with the decision id', async () => {
    const { h, send, user, prompt, cards } = mount()
    send(asked(41, { min: 2, max: 2 }))
    expect(prompt()).not.toBeNull()
    expect(cards()[0]).toHaveFocus()
    await user.click(must(cards()[2]))
    await user.click(must(cards()[0]))
    await user.click(screen.getByRole('button', { name: 'Confirm' }))
    expect(h.sentOf('Answer')).toEqual([
      { type: 'Answer', decision: 41, answer: { ChooseCards: [hand[2], hand[0]] } },
    ])
    expect(prompt()).toBeNull()
    expect(h.state().pending).toBeNull()
  })

  it('sends nothing while the number is not legal, and one answer for a double click', async () => {
    const { h, send, user, cards } = mount()
    send(asked(42))
    const confirm = screen.getByRole('button', { name: 'Confirm' })
    await user.click(confirm)
    expect(h.sentOf('Answer')).toEqual([])
    await user.keyboard('2')
    await user.dblClick(confirm)
    expect(h.answeredIds()).toEqual([42])
    expect(cards).toBeDefined()
  })

  it('starts the next decision with an empty choice and its own cards', async () => {
    const { send, user, cards } = mount()
    send(asked(43))
    await user.click(must(cards()[1]))
    expect(screen.getByRole('button', { name: 'Confirm' })).toBeEnabled()
    send(asked(44, { from: hand.slice(3, 5), text: 'Sacrifice a creature.' }))
    expect(screen.getByRole('button', { name: 'Confirm' })).toBeDisabled()
    expect(cards()).toHaveLength(2)
    expect(document.querySelector('[data-slot="prompt-text"]')).toHaveTextContent('Sacrifice a creature.')
  })

  it('can be answered again when the server repeats it after a new snapshot', async () => {
    const { h, send, user, cards } = mount()
    send(asked(45))
    await user.click(must(cards()[0]))
    await user.click(screen.getByRole('button', { name: 'Confirm' }))
    act(() => h.deliver({ type: 'Snapshot', seq: 2, state: fixture.view }))
    send(asked(45))
    await user.click(must(cards()[0]))
    await user.click(screen.getByRole('button', { name: 'Confirm' }))
    expect(h.answeredIds()).toEqual([45, 45])
  })

  it('draws no prompt for the choice of another seat', () => {
    const { send, prompt } = mount()
    send(asked(46, { player: 1 }))
    expect(prompt()).toBeNull()
  })
})

describe('the response clock', () => {
  it.each([
    [1, 'Choosing the first cards for you in 10 s'],
    [2, 'Choosing the first cards for you in 10 s'],
    [0, 'Choosing no cards for you in 10 s'],
  ])('of a choice of at least %i cards counts "%s", and the prompt goes at zero', (min, words) => {
    const { h, send, prompt, timer } = mount()
    send(asked(47, { deadline: 10_000, min, max: 3 }))
    expect(timer()).toBe(words)
    act(() => h.timers.advance(7_000))
    expect(timer()).toBe(words.replace('10 s', '3 s'))
    act(() => h.timers.advance(3_000))
    expect(prompt()).toBeNull()
  })

  it('has no countdown without a clock', () => {
    const { send, container } = mount()
    send(asked(48))
    expect(container.querySelector('[role="timer"]')).toBeNull()
  })
})
