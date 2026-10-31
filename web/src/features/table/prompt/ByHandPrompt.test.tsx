// Goal: over a real session, the Priority prompt of a seat held at its by-hand window (its own spell or ability with clauses left to the table has resolved and it has not passed since) says to apply the named card's text by hand and then continue, from the first window after the resolution until the seat passes; any other prompt, a spell without clauses or with only activated and loyalty abilities left to the table, an opponent's spell and the seats that only wait read their usual words.
import { act, screen, waitFor } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { EventView, ObjectId, PlayerId, ServerMessage } from '@/protocol/generated'
import { batchOf, must, tableOf } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'
import { byHandText } from './priorityText'

const CLAUSE = { text: 'Flip a coin.', reason: 'random' } as const
const STANDING = { text: '{T}: Add {C}.', reason: 'by hand' } as const
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
function mount(
  viewer: PlayerId,
  controller: PlayerId,
  clauses: boolean | 'standing',
  holder: PlayerId = viewer,
) {
  const fixture = buildFixture({ seats: 2, viewer })
  const spell = must(
    fixture.view.objects.find((object) => object.zone === 'Stack' && object.controller === controller),
  )
  const objects = fixture.view.objects.map((object) =>
    object.id !== spell.id || clauses === false
      ? object
      : clauses === 'standing'
        ? { ...object, manual: [STANDING] }
        : { ...object, manual: [CLAUSE], manual_owed: true },
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
  it('names the card to apply by hand from its resolution until the seat passes', async () => {
    const { h, spell, send, events, prompt, container } = mount(0, 0, true)
    send(decision(1, 0))
    await waitFor(() => expect(prompt()).toMatch(/^You are responding to/))
    events(resolved(spell.id))
    expect(h.state().byHand).toBe(spell.name)
    send(decision(2, 0))
    await waitFor(() => expect(prompt()).toBe(byHandText(spell.name)))
    expect(container.querySelector('[data-decision="Priority"]')).not.toBeNull()
    expect(prompt()).toBe(`Apply ${spell.name}'s text by hand, then continue.`)

    // The same words at every window of the hold, until the seat passes.
    send(decision(3, 0))
    await waitFor(() => expect(prompt()).toBe(byHandText(spell.name)))
    events(passed(0))
    expect(h.state().byHand).toBeNull()
    send(decision(4, 0))
    await waitFor(() => expect(prompt()).toMatch(/^You (have priority|are responding)/))
    expect(prompt()).not.toBe(byHandText(spell.name))
  })

  it.each<[string, PlayerId, PlayerId, boolean | 'standing']>([
    ['its own spell has no clauses', 0, 0, false],
    ['its own spell has only activated or loyalty abilities left to the table', 0, 0, 'standing'],
    ["the spell is an opponent's, with clauses", 0, 1, true],
  ])('keeps the usual words when %s', async (_what, viewer, controller, clauses) => {
    const { h, spell, send, events, prompt } = mount(viewer, controller, clauses)
    events(resolved(spell.id))
    expect(h.state().byHand).toBeNull()
    send(decision(1, viewer))
    await waitFor(() => expect(prompt()).toMatch(/^You (have priority|are responding)/))
    expect(prompt()).not.toContain('by hand')
  })

  it('shows another seat that waits for the holder its usual waiting words, never the by-hand ones', async () => {
    const { h, spell, events, container } = mount(1, 0, true, 0)
    events(resolved(spell.id))
    expect(h.state().byHand).toBeNull()
    expect(container.querySelector('[data-decision]')).toBeNull()
    expect(await screen.findByText('Waiting for Alice')).toBeInTheDocument()
    expect(container.textContent).not.toContain(byHandText(spell.name))
  })

  it('keeps the hold over a resync whose history holds neither the resolution nor a pass', () => {
    const { h, spell, events, send } = mount(0, 0, true)
    events(resolved(spell.id))
    const view = must(h.state().view)
    send({ type: 'Snapshot', seq: 5, state: view, log: [] })
    expect(h.state().byHand).toBe(spell.name)
    send({ type: 'Snapshot', seq: 6, state: view, log: [passed(0)] })
    expect(h.state().byHand).toBeNull()
  })

  it('takes the hold from the history of a snapshot, as after a reload', () => {
    const fixture = buildFixture({ seats: 2 })
    const spell = must(fixture.view.objects.find((o) => o.zone === 'Stack' && o.controller === 0))
    const objects = fixture.view.objects.map((o) =>
      o.id === spell.id ? { ...o, manual: [CLAUSE], manual_owed: true } : o,
    )
    const h = startSession()
    h.join(0, 2)
    act(() =>
      h.deliver({ type: 'Snapshot', seq: 9, state: { ...fixture.view, objects }, log: [resolved(spell.id)] }),
    )
    expect(h.state().byHand).toBe(spell.name)
  })
})
