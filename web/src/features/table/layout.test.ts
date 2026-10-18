// Goal: the seat arrangement is right for every table size 2 to 12 and every viewer seat: opponents in turn order after the viewer, grid mode with 1 to 3 columns, focus mode with one large opponent and a tile for each, and a default focus that follows the turn.
import { describe, expect, it } from 'vitest'
import {
  arrangeSeats,
  COMPACT_FROM_SEATS,
  defaultFocus,
  defaultMode,
  GRID_MAX_SEATS,
  opponentsOf,
} from './layout'
import type { TableMode } from './layout'

const COUNTS = Array.from({ length: 11 }, (_, index) => index + 2)
const viewersOf = (count: number): number[] => Array.from({ length: count }, (_, viewer) => viewer)
const MODES: TableMode[] = ['grid', 'focus']

describe('defaultMode', () => {
  it.each(COUNTS)('picks the mode of a table of %i seats by the grid limit', (count) => {
    expect(defaultMode(count)).toBe(count <= GRID_MAX_SEATS ? 'grid' : 'focus')
  })

  it('uses grid for up to 4 seats and focus from 5', () => {
    expect([2, 3, 4].map(defaultMode)).toEqual(['grid', 'grid', 'grid'])
    expect([5, 8, 12].map(defaultMode)).toEqual(['focus', 'focus', 'focus'])
  })
})

describe('opponentsOf', () => {
  it.each(COUNTS)(
    'lists every other seat once, in turn order, at a table of %i for every viewer',
    (count) => {
      for (const viewer of viewersOf(count)) {
        const opponents = opponentsOf(count, viewer)
        expect(opponents, `viewer ${viewer}`).toHaveLength(count - 1)
        expect(opponents, `viewer ${viewer}`).not.toContain(viewer)
        expect(new Set(opponents).size, `viewer ${viewer}`).toBe(count - 1)
        expect(opponents[0], `viewer ${viewer}`).toBe((viewer + 1) % count)
        expect(opponents.at(-1), `viewer ${viewer}`).toBe((viewer + count - 1) % count)
        expect(
          opponents.every((seat) => seat >= 0 && seat < count),
          `viewer ${viewer}`,
        ).toBe(true)
      }
    },
  )

  it('wraps past the last seat', () => {
    expect(opponentsOf(4, 2)).toEqual([3, 0, 1])
    expect(opponentsOf(2, 1)).toEqual([0])
  })

  it('has nobody to play against alone at the table', () => {
    expect(opponentsOf(1, 0)).toEqual([])
    expect(opponentsOf(0, 0)).toEqual([])
  })
})

describe('arrangeSeats in grid mode', () => {
  it.each(COUNTS)('draws every opponent with 1 to 3 columns at a table of %i for every viewer', (count) => {
    for (const viewer of viewersOf(count)) {
      const plan = arrangeSeats({ seatCount: count, viewer, mode: 'grid' })
      expect(plan.mode).toBe('grid')
      expect(plan.viewer).toBe(viewer)
      expect(plan.boards, `viewer ${viewer}`).toEqual(opponentsOf(count, viewer))
      expect(plan.opponents).toEqual(plan.boards)
      expect(plan.tiles).toEqual([])
      expect(plan.focused).toBeNull()
      expect(plan.columns).toBe(Math.min(3, count - 1))
      expect(plan.compact).toBe(count >= COMPACT_FROM_SEATS)
    }
  })

  it.each([
    [2, 1],
    [3, 2],
    [4, 3],
    [5, 3],
    [12, 3],
  ])('uses %i seats in %i columns', (count, columns) => {
    expect(arrangeSeats({ seatCount: count, viewer: 0, mode: 'grid' }).columns).toBe(columns)
  })

  it('ignores a focus in grid mode', () => {
    const plan = arrangeSeats({ seatCount: 4, viewer: 0, mode: 'grid', focus: 2 })
    expect(plan.focused).toBeNull()
    expect(plan.boards).toEqual([1, 2, 3])
  })

  it('is the mode of a table of up to 4 seats when none is given', () => {
    for (const count of [2, 3, 4]) expect(arrangeSeats({ seatCount: count, viewer: 0 }).mode).toBe('grid')
  })
})

