// Goal: over a real session, the Priority prompt of a seat held at its by-hand window (its own spell or ability with clauses left to the table has resolved and it has not passed since) says to apply the card's text by hand and then pass, from the first window after the resolution until the seat passes; any other prompt, a spell without clauses, an opponent's spell and the seats that only wait read their usual words.
import { act, screen, waitFor } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { EventView, ObjectId, PlayerId, ServerMessage } from '@/protocol/generated'
import { batchOf, must, tableOf } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'
import { BY_HAND_TEXT } from './priorityText'

const CLAUSE = { text: 'Flip a coin.', reason: 'random' } as const
const OPTIONS = { playable_lands: [], castable: [], mana_abilities: [], can_act: false, can_pass: true }

const decision = (id: number, player: PlayerId): ServerMessage => ({
  type: 'Decision',
  id,
  decision: { player, kind: { Priority: OPTIONS } },
})
const resolved = (object: ObjectId): EventView => ({ Public: { SpellResolved: { object } } })
const passed = (player: PlayerId): EventView => ({ Public: { PriorityPassed: { player } } })

/**
 * A live table of `viewer` over the fixture whose spell of `controller` on the stack holds clauses (or none), with
 * `holder` holding priority; the spell is the one the events resolve.
 */
function mount(viewer: PlayerId, controller: PlayerId, clauses: boolean, holder: PlayerId = viewer) {
  const fixture = buildFixture({ seats: 2, viewer })
  const spell = must(
    fixture.view.objects.find((object) => object.zone === 'Stack' && object.controller === controller),
  )
  const objects = fixture.view.objects.map((object) =>
    object.id === spell.id && clauses ? { ...object, manual: [CLAUSE] } : object,
  )
  const view = { ...fixture.view, objects, priority: { holder, passed: [] } }
  const h = startSession()
  h.join(viewer, 2)
  act(() => h.deliver({ type: 'Snapshot', seq: 1, state: view }))
  const table = renderLiveTable(h.session, fixtureLookup(fixture))
  let seq = 1
  const send = (...messages: ServerMessage[]): void => act(() => h.deliver(...messages))
  /** The server's batch of `events`; one that resolves the spell takes it off the stack. */
  const events = (...list: EventView[]): void => {
    const stack = view.stack.cards?.filter((id) => id !== spell.id) ?? []
    const batch = batchOf(view, {
      from_seq: seq,
      events: list,
      table: { ...tableOf(view), stack: { count: stack.length, cards: stack } },
    })
    seq += list.length
    send({ type: 'Events', ...batch })
  }
  const prompt = (): string | null =>
    table.container.querySelector('[data-slot="prompt-text"]')?.textContent ?? null
  return { h, spell, send, events, prompt, ...table }
}

describe('the Priority prompt at the by-hand window', () => {
  it('says to apply the card by hand from its resolution until the seat passes', async () => {
    const { h, spell, send, events, prompt, container } = mount(0, 0, true)
    send(decision(1, 0))
    await waitFor(() => expect(prompt()).toMatch(/^You are responding to/))
    events(resolved(spell.id))
    expect(h.state().byHand).toBe(true)
    send(decision(2, 0))
    await waitFor(() => expect(prompt()).toBe(BY_HAND_TEXT))
    expect(container.querySelector('[data-decision="Priority"]')).not.toBeNull()
    expect(prompt()).toBe("Apply the card's text by hand, then pass.")

    // The same words at every window of the hold, until the seat passes.
    send(decision(3, 0))
    await waitFor(() => expect(prompt()).toBe(BY_HAND_TEXT))
    events(passed(0))
    expect(h.state().byHand).toBe(false)
    send(decision(4, 0))
    await waitFor(() => expect(prompt()).toMatch(/^You (have priority|are responding)/))
    expect(prompt()).not.toBe(BY_HAND_TEXT)
  })

  it.each<[string, PlayerId, PlayerId, boolean]>([
    ['its own spell has no clauses', 0, 0, false],
    ["the spell is an opponent's, with clauses", 0, 1, true],
  ])('keeps the usual words when %s', async (_what, viewer, controller, clauses) => {
    const { h, spell, send, events, prompt } = mount(viewer, controller, clauses)
    events(resolved(spell.id))
    expect(h.state().byHand).toBe(false)
    send(decision(1, viewer))
    await waitFor(() => expect(prompt()).toMatch(/^You (have priority|are responding)/))
    expect(prompt()).not.toBe(BY_HAND_TEXT)
  })

  it('shows another seat that waits for the holder its usual waiting words, never the by-hand ones', async () => {
    const { h, spell, events, container } = mount(1, 0, true, 0)
    events(resolved(spell.id))
    expect(h.state().byHand).toBe(false)
    expect(container.querySelector('[data-decision]')).toBeNull()
    expect(await screen.findByText('Waiting for Alice')).toBeInTheDocument()
    expect(container.textContent).not.toContain(BY_HAND_TEXT)
  })

  it('keeps the hold over a resync whose history holds neither the resolution nor a pass', () => {
    const { h, spell, events, send } = mount(0, 0, true)
    events(resolved(spell.id))
    const view = must(h.state().view)
    send({ type: 'Snapshot', seq: 5, state: view, log: [] })
    expect(h.state().byHand).toBe(true)
    send({ type: 'Snapshot', seq: 6, state: view, log: [passed(0)] })
    expect(h.state().byHand).toBe(false)
  })

  it('takes the hold from the history of a snapshot, as after a reload', () => {
    const fixture = buildFixture({ seats: 2 })
    const spell = must(fixture.view.objects.find((o) => o.zone === 'Stack' && o.controller === 0))
    const objects = fixture.view.objects.map((o) => (o.id === spell.id ? { ...o, manual: [CLAUSE] } : o))
    const h = startSession()
    h.join(0, 2)
    act(() =>
      h.deliver({ type: 'Snapshot', seq: 9, state: { ...fixture.view, objects }, log: [resolved(spell.id)] }),
    )
    expect(h.state().byHand).toBe(true)
  })
})
