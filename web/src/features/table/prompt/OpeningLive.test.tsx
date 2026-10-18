/** Goal: over a real session, the Mulligan, BottomCards and Discard decisions from the server draw their prompts, a double or triple click sends one Answer that echoes the decision id and holds the cards in the order chosen, the table clears its marks once answered, and a decision the server replaces starts with nothing chosen and the new options. */
import { act, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { DecisionKind, ObjectId, ServerMessage } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'

const fixture = buildFixture({ seats: 2 })
const hand = must(fixture.view.players[0]?.hand.cards) as [ObjectId, ...ObjectId[]]

const decision = (id: number, kind: DecisionKind): ServerMessage => ({
  type: 'Decision',
  id,
  decision: { player: 0, kind },
})
const mulligan = (taken = 0): DecisionKind => ({
  Mulligan: { hand, mulligans_taken: taken, can_mulligan: true },
})
const bottom = (count: number): DecisionKind => ({ BottomCards: { hand, count } })
const discard = (count: number): DecisionKind => ({ Discard: { hand, count } })

/** A live table of seat 0 that has been sent the fixture as a Snapshot. */
function mount() {
  const h = startSession()
  h.join(0, 2)
  act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
  const table = renderLiveTable(h.session, fixtureLookup(fixture))
  const card = (id: ObjectId): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`))
  const send = (message: ServerMessage): void => act(() => h.deliver(message))
  const marked = (): number => table.container.querySelectorAll('[data-highlight]').length
  const pressed = (): ObjectId[] => hand.filter((id) => card(id).getAttribute('aria-pressed') === 'true')
  const text = (): string =>
    must(table.container.querySelector('[data-slot="prompt-text"]')).textContent ?? ''
  const confirm = (): HTMLElement => screen.getByRole('button', { name: 'Confirm' })
  return { h, ...table, card, send, marked, pressed, text, confirm }
}

describe('Mulligan', () => {
  it('draws the prompt and a double click on Keep sends one Answer with the decision id', async () => {
    const { h, send, user, container, text } = mount()
    send(decision(21, mulligan()))
    expect(container.querySelector('[data-decision="Mulligan"]')).not.toBeNull()
    expect(text()).toContain('Keep this hand of 7 cards')
    await user.dblClick(screen.getByRole('button', { name: 'Keep' }))
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 21, answer: { Mulligan: 'Keep' } }])
    expect(container.querySelector('[data-decision]')).toBeNull()
  })

  it('sends one Answer for Enter twice on Mulligan, and shows the new count when asked again', async () => {
    const { h, send, user, text } = mount()
    send(decision(22, mulligan()))
    act(() => screen.getByRole('button', { name: 'Mulligan' }).focus())
    await user.keyboard('{Enter}{Enter}')
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 22, answer: { Mulligan: 'Mulligan' } }])
    send(decision(23, mulligan(1)))
    expect(text()).toContain('You have taken 1 mulligan so far.')
    expect(text()).toContain('puts 2 cards on the bottom')
  })

  it('names the seat still deciding and stops when a new snapshot says it kept', () => {
    const { h, send, container } = mount()
    const view = clone(fixture.view)
    view.stage = 'Mulligan'
    must(view.players[1]).mulligan = 'Deciding'
    act(() => h.deliver({ type: 'Snapshot', seq: 2, state: view }))
    send(decision(24, mulligan()))
    expect(container.querySelector('[data-slot="mulligan-waiting"]')).toHaveTextContent('Still deciding: Bo.')
    must(view.players[1]).mulligan = 'Kept'
    act(() => h.deliver({ type: 'Snapshot', seq: 3, state: view }))
    expect(container.querySelector('[data-slot="mulligan-waiting"]')).toBeNull()
  })
})

describe.each([
  ['BottomCards', bottom, 'BottomCards'],
  ['Discard', discard, 'Discard'],
] as const)('%s', (name, kind, key) => {
  it('draws the prompt and marks the offered cards, and clears the marks when answered', async () => {
    const { send, container, marked, card, user, confirm } = mount()
    send(decision(31, kind(1)))
    expect(container.querySelector(`[data-decision="${name}"]`)).not.toBeNull()
    expect(card(must(hand[0]))).toHaveAttribute('data-highlight', 'legal')
    await user.click(card(must(hand[0])))
    await user.click(confirm())
    expect(marked()).toBe(0)
  })

  it('sends one Answer with the cards in the order chosen for a triple click on Confirm', async () => {
    const { h, send, user, card, confirm } = mount()
    send(decision(32, kind(2)))
    await user.click(card(must(hand[4])))
    await user.click(card(must(hand[1])))
    await user.tripleClick(confirm())
    expect(h.sentOf('Answer')).toEqual([
      { type: 'Answer', decision: 32, answer: { [key]: [hand[4], hand[1]] } },
    ])
    expect(h.answeredIds()).toEqual([32])
  })

  it('sends one Answer for Enter twice on Confirm', async () => {
    const { h, send, user, card, confirm } = mount()
    send(decision(33, kind(1)))
    act(() => card(must(hand[2])).focus())
    await user.keyboard('{Enter}')
    act(() => confirm().focus())
    await user.keyboard('{Enter}{Enter}')
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 33, answer: { [key]: [hand[2]] } }])
  })

  it('starts a replaced decision with nothing chosen and the new options', async () => {
    const { send, user, card, pressed, text, confirm } = mount()
    send(decision(34, kind(2)))
    await user.click(card(must(hand[0])))
    await user.click(card(must(hand[1])))
    expect(pressed()).toHaveLength(2)
    expect(confirm()).toBeEnabled()
    send(decision(35, kind(3)))
    expect(pressed()).toEqual([])
    expect(text()).toContain('0 of 3 chosen.')
    expect(confirm()).toBeDisabled()
  })

  it('can be answered again when the server repeats it after a new snapshot', async () => {
    const { h, send, user, card, confirm } = mount()
    send(decision(36, kind(1)))
    await user.click(card(must(hand[0])))
    await user.click(confirm())
    act(() => h.deliver({ type: 'Snapshot', seq: 2, state: fixture.view }))
    send(decision(36, kind(1)))
    await user.click(card(must(hand[0])))
    await user.click(confirm())
    expect(h.answeredIds()).toEqual([36, 36])
  })

  it('takes the prompt and the marks away when the connection is lost', async () => {
    const { h, send, container, marked, card, user } = mount()
    send(decision(37, kind(1)))
    await user.click(card(must(hand[0])))
    expect(marked()).toBeGreaterThan(0)
    act(() => h.net.last.fail())
    expect(container.querySelector('[data-decision]')).toBeNull()
    expect(marked()).toBe(0)
  })
})
