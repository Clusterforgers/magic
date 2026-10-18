/** Goal: over a real session, an AssignDamage decision from the server draws the prompt with the default split and marks the blockers, one click (or a double click, or Enter twice) sends one Answer that echoes the decision id and names the blockers that get damage, the table clears its marks once it is answered, a decision the server replaces starts again from the new options, and a repeat of the decision after a resync can be answered again. */
import { act, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { DamageOptions, ObjectId, ServerMessage } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'

const fixture = buildFixture({ seats: 2 })
const creatures = (seat: number) =>
  fixture.view.objects.filter(
    (object) =>
      object.zone === 'Battlefield' && object.controller === seat && object.types.includes('Creature'),
  )
const attacker = must(creatures(0)[0])
const [b1, b2, b3] = creatures(1).map((object) => object.id) as [ObjectId, ObjectId, ObjectId]
const nameOf = (id: ObjectId): string => must(fixture.view.objects.find((object) => object.id === id)).name

const options = (power: number, blockers: ObjectId[]): DamageOptions => ({
  attacker: attacker.id,
  power,
  blockers,
})
const decision = (id: number, given: DamageOptions, player = 0): ServerMessage => ({
  type: 'Decision',
  id,
  decision: { player, kind: { AssignDamage: given } },
})

/** A live table of seat 0 that has been sent the fixture as a Snapshot. */
function mount() {
  const h = startSession()
  h.join(0, 2)
  act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
  const table = renderLiveTable(h.session, fixtureLookup(fixture))
  const send = (message: ServerMessage): void => act(() => h.deliver(message))
  const confirm = (): HTMLElement => screen.getByRole('button', { name: 'Confirm' })
  const amounts = (): number[] =>
    screen.getAllByRole<HTMLInputElement>('spinbutton').map((box) => Number(box.value))
  const marked = (): number => table.container.querySelectorAll('[data-highlight]').length
  return { h, ...table, send, confirm, amounts, marked }
}

describe('an AssignDamage decision from the server', () => {
  it('draws the prompt with the default split and marks the blockers', () => {
    const { send, amounts, container, marked, confirm } = mount()
    expect(container.querySelector('[data-decision]')).toBeNull()
    send(decision(21, options(5, [b1, b2])))
    expect(container.querySelector('[data-decision="AssignDamage"]')).not.toBeNull()
    expect(amounts()).toHaveLength(2)
    expect(amounts().reduce((sum, amount) => sum + amount, 0)).toBe(5)
    expect(confirm()).toBeEnabled()
    expect(marked()).toBe(2)
  })

  it('sends one Answer with the decision id on a click, and takes the prompt and the marks away', async () => {
    const { h, send, user, confirm, amounts, container, marked } = mount()
    send(decision(22, options(5, [b1, b2, b3])))
    const split = amounts()
    await user.click(confirm())
    const shares = [b1, b2, b3].flatMap((id, index): [ObjectId, number][] => {
      const amount = split[index] ?? 0
      return amount > 0 ? [[id, amount]] : []
    })
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 22, answer: { AssignDamage: shares } }])
    expect(container.querySelector('[data-decision]')).toBeNull()
    expect(marked()).toBe(0)
    expect(h.state().pending).toBeNull()
  })

  it('sends the split that was edited, not the default', async () => {
    const { h, send, user, confirm } = mount()
    send(decision(23, options(6, [b1, b2])))
    const less = screen.getByRole('button', { name: `Less damage to ${nameOf(b2)}` })
    const more = screen.getByRole('button', { name: `More damage to ${nameOf(b1)}` })
    // Whatever the default is, empty the second blocker into the first.
    for (let times = 0; times < 6; times += 1) await user.click(less)
    for (let times = 0; times < 6; times += 1) await user.click(more)
    await user.click(confirm())
    expect(h.sentOf('Answer')).toEqual([
      { type: 'Answer', decision: 23, answer: { AssignDamage: [[b1, 6]] } },
    ])
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
    const { h, send, user, confirm } = mount()
    send(decision(24, options(6, [b1, b2])))
    await gesture(user, confirm())
    expect(h.sentOf('Answer')).toHaveLength(1)
    expect(h.answeredIds()).toEqual([24])
  })

  it('sends one answer for Enter in a number box', async () => {
    const { h, send, user } = mount()
    send(decision(25, options(6, [b1, b2])))
    const box = screen.getByRole('spinbutton', { name: `Damage to ${nameOf(b1)}` })
    act(() => box.focus())
    await user.keyboard('{Enter}')
    expect(h.answeredIds()).toEqual([25])
  })

  it('starts again from the new options when the server replaces the decision', async () => {
    const { send, user, amounts, container, marked } = mount()
    send(decision(26, options(4, [b1, b2])))
    await user.click(screen.getByRole('button', { name: `Less damage to ${nameOf(b2)}` }))
    expect(container.querySelector('[data-slot="damage-row"][data-active="true"]')).not.toBeNull()
    send(decision(27, options(7, [b1, b2, b3])))
    expect(amounts()).toHaveLength(3)
    expect(amounts().reduce((sum, amount) => sum + amount, 0)).toBe(7)
    expect(container.querySelector('[data-slot="damage-row"][data-active="true"]')).toBeNull()
    expect(marked()).toBe(3)
  })

  it('can be answered again when the server repeats it after a new snapshot', async () => {
    const { h, send, user, confirm } = mount()
    send(decision(28, options(3, [b1, b2])))
    await user.click(confirm())
    expect(h.answeredIds()).toEqual([28])
    act(() => h.deliver({ type: 'Snapshot', seq: 2, state: fixture.view }))
    send(decision(28, options(3, [b1, b2])))
    await user.click(confirm())
    expect(h.answeredIds()).toEqual([28, 28])
  })

  it('takes the prompt away when the connection is lost, since the decision goes with it', () => {
    const { h, send, container, marked } = mount()
    send(decision(29, options(3, [b1, b2])))
    expect(marked()).toBeGreaterThan(0)
    act(() => h.net.last.fail())
    expect(container.querySelector('[data-decision]')).toBeNull()
    expect(marked()).toBe(0)
  })

  it('draws no prompt for the decision of another seat', () => {
    const { send, container, marked } = mount()
    send(decision(30, options(3, [b1, b2]), 1))
    expect(container.querySelector('[data-decision]')).toBeNull()
    expect(marked()).toBe(0)
  })
})
