/** Goal: over a real session, a payment decision from the server draws the prompt, a payment built by clicks goes out as one Manual Answer that echoes the decision id (and so do Auto and Cancel, even on a double click or a double Enter), the table clears its marks once answered, and a decision the server replaces or repeats starts with no payment and the new options. */
import { act, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectId, PayOptions, ServerMessage } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { cost, poolOf, source } from '@/test-utils/mana'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'

const fixture = buildFixture({ seats: 2 })
const lands = fixture.view.objects
  .filter(
    (object) =>
      object.controller === 0 &&
      object.zone === 'Battlefield' &&
      object.permanent?.tapped === false &&
      object.types.includes('Land'),
  )
  .map((object) => object.id)
// The lands are stand-ins: the options say what each makes, whatever their names.
const [greenLand, blueLand, redLand, blackLand] = lands as [ObjectId, ObjectId, ObjectId, ObjectId]
const spell = must(fixture.view.players[0]?.hand.cards?.at(-1))

const pay = (text: string, over: Partial<PayOptions> = {}): PayOptions => ({
  spell,
  cost: cost(text),
  pool: poolOf(''),
  sources: [source(greenLand, 'G'), source(blueLand, 'U'), source(redLand, 'R'), source(blackLand, 'B')],
  auto_available: true,
  ...over,
})
const decision = (id: number, given: PayOptions, player = 0): ServerMessage => ({
  type: 'Decision',
  id,
  decision: { player, kind: { PayMana: given } },
})

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
  const button = (name: string): HTMLElement => screen.getByRole('button', { name })
  return { h, ...table, card, send, marked, button }
}

describe('a payment decision from the server', () => {
  it('draws the prompt and marks the sources, and a payment goes out as one Manual Answer with the decision id', async () => {
    const { h, card, send, user, marked, container, button } = mount()
    expect(container.querySelector('[data-decision]')).toBeNull()
    send(decision(21, pay('{1}{G}')))
    expect(container.querySelector('[data-decision="PayMana"]')).not.toBeNull()
    expect(card(greenLand)).toHaveAttribute('data-highlight', 'legal')
    await user.click(card(blueLand))
    await user.click(card(greenLand))
    expect(card(greenLand)).toHaveAttribute('data-highlight', 'selected')
    await user.click(button('Confirm payment'))
    expect(h.sentOf('Answer')).toEqual([
      {
        type: 'Answer',
        decision: 21,
        answer: {
          PayMana: {
            Manual: {
              pool: [],
              taps: [
                { object: blueLand, ability: 0 },
                { object: greenLand, ability: 0 },
              ],
            },
          },
        },
      },
    ])
    expect(container.querySelector('[data-decision]')).toBeNull()
    expect(marked()).toBe(0)
    expect(h.state().pending).toBeNull()
  })

  it.each([
    ['Pay automatically', { PayMana: 'Auto' }],
    ['Cancel casting', { PayMana: 'Cancel' }],
  ])('%s sends the answer once, even on a double click', async (name, expected) => {
    const { h, send, user, button } = mount()
    send(decision(22, pay('{2}')))
    await user.dblClick(button(name))
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 22, answer: expected }])
  })

  it.each([
    ['a double click', async (user: ReturnType<typeof mount>['user'], el: HTMLElement) => user.dblClick(el)],
    [
      'three quick clicks',
      async (user: ReturnType<typeof mount>['user'], el: HTMLElement) => user.tripleClick(el),
    ],
    [
      'Enter twice',
      async (user: ReturnType<typeof mount>['user'], el: HTMLElement) => {
        act(() => el.focus())
        await user.keyboard('{Enter}{Enter}')
      },
    ],
  ])('sends one answer for %s on Confirm', async (_how, gesture) => {
    const { h, card, send, user, button } = mount()
    send(decision(23, pay('{G}')))
    await user.click(card(greenLand))
    await gesture(user, button('Confirm payment'))
    expect(h.sentOf('Answer')).toHaveLength(1)
    expect(h.answeredIds()).toEqual([23])
  })

  it('sends nothing for a payment that does not cover the cost', async () => {
    const { h, card, send, user, button } = mount()
    send(decision(24, pay('{G}{G}', { sources: [source(greenLand, 'G'), source(blueLand, 'U')] })))
    await user.click(card(greenLand))
    await user.click(card(blueLand))
    await user.click(button('Confirm payment'))
    expect(h.sentOf('Answer')).toEqual([])
  })

  it('takes the prompt away when the connection is lost, since the decision goes with it', () => {
    const { h, send, container, marked } = mount()
    send(decision(25, pay('{G}')))
    expect(marked()).toBeGreaterThan(0)
    act(() => h.net.last.fail())
    expect(container.querySelector('[data-decision]')).toBeNull()
    expect(marked()).toBe(0)
  })

  it('clears the payment and the choice when the server replaces the decision, and marks the new options', async () => {
    const { card, send, user, container, button } = mount()
    send(decision(26, pay('{2}')))
    await user.click(card(greenLand))
    expect(card(greenLand)).toHaveAttribute('data-highlight', 'selected')
    expect(container.querySelectorAll('[data-slot="pay-chip"]')).toHaveLength(1)
    send(decision(27, pay('{U}')))
    expect(container.querySelectorAll('[data-slot="pay-chip"]')).toHaveLength(0)
    expect(card(greenLand)).toHaveAttribute('data-highlight', 'dimmed')
    expect(card(blueLand)).toHaveAttribute('data-highlight', 'legal')
    expect(button('Confirm payment')).toBeDisabled()
  })

  it('can be answered again when the server repeats it after a new snapshot', async () => {
    const { h, send, user, button } = mount()
    send(decision(28, pay('{2}')))
    await user.click(button('Pay automatically'))
    expect(h.answeredIds()).toEqual([28])
    act(() => h.deliver({ type: 'Snapshot', seq: 2, state: fixture.view }))
    send(decision(28, pay('{2}')))
    await user.click(button('Pay automatically'))
    expect(h.answeredIds()).toEqual([28, 28])
  })

  it('draws no prompt for the decision of another seat', () => {
    const { send, container, marked } = mount()
    send(decision(29, pay('{2}'), 1))
    expect(container.querySelector('[data-decision]')).toBeNull()
    expect(marked()).toBe(0)
  })
})
