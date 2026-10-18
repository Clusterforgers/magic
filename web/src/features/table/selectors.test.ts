// Goal: the table's selectors split the shared battlefield by controller and row, filter exile by owner, read the stack top first and pick the default focus, and they hand back the same list while its content is the same.
import { describe, expect, it } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import { create, useStore } from 'zustand'
import { useShallow } from 'zustand/react/shallow'
import { clone, must } from '@/test-utils/batches'
import type { StateView } from '@/protocol/generated'
import { buildFixture } from './dev/fixture'
import { fixtureState } from './dev/devGame'
import {
  battlefieldCounts,
  battlefieldRow,
  combatRoleIn,
  defaultFocusOf,
  exileOf,
  isBattlefieldEmpty,
  NO_IDS,
  playerOf,
  seatCountOf,
  splitBattlefield,
  stackTopFirst,
} from './selectors'
import type { PlayState } from '@/play/store'
import { indexById } from '@/play/view'

const fixture = buildFixture({ seats: 4 })
const state = fixtureState(fixture)

/** The state with `view` replaced, as a batch would leave it. */
function withView(view: StateView): PlayState {
  return { ...state, view, objectsById: indexById(view.objects) }
}

describe('splitBattlefield', () => {
  const rows = splitBattlefield(must(state.view), state.objectsById)

  it('gives every seat 16 lands, 14 creatures and 10 other permanents', () => {
    for (let seat = 0; seat < 4; seat += 1) {
      const mine = must(rows.get(seat), `seat ${seat}`)
      expect([mine.lands.length, mine.creatures.length, mine.other.length]).toEqual([16, 14, 10])
    }
  })

  it('files each id in the row of its types and under the seat that controls it', () => {
    for (const [seat, mine] of rows) {
      for (const id of mine.creatures) {
        const object = must(state.objectsById[id])
        expect(object.controller).toBe(seat)
        expect(object.types).toContain('Creature')
      }
      for (const id of mine.lands) expect(must(state.objectsById[id]).types).toContain('Land')
      for (const id of mine.other) {
        expect(must(state.objectsById[id]).types.some((type) => type === 'Creature' || type === 'Land')).toBe(
          false,
        )
      }
    }
  })

  it('covers the whole battlefield exactly once, in battlefield order', () => {
    const all = [...rows.values()].flatMap((mine) => [...mine.lands, ...mine.creatures, ...mine.other])
    expect(all.toSorted((a, b) => a - b)).toEqual(
      must(state.view).battlefield.cards?.toSorted((a, b) => a - b),
    )
    for (const mine of rows.values()) {
      const order = (ids: number[]) => ids.map((id) => must(state.view).battlefield.cards?.indexOf(id) ?? -1)
      for (const ids of [mine.lands, mine.creatures, mine.other])
        expect(order(ids)).toEqual(order(ids).toSorted((a, b) => a - b))
    }
  })

  it('is computed once per view and again for a new one', () => {
    const view = must(state.view)
    expect(splitBattlefield(view, state.objectsById)).toBe(rows)
    expect(splitBattlefield({ ...view }, state.objectsById)).not.toBe(rows)
  })

  it('moves a permanent to its new controller', () => {
    const view = clone(must(state.view))
    const stolen = must(
      view.objects.find((object) => object.controller === 0 && object.types.includes('Creature')),
    )
    stolen.controller = 2
    const moved = splitBattlefield(view, indexById(view.objects))
    expect(moved.get(0)?.creatures).not.toContain(stolen.id)
    expect(moved.get(2)?.creatures).toContain(stolen.id)
  })

  it('skips an id whose object is not in the view', () => {
    const view = clone(must(state.view))
    const [missing] = must(view.battlefield.cards)
    const objects = indexById(view.objects.filter((object) => object.id !== missing))
    const split = splitBattlefield(view, objects)
    expect(
      [...split.values()].flatMap((mine) => [...mine.lands, ...mine.creatures, ...mine.other]),
    ).not.toContain(missing)
  })
})

