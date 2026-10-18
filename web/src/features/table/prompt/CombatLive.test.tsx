/** Goal: over a real session, an attack or block decision from the server draws its prompt, a double click or a double Enter sends one Answer that echoes the decision id and holds the pairs chosen, the table clears its marks and notes as soon as it is answered, and a decision the server replaces (or repeats after a resync) starts with no choices and the new options. */
import { act, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { AttackTarget, ObjectId, ServerMessage } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf } from '@/test-utils/combatTable'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'

const SEATS = 4
const fixture = buildFixture({ seats: SEATS })
const mine = creaturesOf(fixture, 0)
const theirs = creaturesOf(fixture, 1)
const [m1, m2, m3] = mine as [ObjectId, ObjectId, ObjectId]
const [t1, t2] = theirs as [ObjectId, ObjectId]
const defenders: AttackTarget[] = [{ Player: 1 }, { Player: 2 }, { Player: 3 }]

const attackers = (id: number, ids: ObjectId[]): ServerMessage => ({
  type: 'Decision',
  id,
  decision: {
    player: 0,
    kind: { DeclareAttackers: { attackers: ids.map((object) => ({ object, defenders })) } },
  },
})
const blockers = (id: number, ids: ObjectId[], canBlock: ObjectId[]): ServerMessage => ({
  type: 'Decision',
  id,
  decision: {
    player: 0,
    kind: { DeclareBlockers: { blockers: ids.map((object) => ({ object, can_block: canBlock })) } },
  },
})

