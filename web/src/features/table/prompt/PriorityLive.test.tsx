/** Goal: over a real session, a decision from the server draws the prompt, a double click or a double Enter sends one Answer that echoes the decision id, the table clears its marks as soon as it is answered, and a decision the server replaces (or repeats after a resync) starts with no selection and the new options. */
import { act, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectId, PriorityOptions, ServerMessage } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'

const fixture = buildFixture({ seats: 2 })
const hand = must(fixture.view.players[0]?.hand.cards)
const [land, otherLand, ...spells] = hand as [ObjectId, ObjectId, ...ObjectId[]]
const rock = must(
  fixture.view.objects.find(
    (object) =>
      object.controller === 0 &&
      object.zone === 'Battlefield' &&
      object.permanent?.tapped === false &&
      !object.types.includes('Land') &&
      object.abilities.length > 0,
  ),
)

const options = (over: Partial<PriorityOptions> = {}): PriorityOptions => ({
  playable_lands: [],
  castable: [],
  mana_abilities: [],
  can_pass: true,
  ...over,
})
const decision = (id: number, given: PriorityOptions): ServerMessage => ({
  type: 'Decision',
  id,
  decision: { player: 0, kind: { Priority: given } },
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
  return { h, ...table, card, send, marked }
}

describe('a decision from the server', () => {
  it('draws the prompt and marks the cards, and a land click sends one Answer with the decision id', async () => {
    const { h, card, send, user, marked, container } = mount()
    expect(container.querySelector('[data-decision]')).toBeNull()
    send(decision(11, options({ playable_lands: [land] })))
    expect(container.querySelector('[data-decision="Priority"]')).not.toBeNull()
    expect(card(land)).toHaveAttribute('data-highlight', 'legal')
    await user.click(card(land))
    expect(h.sentOf('Answer')).toEqual([
      { type: 'Answer', decision: 11, answer: { Priority: { PlayLand: land } } },
    ])
    expect(container.querySelector('[data-decision]')).toBeNull()
    expect(marked()).toBe(0)
    expect(h.state().pending).toBeNull()
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
  ])('sends one answer for %s on a legal card', async (_how, gesture) => {
    const { h, card, send, user } = mount()
    send(decision(12, options({ castable: [must(spells[0])] })))
    await gesture(user, card(must(spells[0])))
    expect(h.sentOf('Answer')).toHaveLength(1)
    expect(h.answeredIds()).toEqual([12])
  })

  it('sends one answer for a double click on Pass', async () => {
    const { h, send, user } = mount()
    send(decision(13, options()))
    await user.dblClick(screen.getByRole('button', { name: 'Pass priority' }))
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 13, answer: { Priority: 'Pass' } }])
  })

  it('takes the prompt away when the connection is lost, since the decision goes with it', () => {
    const { h, send, container, marked } = mount()
    send(decision(14, options({ playable_lands: [land] })))
    expect(marked()).toBeGreaterThan(0)
    act(() => h.net.last.fail())
    expect(container.querySelector('[data-decision]')).toBeNull()
    expect(marked()).toBe(0)
  })

  it('clears the mana choice when the server replaces the decision, and marks the new options', async () => {
    const { card, send, user, container } = mount()
    const two = [
      { source: { object: rock.id, ability: 0 }, produces: 'Colorless' as const },
      { source: { object: rock.id, ability: 1 }, produces: 'Red' as const },
    ]
    send(decision(15, options({ mana_abilities: two })))
    await user.click(card(rock.id))
    expect(card(rock.id)).toHaveAttribute('data-highlight', 'selected')
    expect(screen.getByRole('group', { name: `Mana from ${rock.name}` })).toBeInTheDocument()
    send(decision(16, options({ mana_abilities: two, playable_lands: [otherLand] })))
    expect(card(rock.id)).toHaveAttribute('data-highlight', 'legal')
    expect(card(otherLand)).toHaveAttribute('data-highlight', 'legal')
    expect(container.querySelector('[data-slot="mana-choice"]')).toBeNull()
  })

  it('can be answered again when the server repeats it after a new snapshot', async () => {
    const { h, card, send, user } = mount()
    send(decision(17, options({ playable_lands: [land] })))
    await user.click(card(land))
    expect(h.answeredIds()).toEqual([17])
    act(() => h.deliver({ type: 'Snapshot', seq: 2, state: fixture.view }))
    send(decision(17, options({ playable_lands: [land] })))
    await user.click(card(land))
    expect(h.answeredIds()).toEqual([17, 17])
  })

  it('draws no prompt for the decision of another seat', () => {
    const { send, container, marked } = mount()
    send({ type: 'Decision', id: 18, decision: { player: 1, kind: { Priority: options() } } })
    expect(container.querySelector('[data-decision]')).toBeNull()
    expect(marked()).toBe(0)
  })
})