describe('battlefieldRow', () => {
  it('returns one shared empty list when there is no view or no such seat', () => {
    expect(battlefieldRow({ ...state, view: null }, 0, 'lands')).toBe(NO_IDS)
    expect(battlefieldRow(state, 9, 'creatures')).toBe(NO_IDS)
  })

  it('keeps the same items for the same row after a change that does not touch it', () => {
    const before = battlefieldRow(state, 1, 'creatures')
    const view = clone(must(state.view))
    const lands = must(
      view.objects.find((object) => object.controller === 1 && object.types.includes('Land')),
    )
    must(lands.permanent).tapped = !must(lands.permanent).tapped
    const after = battlefieldRow(withView(view), 1, 'creatures')
    expect(after).toEqual(before)
  })

  it('is a stable list under useShallow while its items are unchanged, and a new one when they change', () => {
    const store = create<PlayState>(() => state)
    const { result } = renderHook(() =>
      useStore(
        store,
        useShallow((current: PlayState) => battlefieldRow(current, 2, 'creatures')),
      ),
    )
    const first = result.current
    // A batch that only taps a land of seat 2 leaves the creature row alone.
    const view = clone(must(state.view))
    const land = must(view.objects.find((object) => object.controller === 2 && object.types.includes('Land')))
    must(land.permanent).tapped = !must(land.permanent).tapped
    act(() => store.setState(withView(view)))
    expect(result.current).toBe(first)
    // Removing a creature from the battlefield changes the row.
    const gone = must(first[0])
    const fewer = clone(view)
    fewer.battlefield = {
      count: fewer.battlefield.count - 1,
      cards: must(fewer.battlefield.cards).filter((id) => id !== gone),
    }
    act(() => store.setState(withView(fewer)))
    expect(result.current).not.toBe(first)
    expect(result.current).toEqual(first.slice(1))
  })
})

describe('battlefieldCounts and isBattlefieldEmpty', () => {
  it('counts each row of a seat', () => {
    expect(battlefieldCounts(state, 3)).toEqual({ creatures: 14, other: 10, lands: 16 })
  })

  it('is empty only for a seat that controls nothing', () => {
    expect(isBattlefieldEmpty(state, 0)).toBe(false)
    expect(isBattlefieldEmpty(state, 7)).toBe(true)
    expect(isBattlefieldEmpty({ ...state, view: null }, 0)).toBe(true)
  })
})

describe('zones', () => {
  it('lists the cards in exile that one seat owns', () => {
    const owners = must(state.view).exile.cards?.map((id) => must(state.objectsById[id]).owner)
    expect(owners?.toSorted()).toEqual([0, 1, 2])
    for (let seat = 0; seat < 3; seat += 1) {
      const ids = exileOf(state, seat)
      expect(ids).toHaveLength(1)
      expect(must(state.objectsById[must(ids[0])]).owner).toBe(seat)
    }
    expect(exileOf(state, 3)).toEqual([])
  })

  it('reads the stack top first', () => {
    const bottomUp = must(must(state.view).stack.cards)
    expect(bottomUp).toHaveLength(3)
    expect(stackTopFirst(state)).toEqual(bottomUp.toReversed())
  })

  it('has an empty stack and exile without a view', () => {
    expect(stackTopFirst({ ...state, view: null })).toEqual([])
    expect(exileOf({ ...state, view: null }, 0)).toEqual([])
  })

  it('finds a player by seat', () => {
    expect(playerOf(state, 2)?.seat).toBe(2)
    expect(playerOf(state, 9)).toBeUndefined()
  })
})

describe('combatRoleIn', () => {
  it('reads the role from the current view', () => {
    const [attacker] = must(state.view).combat.attackers
    expect(combatRoleIn(state, must(attacker).object)).toBe('blocked')
    expect(combatRoleIn(state, 9999)).toBeNull()
    expect(combatRoleIn({ ...state, view: null }, must(attacker).object)).toBeNull()
  })
})

describe('seatCountOf and defaultFocusOf', () => {
  it('counts the seats from the view, else from the Welcome', () => {
    expect(seatCountOf(state)).toBe(4)
    expect(seatCountOf({ ...state, view: null, seatCount: 6 })).toBe(6)
    expect(seatCountOf({ ...state, view: null, seatCount: null })).toBe(0)
  })

  it('follows the active player, then the priority holder, then the next seat', () => {
    expect(defaultFocusOf(state)).toBe(1)
    const own = clone(must(state.view))
    own.turn.active = 0
    own.priority = { holder: 3, passed: [] }
    expect(defaultFocusOf(withView(own))).toBe(3)
    own.priority = null
    expect(defaultFocusOf(withView(own))).toBe(1)
  })

  it('skips an eliminated next seat and says nothing without a view or a seat', () => {
    const own = clone(must(state.view))
    own.turn.active = 0
    own.priority = null
    must(own.players[1]).lost = 'Conceded'
    expect(defaultFocusOf(withView(own))).toBe(2)
    expect(defaultFocusOf({ ...state, view: null })).toBeNull()
    expect(defaultFocusOf({ ...state, seat: null })).toBeNull()
  })
})