/** A live table of seat 0 that has been sent the fixture as a Snapshot. */
function mount() {
  const h = startSession()
  h.join(0, SEATS)
  act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
  const table = renderLiveTable(h.session, fixtureLookup(fixture))
  const card = (id: ObjectId): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`))
  const send = (message: ServerMessage): void => act(() => h.deliver(message))
  const marked = (): number => table.container.querySelectorAll('[data-highlight]').length
  const noted = (): number => table.container.querySelectorAll('[data-slot="card-note"]').length
  return { h, ...table, card, send, marked, noted }
}

describe('an attack decision from the server', () => {
  it('draws the prompt, and Confirm attack sends one Answer with the decision id and the chosen defenders', async () => {
    const { h, card, send, user, marked, noted, container } = mount()
    send(attackers(21, [m1, m2, m3]))
    expect(container.querySelector('[data-decision="DeclareAttackers"]')).not.toBeNull()
    expect(card(m1)).toHaveAttribute('data-highlight', 'legal')
    await user.click(card(m1))
    await user.selectOptions(screen.getByRole('combobox', { name: 'Attack at' }), 'player:3')
    await user.click(card(m2))
    expect(noted()).toBe(2)
    await user.click(screen.getByRole('button', { name: 'Confirm attack' }))
    expect(h.sentOf('Answer')).toEqual([
      {
        type: 'Answer',
        decision: 21,
        answer: {
          DeclareAttackers: [
            [m1, { Player: 1 }],
            [m2, { Player: 3 }],
          ],
        },
      },
    ])
    expect(container.querySelector('[data-decision]')).toBeNull()
    expect(marked()).toBe(0)
    expect(noted()).toBe(0)
    expect(h.state().pending).toBeNull()
  })

  it.each([
    [
      'a double click on Confirm attack',
      async (user: ReturnType<typeof mount>['user']) =>
        user.dblClick(screen.getByRole('button', { name: 'Confirm attack' })),
    ],
    [
      'a double click on No attack',
      async (user: ReturnType<typeof mount>['user']) =>
        user.dblClick(screen.getByRole('button', { name: 'No attack' })),
    ],
    [
      'Enter twice on Confirm attack',
      async (user: ReturnType<typeof mount>['user']) => {
        act(() => screen.getByRole('button', { name: 'Confirm attack' }).focus())
        await user.keyboard('{Enter}{Enter}')
      },
    ],
  ])('sends one answer for %s', async (_how, gesture) => {
    const { h, card, send, user } = mount()
    send(attackers(22, [m1]))
    await user.click(card(m1))
    await gesture(user)
    expect(h.sentOf('Answer')).toHaveLength(1)
    expect(h.answeredIds()).toEqual([22])
  })

  it('answers an empty attack with No attack', async () => {
    const { h, send, user } = mount()
    send(attackers(23, [m1]))
    await user.click(screen.getByRole('button', { name: 'No attack' }))
    expect(h.sentOf('Answer')).toEqual([{ type: 'Answer', decision: 23, answer: { DeclareAttackers: [] } }])
  })

  it('forgets the chosen attackers and the aim when the server replaces the decision', async () => {
    const { card, send, user, noted } = mount()
    send(attackers(24, [m1, m2]))
    await user.click(card(m1))
    await user.selectOptions(screen.getByRole('combobox', { name: 'Attack at' }), 'player:2')
    send(attackers(25, [m2, m3]))
    expect(card(m1)).toHaveAttribute('data-highlight', 'dimmed')
    expect(card(m3)).toHaveAttribute('data-highlight', 'legal')
    expect(noted()).toBe(0)
    expect(screen.getByRole('combobox', { name: 'Attack at' })).toHaveValue('player:1')
    expect(screen.getByRole('button', { name: 'Confirm attack' })).toBeDisabled()
  })

  it('can be answered again when the server repeats it after a new snapshot', async () => {
    const { h, card, send, user } = mount()
    send(attackers(26, [m1]))
    await user.click(card(m1))
    await user.click(screen.getByRole('button', { name: 'Confirm attack' }))
    act(() => h.deliver({ type: 'Snapshot', seq: 2, state: fixture.view }))
    send(attackers(26, [m1]))
    await user.click(card(m1))
    await user.click(screen.getByRole('button', { name: 'Confirm attack' }))
    expect(h.answeredIds()).toEqual([26, 26])
  })

  it('draws no prompt for the attack decision of another seat', () => {
    const { send, container, marked } = mount()
    send({
      type: 'Decision',
      id: 27,
      decision: { player: 1, kind: { DeclareAttackers: { attackers: [] } } },
    })
    expect(container.querySelector('[data-decision]')).toBeNull()
    expect(marked()).toBe(0)
  })
})

describe('a block decision from the server', () => {
  it('draws the prompt, and Confirm blocks sends one Answer with the decision id and the pairs', async () => {
    const { h, card, send, user, marked, noted, container } = mount()
    send(blockers(31, [m1, m2], [t1, t2]))
    expect(container.querySelector('[data-decision="DeclareBlockers"]')).not.toBeNull()
    await user.click(card(m1))
    await user.click(card(t2))
    await user.click(card(m2))
    await user.click(card(t2))
    expect(noted()).toBe(3)
    await user.click(screen.getByRole('button', { name: 'Confirm blocks' }))
    expect(h.sentOf('Answer')).toEqual([
      {
        type: 'Answer',
        decision: 31,
        answer: {
          DeclareBlockers: [
            [m1, t2],
            [m2, t2],
          ],
        },
      },
    ])
    expect(container.querySelector('[data-decision]')).toBeNull()
    expect(marked()).toBe(0)
    expect(noted()).toBe(0)
  })

  it.each([
    [
      'a double click on Confirm blocks',
      async (user: ReturnType<typeof mount>['user']) =>
        user.dblClick(screen.getByRole('button', { name: 'Confirm blocks' })),
    ],
    [
      'a double click on No blocks',
      async (user: ReturnType<typeof mount>['user']) =>
        user.dblClick(screen.getByRole('button', { name: 'No blocks' })),
    ],
    [
      'Enter twice on Confirm blocks',
      async (user: ReturnType<typeof mount>['user']) => {
        act(() => screen.getByRole('button', { name: 'Confirm blocks' }).focus())
        await user.keyboard('{Enter}{Enter}')
      },
    ],
  ])('sends one answer for %s', async (_how, gesture) => {
    const { h, card, send, user } = mount()
    send(blockers(32, [m1], [t1]))
    await user.click(card(m1))
    await user.click(card(t1))
    await gesture(user)
    expect(h.sentOf('Answer')).toHaveLength(1)
    expect(h.answeredIds()).toEqual([32])
  })

  it('forgets the pairs and the chosen blocker when the server replaces the decision', async () => {
    const { card, send, user, noted } = mount()
    send(blockers(33, [m1, m2], [t1, t2]))
    await user.click(card(m1))
    await user.click(card(t1))
    await user.click(card(m2))
    send(blockers(34, [m2, m3], [t2]))
    expect(noted()).toBe(0)
    expect(card(m1)).toHaveAttribute('data-highlight', 'dimmed')
    expect(card(m3)).toHaveAttribute('data-highlight', 'legal')
    expect(screen.queryByRole('group', { name: /may block$/ })).toBeNull()
    expect(screen.queryByRole('list', { name: 'Your blocks' })).toBeNull()
  })

  it('takes the prompt away when the connection is lost, since the decision goes with it', () => {
    const { h, send, container, marked } = mount()
    send(blockers(35, [m1], [t1]))
    expect(marked()).toBeGreaterThan(0)
    act(() => h.net.last.fail())
    expect(container.querySelector('[data-decision]')).toBeNull()
    expect(marked()).toBe(0)
  })
})