describe('arrangeSeats in focus mode', () => {
  it.each(COUNTS)(
    'shows one large opponent and a tile for each at a table of %i for every viewer',
    (count) => {
      for (const viewer of viewersOf(count)) {
        const plan = arrangeSeats({ seatCount: count, viewer, mode: 'focus' })
        expect(plan.mode).toBe('focus')
        expect(plan.tiles, `viewer ${viewer}`).toEqual(opponentsOf(count, viewer))
        expect(plan.columns).toBe(1)
        expect(plan.focused, `viewer ${viewer}`).not.toBeNull()
        expect(plan.focused).not.toBe(viewer)
        expect(plan.tiles).toContain(plan.focused)
        expect(plan.boards).toEqual([plan.focused])
      }
    },
  )

  it('is the mode of a table of 5 or more seats when none is given', () => {
    for (const count of [5, 8, 12]) expect(arrangeSeats({ seatCount: count, viewer: 0 }).mode).toBe('focus')
  })

  it.each(COUNTS)('honours any chosen opponent at a table of %i for every viewer', (count) => {
    for (const viewer of viewersOf(count)) {
      for (const focus of opponentsOf(count, viewer)) {
        const plan = arrangeSeats({ seatCount: count, viewer, mode: 'focus', focus })
        expect(plan.focused, `viewer ${viewer} focus ${focus}`).toBe(focus)
        expect(plan.boards).toEqual([focus])
      }
    }
  })

  it.each([
    ['the viewer', 2],
    ['a seat that is not at the table', 40],
    ['a negative seat', -1],
  ])('falls back to the default when asked to focus %s', (_what, focus) => {
    const context = { active: 4, holder: null, eliminated: [] }
    const plan = arrangeSeats({ seatCount: 6, viewer: 2, mode: 'focus', focus, context })
    expect(plan.focused).toBe(4)
  })

  it('marks cards compact from 8 seats whatever the mode', () => {
    for (const mode of MODES) {
      expect(arrangeSeats({ seatCount: 7, viewer: 0, mode }).compact).toBe(false)
      expect(arrangeSeats({ seatCount: 8, viewer: 0, mode }).compact).toBe(true)
    }
  })
})

describe('defaultFocus', () => {
  const none = { active: null, holder: null, eliminated: [] }

  it('follows the active player when it is not the viewer', () => {
    expect(defaultFocus(6, 0, { ...none, active: 3, holder: 5 })).toBe(3)
  })

  it('follows the priority holder when the viewer is the active player', () => {
    expect(defaultFocus(6, 0, { ...none, active: 0, holder: 5 })).toBe(5)
  })

  it('takes the next seat after the viewer when neither is an opponent', () => {
    expect(defaultFocus(6, 4, { ...none, active: 4, holder: 4 })).toBe(5)
    expect(defaultFocus(6, 5, { ...none, active: 5, holder: null })).toBe(0)
    expect(defaultFocus(6, 2)).toBe(3)
  })

  it('skips eliminated seats when it falls back to the next seat', () => {
    expect(defaultFocus(6, 0, { ...none, eliminated: [1, 2] })).toBe(3)
  })

  it('still names a seat when every opponent has lost', () => {
    expect(defaultFocus(3, 0, { ...none, eliminated: [1, 2] })).toBe(1)
  })

  it('names nobody when the viewer is alone', () => {
    expect(defaultFocus(1, 0)).toBeNull()
  })

  it('keeps the turn in view even when the active player has lost', () => {
    expect(defaultFocus(5, 0, { active: 2, holder: null, eliminated: [2] })).toBe(2)
  })
})

describe('arrangeSeats on bad input', () => {
  it.each([
    [0, 0],
    [1, 0],
    [3, 7],
    [3, -2],
    [Number.NaN, 0],
  ])('never throws for %s seats and viewer %s', (count, viewer) => {
    for (const mode of MODES) {
      const plan = arrangeSeats({ seatCount: count, viewer, mode })
      expect(plan.columns).toBeGreaterThanOrEqual(1)
      expect(plan.columns).toBeLessThanOrEqual(3)
      expect(plan.opponents).not.toContain(plan.viewer)
    }
  })

  it('keeps the viewer in range by wrapping', () => {
    expect(arrangeSeats({ seatCount: 3, viewer: 7 }).viewer).toBe(1)
    expect(arrangeSeats({ seatCount: 3, viewer: -2 }).viewer).toBe(1)
  })
})
