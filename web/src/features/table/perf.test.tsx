// Goal: a batch redraws only what it changed, at the worst case of 12 seats with 40 permanents each (480 cards on screen): tapping one permanent renders that card and no battlefield row, a change to the turn renders no card, a change of controller renders that card and the two rows it leaves and joins; a decision arriving, being replaced or answered renders only the viewer's own marked cards (once each) and no row, and while one is on screen a tap still renders one card.
import { act } from '@testing-library/react'
import type { ComponentProps } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { applyBatch, indexById } from '@/play/view'
import type { EventBatch, ObjectView, PriorityOptions, ServerMessage, StateView } from '@/protocol/generated'
import { batchOf, clone, must, tableOf, withTapped } from '@/test-utils/batches'
import { startSession } from '@/test-utils/harness'
import type { Harness } from '@/test-utils/harness'
import { renderLiveTable } from '@/test-utils/tableHarness'
import { fixtureLookup } from './dev/devGame'
import { buildFixture } from './dev/fixture'
import { splitBattlefield } from './selectors'

// The probe sits on two seams: every render of a card face is one render of a card, and every render of a
// `CardRow` is one render of the component that owns it (a battlefield row, a hand, a command zone).
const probe = vi.hoisted(() => ({ faces: new Map<number, number>(), rows: new Map<string, number>() }))

vi.mock('./CardFace', async (importOriginal) => {
  const original = await importOriginal<typeof import('./CardFace')>()
  const { createElement } = await import('react')
  return {
    ...original,
    CardFace(props: ComponentProps<typeof original.CardFace>) {
      probe.faces.set(props.object.id, (probe.faces.get(props.object.id) ?? 0) + 1)
      return createElement(original.CardFace, props)
    },
  }
})

