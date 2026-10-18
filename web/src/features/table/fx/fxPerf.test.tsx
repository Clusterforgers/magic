// Goal: with the decoration layer on at the worst case of 12 seats with 40 permanents each (480 cards), a batch and the end of its effects redraw only what they decorate, never a battlefield row for damage, life or an attack and just the one row a death leaves a ghost in, never the whole table, and an unrelated batch during an effect still redraws only what it changes.
import { act } from '@testing-library/react'
import type { ComponentProps } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { EventView, ObjectView, StateView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import { batchFrom } from '@/test-utils/miniTable'
import type { Draft } from '@/test-utils/miniTable'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from '../dev/devGame'
import { buildFixture } from '../dev/fixture'
import { FX_MS } from './model'

// The same probe as `perf.test.tsx`: a render of a card face is a render of a card, a render of a `CardRow` one of
// the component that owns it. A ghost draws a face too, which is how its render shows up.
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
const BATTLEFIELD_ROW = /'s (lands|creatures|other permanents)$/
const battlefieldRows = (): string[] => [...probe.rows.keys()].filter((label) => BATTLEFIELD_ROW.test(label))
const reset = (): void => {
  probe.faces.clear()
  probe.rows.clear()
}
const advance = (ms: number): void => {
  act(() => {
    vi.advanceTimersByTime(ms)
  })
}

/** The whole table in grid mode on a real session fed a snapshot, with the decoration layer on. */
function mount() {
  const h = startSession()
  h.join(0, SEATS)
  act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
  renderLiveTable(h.session, fixtureLookup(fixture), { mode: 'grid', effects: true })
  const view = (): StateView => must(h.state().view)
  const send = (change: (draft: Draft) => void, events: EventView[]): void => {
    const from = must(h.state().seq)
    act(() => h.deliver({ type: 'Events', ...batchFrom(view(), change, events, from) }))
  }
  return { h, view, send }
}

const creatureOf = (view: StateView, seat: number, nth = 0): ObjectView =>
  must(
    view.objects.filter(
      (object) =>
        object.controller === seat &&
        object.zone === 'Battlefield' &&
        object.types.includes('Creature') &&
        object.permanent?.damage === 0,
    )[nth],
  )

const untappedLand = (view: StateView, seat: number): ObjectView =>
  must(
    view.objects.find(
      (object) =>
        object.controller === seat && object.types.includes('Land') && object.permanent?.tapped === false,
    ),
  )

const hurt = (id: number, damage: number) => (draft: Draft) =>
  draft.edit(id, (object) => {
    must(object.permanent).damage = damage
  })
const dealt = (source: number, target: number, amount: number): EventView[] => [
  { Public: { DamageDealt: { source, target: { Object: target }, amount } } },
]
const tapped = (id: number): EventView[] => [{ Public: { PermanentTapped: { object: id } } }]

beforeEach(() => {
  vi.useFakeTimers()
  reset()
})

afterEach(() => {
  vi.useRealTimers()
})

describe('a table of 12 seats with 40 permanents each, decorations on', () => {
  it('redraws a damaged card once when its flash starts and once when it ends, and no row', () => {
    const { send, view } = mount()
    const victim = creatureOf(view(), 7)
    reset()
    send(hurt(victim.id, 1), dealt(creatureOf(view(), 3).id, victim.id, 1))
    expect([...probe.faces]).toEqual([[victim.id, 1]])
    expect(battlefieldRows()).toEqual([])
    reset()
    advance(FX_MS.damaged)
    expect([...probe.faces]).toEqual([[victim.id, 1]])
    expect(battlefieldRows()).toEqual([])
    expect(vi.getTimerCount()).toBe(0)
  })

  it('redraws only the five attackers, once at the start and once at the end', () => {
    const { send, view } = mount()
    const attackers = [0, 1, 2, 3, 4].map((nth) => creatureOf(view(), 2, nth))
    reset()
    send(
      (draft) => {
        // They join the attack the fixture already has under way, whose cards keep their roles.
        draft.view.combat = {
          ...draft.view.combat,
          attackers: [
            ...draft.view.combat.attackers,
            ...attackers.map((object) => ({
              object: object.id,
              target: { Player: 0 },
              blocked: false,
              blockers: [],
            })),
          ],
        }
      },
      [
        {
          Public: {
            AttackersDeclared: { player: 2, attackers: attackers.map((o) => [o.id, { Player: 0 }]) },
          },
        },
      ],
    )
    expect(new Set(probe.faces.keys())).toEqual(new Set(attackers.map((object) => object.id)))
    expect([...probe.faces.values()].every((count) => count === 1)).toBe(true)
    expect(battlefieldRows()).toEqual([])
    reset()
    advance(FX_MS.attacked)
    expect(new Set(probe.faces.keys())).toEqual(new Set(attackers.map((object) => object.id)))
    expect([...probe.faces.values()].every((count) => count === 1)).toBe(true)
    expect(battlefieldRows()).toEqual([])
  })

  it('redraws no card and no row for a life total that changes', () => {
    const { send } = mount()
    reset()
    send((draft) => draft.life(5, 30), [{ Public: { PriorityPassed: { player: 0 } } }])
    expect(probe.faces.size).toBe(0)
    expect(battlefieldRows()).toEqual([])
    advance(FX_MS.damaged)
    expect(probe.faces.size).toBe(0)
    expect(battlefieldRows()).toEqual([])
  })

  it('redraws the one row a creature dies in, once with its ghost and once without, and no other row', () => {
    const { send, view } = mount()
    const victim = creatureOf(view(), 4, 2)
    reset()
    send(
      (draft) => draft.die(victim.id),
      [
        ...dealt(creatureOf(view(), 3).id, victim.id, 9),
        { Public: { CreatureDied: { object: victim.id, cause: 'LethalDamage' } } },
      ],
    )
    expect(battlefieldRows()).toEqual(["Eli's creatures"])
    expect(probe.rows.get("Eli's creatures")).toBe(1)
    // The ghost draws the card once, and so does the graveyard it now tops; no other card is drawn.
    expect([...probe.faces.keys()]).toEqual([victim.id])
    reset()
    advance(FX_MS.died)
    expect(battlefieldRows()).toEqual(["Eli's creatures"])
    expect(probe.rows.get("Eli's creatures")).toBe(1)
    expect(probe.faces.size).toBe(0)
  })

  it('costs the same rows for a death with the layer off, plus nothing', () => {
    const h = startSession()
    h.join(0, SEATS)
    act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
    renderLiveTable(h.session, fixtureLookup(fixture), { mode: 'grid', effects: false })
    const view = must(h.state().view)
    const victim = creatureOf(view, 4, 2)
    reset()
    act(() =>
      h.deliver({
        type: 'Events',
        ...batchFrom(
          view,
          (draft) => draft.die(victim.id),
          [{ Public: { CreatureDied: { object: victim.id, cause: 'LethalDamage' } } }],
          must(h.state().seq),
        ),
      }),
    )
    expect(battlefieldRows()).toEqual(["Eli's creatures"])
    expect(probe.rows.get("Eli's creatures")).toBe(1)
    expect([...probe.faces.keys()]).toEqual([victim.id])
  })

  it('still redraws only what an unrelated batch changes while a decoration plays', () => {
    const { send, view } = mount()
    const victim = creatureOf(view(), 7)
    send(hurt(victim.id, 1), dealt(creatureOf(view(), 3).id, victim.id, 1))
    const land = untappedLand(view(), 9)
    reset()
    send(
      (draft) =>
        draft.edit(land.id, (object) => {
          must(object.permanent).tapped = true
        }),
      tapped(land.id),
    )
    expect([...probe.faces]).toEqual([[land.id, 1]])
    expect(battlefieldRows()).toEqual([])
  })

  it('starts one timer for a batch of one kind of effect, whatever the number of cards', () => {
    const { send, view } = mount()
    const victims = [0, 1, 2, 3, 4, 5].map((nth) => creatureOf(view(), 6, nth))
    send(
      (draft) => {
        for (const victim of victims) hurt(victim.id, 1)(draft)
      },
      victims.flatMap((victim) => dealt(creatureOf(view(), 3).id, victim.id, 1)),
    )
    expect(vi.getTimerCount()).toBe(1)
    advance(FX_MS.damaged)
    expect(vi.getTimerCount()).toBe(0)
  })
})
