// Goal: at the worst case of 12 seats with 40 permanents each (480 cards on screen) an attack or block decision arriving, a creature being chosen, attacking with all, a blocker being paired and the decision being answered redraw only the cards whose mark or note changed (the viewer's own cards, the chosen creature, the attackers it may block) and no battlefield row, so combat prompts cost no more than the Priority prompt.
import { act, screen } from '@testing-library/react'
import type { ComponentProps } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { ObjectId, ServerMessage } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, ownOf } from '@/test-utils/combatTable'
import { startSession } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'

// Every render of a card face is one render of a card, every render of a `CardRow` one of its owner.
const probe = vi.hoisted(() => ({ faces: new Map<number, number>(), rows: new Map<string, number>() }))

vi.mock('../CardFace', async (importOriginal) => {
  const original = await importOriginal<typeof import('../CardFace')>()
  const { createElement } = await import('react')
  return {
    ...original,
    CardFace(props: ComponentProps<typeof original.CardFace>) {
      probe.faces.set(props.object.id, (probe.faces.get(props.object.id) ?? 0) + 1)
      return createElement(original.CardFace, props)
    },
  }
})

vi.mock('../CardRow', async (importOriginal) => {
  const original = await importOriginal<typeof import('../CardRow')>()
  const { createElement } = await import('react')
  return {
    ...original,
    CardRow(props: ComponentProps<typeof original.CardRow>) {
      probe.rows.set(props.label, (probe.rows.get(props.label) ?? 0) + 1)
      return createElement(original.CardRow, props)
    },
  }
})

const SEATS = 12
const fixture = buildFixture({ seats: SEATS })
const own = new Set(ownOf(fixture))
const mine = creaturesOf(fixture, 0)
const theirs = creaturesOf(fixture, 1)
const [m1, m2, m3] = mine as [ObjectId, ObjectId, ObjectId]
const [t1, t2, t3] = theirs as [ObjectId, ObjectId, ObjectId]

const reset = (): void => {
  probe.faces.clear()
  probe.rows.clear()
}
const battlefieldRows = (): string[] =>
  [...probe.rows.keys()].filter((label) => /'s (lands|creatures|other permanents)$/.test(label))

const attack = (id: number): ServerMessage => ({
  type: 'Decision',
  id,
  decision: {
    player: 0,
    kind: {
      DeclareAttackers: {
        attackers: [m1, m2, m3].map((object) => ({ object, defenders: [{ Player: 1 }, { Player: 2 }] })),
      },
    },
  },
})
const block = (id: number): ServerMessage => ({
  type: 'Decision',
  id,
  decision: {
    player: 0,
    kind: { DeclareBlockers: { blockers: [m1, m2].map((object) => ({ object, can_block: [t1, t2] })) } },
  },
})

function mount() {
  const h = startSession()
  h.join(0, SEATS)
  act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
  const table = renderLiveTable(h.session, fixtureLookup(fixture), { mode: 'grid' })
  const card = (id: ObjectId): HTMLElement =>
    must(table.container.querySelector<HTMLElement>(`button[data-object-id="${id}"]`))
  return { h, ...table, card, send: (message: ServerMessage) => act(() => h.deliver(message)) }
}

describe('an attack decision at a table of 12 seats', () => {
  beforeEach(reset)

  it('redraws each of the viewer cards once when it arrives, and no other card and no row', () => {
    const { send } = mount()
    reset()
    send(attack(5))
    expect(new Set(probe.faces.keys())).toEqual(own)
    for (const count of probe.faces.values()) expect(count).toBe(1)
    expect(battlefieldRows()).toEqual([])
  })

  it('redraws only the chosen creature when it is chosen, and no row', async () => {
    const { send, card, user } = mount()
    send(attack(5))
    reset()
    await user.click(card(m1))
    expect([...probe.faces.keys()]).toEqual([m1])
    expect(battlefieldRows()).toEqual([])
  })

  it('redraws only the creatures that attacking with all adds', async () => {
    const { send, user } = mount()
    send(attack(5))
    reset()
    await user.click(screen.getByRole('button', { name: 'Attack with all' }))
    expect(new Set(probe.faces.keys())).toEqual(new Set([m1, m2, m3]))
    expect(battlefieldRows()).toEqual([])
  })

  it('redraws only the viewer cards when it is answered or replaced', () => {
    const { send, h } = mount()
    send(attack(5))
    reset()
    send(attack(6))
    for (const id of probe.faces.keys()) expect(own.has(id)).toBe(true)
    reset()
    act(() => {
      h.session.answer({ DeclareAttackers: [] })
    })
    expect(new Set(probe.faces.keys())).toEqual(own)
    expect(battlefieldRows()).toEqual([])
  })
})

describe('a block decision at a table of 12 seats', () => {
  beforeEach(reset)

  it('redraws each of the viewer cards once when it arrives, and no other card and no row', () => {
    const { send } = mount()
    reset()
    send(block(5))
    expect(new Set(probe.faces.keys())).toEqual(own)
    for (const count of probe.faces.values()) expect(count).toBe(1)
    expect(battlefieldRows()).toEqual([])
  })

  it('redraws only the blocker and the attackers around it when a blocker is chosen, and no row', async () => {
    const { send, card, user } = mount()
    send(block(5))
    reset()
    await user.click(card(m1))
    const redrawn = new Set(probe.faces.keys())
    // The chosen blocker, the two attackers it may block, and the other attackers of the combat, now dimmed.
    expect(redrawn.has(m1)).toBe(true)
    expect(redrawn.has(t1) && redrawn.has(t2)).toBe(true)
    expect(redrawn.has(t3)).toBe(false)
    expect(redrawn.has(m3)).toBe(false)
    expect(redrawn.size).toBeLessThanOrEqual(1 + 2 + fixture.view.combat.attackers.length)
    expect(battlefieldRows()).toEqual([])
  })

  it('redraws only the blocker and the attacker when they are paired', async () => {
    const { send, card, user } = mount()
    send(block(5))
    await user.click(card(m1))
    reset()
    await user.click(card(t1))
    const redrawn = new Set(probe.faces.keys())
    expect(redrawn.has(m1) && redrawn.has(t1)).toBe(true)
    expect(redrawn.has(m2)).toBe(false)
    expect(battlefieldRows()).toEqual([])
  })
})