vi.mock('./CardRow', async (importOriginal) => {
  const original = await importOriginal<typeof import('./CardRow')>()
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

interface Mounted {
  h: Harness
  view: () => StateView
  /** Delivers a batch whose table is the current one, as the server would. */
  send(over: Partial<EventBatch>): void
}

/** The whole table in grid mode, 12 seats, on a real session fed a Snapshot of the fixture. */
function mount(): Mounted {
  const h = startSession()
  h.join(0, SEATS)
  act(() => h.deliver({ type: 'Snapshot', seq: 1, state: fixture.view }))
  renderLiveTable(h.session, fixtureLookup(fixture), { mode: 'grid' })
  const view = (): StateView => must(h.state().view)
  return {
    h,
    view,
    send(over) {
      const from = must(h.state().seq)
      act(() => h.deliver({ type: 'Events', ...batchOf(view(), { from_seq: from, ...over }) }))
    },
  }
}

const untappedLand = (view: StateView, seat: number): ObjectView =>
  must(
    view.objects.find(
      (object) =>
        object.controller === seat && object.types.includes('Land') && object.permanent?.tapped === false,
    ),
  )

describe('a table of 12 seats with 40 permanents each', () => {
  beforeEach(reset)

  it('draws every permanent once when it first appears, and has 480 on the battlefield', () => {
    const { view } = mount()
    const onBattlefield = must(view().battlefield.cards)
    expect(onBattlefield).toHaveLength(480)
    for (const id of onBattlefield) expect(probe.faces.get(id), `card ${id}`).toBe(1)
    expect(battlefieldRows()).toHaveLength(SEATS * 3)
    for (const label of battlefieldRows()) expect(probe.rows.get(label), label).toBe(1)
  })

  it('redraws only the tapped card when a batch taps one permanent, and no battlefield row', () => {
    const { send, view } = mount()
    const land = untappedLand(view(), 7)
    reset()
    send({
      events: [{ Public: { PermanentTapped: { object: land.id } } }],
      objects: [withTapped(land, true)],
    })
    expect([...probe.faces]).toEqual([[land.id, 1]])
    expect(battlefieldRows()).toEqual([])
    expect(must(view().objects.find((object) => object.id === land.id)?.permanent).tapped).toBe(true)
  })

  it('redraws exactly the cards that untapping lists and nothing around them', () => {
    const { send, view } = mount()
    const tapped = view().objects.filter(
      (object) => object.controller === 3 && object.permanent?.tapped === true,
    )
    expect(tapped.length).toBeGreaterThan(5)
    reset()
    send({
      events: tapped.map((object) => ({ Public: { PermanentUntapped: { object: object.id } } })),
      objects: tapped.map((object) => withTapped(object, false)),
    })
    expect(new Set(probe.faces.keys())).toEqual(new Set(tapped.map((object) => object.id)))
    for (const count of probe.faces.values()) expect(count).toBe(1)
    expect(battlefieldRows()).toEqual([])
  })

  it('redraws no card and no battlefield row for a batch that only moves the turn along', () => {
    const { send, view } = mount()
    const table = clone(tableOf(view()))
    table.turn = { ...table.turn, step: 'CombatDamage' }
    table.priority = { holder: 4, passed: [0, 1] }
    reset()
    send({ events: [{ Public: { StepBegan: { step: 'CombatDamage' } } }], table })
    expect(probe.faces.size).toBe(0)
    expect(battlefieldRows()).toEqual([])
  })

  it('redraws the card and the two rows it leaves and joins when its controller changes', () => {
    const { send, view } = mount()
    // Neither first in its row nor about to be first in the next, so no tab stop moves.
    const id = must(splitBattlefield(view(), indexById(view().objects)).get(3)?.creatures[4])
    const creature = must(view().objects.find((object) => object.id === id))
    reset()
    send({ events: [], objects: [{ ...clone(creature), controller: 4 }] })
    expect([...probe.faces]).toEqual([[id, 1]])
    expect(battlefieldRows().toSorted()).toEqual(["Dee's creatures", "Eli's creatures"])
  })

  it("also redraws the one card per row whose place as the row's tab stop changes", () => {
    const { send, view } = mount()
    const rows = splitBattlefield(view(), indexById(view().objects))
    const [first, second] = must(rows.get(3)?.creatures)
    const creature = must(view().objects.find((object) => object.id === first))
    reset()
    // The first creature of Dee's row, the row's tab stop, goes to Eli.
    send({ events: [], objects: [{ ...clone(creature), controller: 4 }] })
    expect(probe.faces.get(must(first))).toBe(1)
    expect(probe.faces.get(must(second))).toBe(1)
    expect(probe.faces.size).toBeLessThanOrEqual(3)
    expect([...probe.faces.values()].every((count) => count === 1)).toBe(true)
  })

  it('redraws one row and no other card when a permanent leaves', () => {
    const { send, view } = mount()
    const land = untappedLand(view(), 9)
    const table = clone(tableOf(view()))
    table.battlefield = {
      count: table.battlefield.count - 1,
      cards: must(table.battlefield.cards).filter((id) => id !== land.id),
    }
    reset()
    send({ events: [{ Public: { ObjectLeftGame: { object: land.id } } }], table, gone: [land.id] })
    expect(probe.faces.size).toBe(0)
    expect(battlefieldRows()).toEqual(["Jun's lands"])
  })

  it('costs one card render per change over a run of 40 single taps', () => {
    const { send, view } = mount()
    const lands = view()
      .objects.filter((object) => object.types.includes('Land') && object.permanent?.tapped === false)
      .slice(0, 40)
    reset()
    for (const land of lands) {
      send({
        events: [{ Public: { PermanentTapped: { object: land.id } } }],
        objects: [withTapped(land, true)],
      })
    }
    expect([...probe.faces.values()].reduce((sum, count) => sum + count, 0)).toBe(40)
    expect(probe.faces.size).toBe(40)
    expect(battlefieldRows()).toEqual([])
  })

  it('shares every untouched object, so the fold itself leaves the rest alone', () => {
    const { view } = mount()
    const before = view()
    const land = untappedLand(before, 2)
    const after = applyBatch(before, batchOf(before, { objects: [withTapped(land, true)] }))
    const changed = after.objects.filter((object, index) => object !== before.objects[index])
    expect(changed.map((object) => object.id)).toEqual([land.id])
  })
})

const priority = (id: number, options: Partial<PriorityOptions> = {}, player = 0): ServerMessage => ({
  type: 'Decision',
  id,
  decision: {
    player,
    kind: {
      Priority: {
        playable_lands: [],
        castable: [],
        mana_abilities: [],
        can_act: false,
        can_pass: true,
        ...options,
      },
    },
  },
})

describe('a decision at the same table', () => {
  beforeEach(reset)

  const handOf = (view: StateView): number[] => must(view.players[0]?.hand.cards)
  /** The viewer's own cards: the hand and the 40 permanents, which a Priority prompt marks. */
  const ownOf = (view: StateView): Set<number> =>
    new Set([
      ...handOf(view),
      ...view.objects
        .filter((object) => object.zone === 'Battlefield' && object.controller === 0)
        .map((object) => object.id),
    ])

  /** A decision whose legal cards are the first two cards of the hand and one untapped land. */
  function options(view: StateView): Partial<PriorityOptions> {
    const land = untappedLand(view, 0)
    const [first, second] = handOf(view)
    return {
      playable_lands: [must(first)],
      castable: [must(second)],
      mana_abilities: [{ source: { object: land.id, ability: 0 }, produces: 'Green' }],
    }
  }

  it('redraws each of the viewer cards once when it arrives, and no other card and no row', () => {
    const { h, view } = mount()
    const own = ownOf(view())
    expect(own.size).toBe(47)
    reset()
    act(() => h.deliver(priority(5, options(view()))))
    expect(new Set(probe.faces.keys())).toEqual(own)
    for (const count of probe.faces.values()) expect(count).toBe(1)
    expect(battlefieldRows()).toEqual([])
    expect([...probe.rows.keys()].filter((label) => !label.startsWith("Alice's"))).toEqual([])
  })

  it('still redraws only the cards that a batch taps while the prompt is on screen', () => {
    const { h, send, view } = mount()
    act(() => h.deliver(priority(5, options(view()))))
    const theirs = untappedLand(view(), 7)
    const mine = untappedLand(view(), 0)
    reset()
    send({
      events: [{ Public: { PermanentTapped: { object: theirs.id } } }],
      objects: [withTapped(theirs, true)],
    })
    send({
      events: [{ Public: { PermanentTapped: { object: mine.id } } }],
      objects: [withTapped(mine, true)],
    })
    expect([...probe.faces]).toEqual([
      [theirs.id, 1],
      [mine.id, 1],
    ])
    expect(battlefieldRows()).toEqual([])
  })

  it('redraws no card for a batch that only moves the turn along while the prompt is on screen', () => {
    const { h, send, view } = mount()
    act(() => h.deliver(priority(5, options(view()))))
    const table = clone(tableOf(view()))
    table.turn = { ...table.turn, step: 'CombatDamage' }
    reset()
    send({ events: [{ Public: { StepBegan: { step: 'CombatDamage' } } }], table })
    expect(probe.faces.size).toBe(0)
  })

  it('redraws only the viewer cards when it is answered, once each', () => {
    const { h, view } = mount()
    const own = ownOf(view())
    act(() => h.deliver(priority(5, options(view()))))
    reset()
    act(() => {
      h.session.answer({ Priority: 'Pass' })
    })
    expect(new Set(probe.faces.keys())).toEqual(own)
    for (const count of probe.faces.values()) expect(count).toBe(1)
    expect(battlefieldRows()).toEqual([])
  })

  it('redraws only viewer cards when another decision replaces it', () => {
    const { h, view } = mount()
    const own = ownOf(view())
    act(() => h.deliver(priority(5, options(view()))))
    reset()
    act(() => h.deliver(priority(6, { ...options(view()), castable: handOf(view()).slice(1, 4) })))
    expect(probe.faces.size).toBeGreaterThan(0)
    for (const id of probe.faces.keys()) expect(own.has(id)).toBe(true)
    for (const count of probe.faces.values()) expect(count).toBeLessThanOrEqual(2)
    expect(battlefieldRows()).toEqual([])
  })

  it('redraws no card for a decision that belongs to another seat', () => {
    const { h, view } = mount()
    reset()
    act(() => h.deliver(priority(5, options(view()), 3)))
    expect(probe.faces.size).toBe(0)
    expect(probe.rows.size).toBe(0)
  })
})
