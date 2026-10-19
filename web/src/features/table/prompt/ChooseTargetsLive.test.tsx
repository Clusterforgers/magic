/** Goal: over a real session, a target decision from the server draws its prompt, a click on a legal card or a button chooses a target, Confirm sends one Answer that echoes the decision id and holds the targets in the order chosen, Cancel sends Cancel for a spell, the table clears its marks as soon as it is answered, a decision the server replaces (or repeats after a resync) starts with no choices and the new options, a decision for another seat draws nothing, and a decision on a clock counts down in its own words and goes with the prompt at zero. */
import { act, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ObjectId, ServerMessage, Target, TargetOptions } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf } from '@/test-utils/combatTable'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'

const SEATS = 3
const fixture = buildFixture({ seats: SEATS })
const [m1, m2, m3] = creaturesOf(fixture, 0) as [ObjectId, ObjectId, ObjectId]
const spell = must(fixture.view.objects.find((o) => o.zone === 'Stack' && o.controller === 0))

const at = (id: ObjectId): Target => ({ Object: { id, counter: 0 } })

const targets = (
  id: number,
  legal: Target[],
  over: Partial<TargetOptions> = {},
  { player = 0, deadline }: { player?: number; deadline?: number } = {},
): ServerMessage => ({
  type: 'Decision',
  id,
  decision: {
    player,
    kind: {
      ChooseTargets: {
        source: spell.id,
        spec: 0,
        legal,
        min: 1,
        max: 2,
        can_cancel: true,
        wording: 'target creature you control or player',
        ...over,
      },
    },
  },
  ...(deadline === undefined ? {} : { deadline_ms: deadline }),
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
  const prompt = (): HTMLElement | null => table.container.querySelector('[data-decision="ChooseTargets"]')
  const listed = (name: string): HTMLElement =>
    within(screen.getByRole('group', { name: 'Legal targets' })).getByRole('button', { name })
  const confirm = (): HTMLElement => screen.getByRole('button', { name: 'Confirm targets' })
  return { h, ...table, card, send, marked, prompt, listed, confirm }
}

const LEGAL = [{ Player: 1 }, { Player: 2 }, at(m1), at(m2)]

describe('a target decision from the server', () => {
  it('draws the prompt, and Confirm sends one Answer with the decision id and the targets in the order chosen', async () => {
    const { h, card, send, user, marked, prompt, listed, confirm } = mount()
    send(targets(31, LEGAL))
    expect(prompt()).not.toBeNull()
    expect(card(m1)).toHaveAttribute('data-highlight', 'legal')
    expect(card(m3)).toHaveAttribute('data-highlight', 'dimmed')
    expect(confirm()).toBeDisabled()
    await user.click(card(m1))
    expect(card(m1)).toHaveAttribute('data-highlight', 'selected')
    await user.click(listed('Bo'))
    expect(confirm()).toBeEnabled()
    await user.click(confirm())
    expect(h.sentOf('Answer')).toEqual([
      { type: 'Answer', decision: 31, answer: { ChooseTargets: { Targets: [at(m1), { Player: 1 }] } } },
    ])
    expect(prompt()).toBeNull()
    expect(marked()).toBe(0)
    expect(h.state().pending).toBeNull()
  })

  it.each([
    [
      'a double click on Confirm targets',
      async (user: ReturnType<typeof mount>['user']) =>
        user.dblClick(screen.getByRole('button', { name: 'Confirm targets' })),
    ],
    [
      'Enter twice on Confirm targets',
      async (user: ReturnType<typeof mount>['user']) => {
        act(() => screen.getByRole('button', { name: 'Confirm targets' }).focus())
        await user.keyboard('{Enter}{Enter}')
      },
    ],
  ])('sends one answer for %s', async (_how, gesture) => {
    const { h, send, user, listed } = mount()
    send(targets(32, LEGAL))
    await user.click(listed('Cy'))
    await gesture(user)
    expect(h.sentOf('Answer')).toHaveLength(1)
    expect(h.answeredIds()).toEqual([32])
  })

  it('takes the spell back with Cancel the spell, which sends Cancel', async () => {
    const { h, send, user, prompt } = mount()
    send(targets(33, LEGAL))
    await user.click(screen.getByRole('button', { name: 'Cancel the spell' }))
    expect(h.sentOf('Answer')).toEqual([
      { type: 'Answer', decision: 33, answer: { ChooseTargets: 'Cancel' } },
    ])
    expect(prompt()).toBeNull()
  })

  it('has no Cancel for a triggered ability, and names it in the sentence', () => {
    const { send, prompt } = mount()
    send(targets(34, LEGAL, { can_cancel: false, triggered: true }))
    expect(screen.queryByRole('button', { name: /^Cancel/ })).toBeNull()
    expect(must(prompt()).querySelector('[data-slot="prompt-text"]')?.textContent).toBe(
      `${spell.name}'s ability: choose target creature you control or player.`,
    )
  })

  it('forgets the chosen targets when the server replaces the decision', async () => {
    const { send, card, user, confirm, listed } = mount()
    send(targets(35, LEGAL))
    await user.click(card(m1))
    await user.click(listed('Bo'))
    send(targets(36, [at(m2), at(m3)]))
    expect(card(m1)).toHaveAttribute('data-highlight', 'dimmed')
    expect(card(m2)).toHaveAttribute('data-highlight', 'legal')
    expect(card(m3)).toHaveAttribute('data-highlight', 'legal')
    expect(screen.queryByRole('list', { name: 'Your targets' })).toBeNull()
    const group = screen.getByRole('group', { name: 'Legal targets' })
    expect(within(group).queryAllByRole('button', { pressed: true })).toEqual([])
    expect(confirm()).toBeDisabled()
    expect(within(screen.getByRole('group', { name: 'Legal targets' })).getAllByRole('button')).toHaveLength(
      2,
    )
  })

  it('can be answered again when the server repeats it after a new snapshot', async () => {
    const { h, send, user, card, confirm } = mount()
    send(targets(37, LEGAL))
    await user.click(card(m1))
    await user.click(confirm())
    act(() => h.deliver({ type: 'Snapshot', seq: 2, state: fixture.view }))
    send(targets(37, LEGAL))
    await user.click(card(m2))
    await user.click(confirm())
    expect(h.answeredIds()).toEqual([37, 37])
  })

  it('draws no prompt for the target decision of another seat', () => {
    const { send, prompt, marked } = mount()
    send(targets(38, LEGAL, {}, { player: 1 }))
    expect(prompt()).toBeNull()
    expect(marked()).toBe(0)
  })

  it('counts down in the words of a target decision, and the prompt goes at zero when the server has answered', () => {
    const { h, send, prompt, container } = mount()
    send(targets(39, LEGAL, {}, { deadline: 10_000 }))
    const timer = (): string => container.querySelector('[role="timer"]')?.textContent ?? ''
    expect(timer()).toBe('Choosing no targets in 10 s')
    act(() => h.timers.advance(4_000))
    expect(timer()).toBe('Choosing no targets in 6 s')
    act(() => h.timers.advance(6_000))
    expect(prompt()).toBeNull()
  })
})
