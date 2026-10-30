// Goal: the dev address opens the Effects bar only for `?bar=1` (or true), takes `?board=` only for a named board (a two-seat table), `?fx=` only for one of the moves the dev table can play, `?slow=` only for a slow-down above 1 and up to 20 and `?preview=` only for a whole number of milliseconds up to 10 000, and shrugs off every other value, so a mistyped address still shows the plain table.
import { describe, expect, it } from 'vitest'
import { DEV_FX, MAX_DEV_PREVIEW_MS, MAX_DEV_SLOW, parseDevSearch } from './devSearch'

describe('?fx=', () => {
  it.each(DEV_FX)('reads %s', (fx) => {
    expect(parseDevSearch({ seats: 2, fx }).fx).toBe(fx)
  })

  it.each([undefined, '', 'Damage', 'damage ', 'zap', 7, true, null, ['death']])('leaves out %j', (fx) => {
    expect(parseDevSearch({ seats: 2, fx }).fx).toBeUndefined()
  })

  it('names the moves the table buttons are made from', () => {
    expect([...DEV_FX]).toEqual([
      'attack',
      'block',
      'damage',
      'death',
      'life',
      'enter',
      'cast',
      'rival',
      'activate',
      'resolve',
      'counter',
      'land',
      'draw',
      'equip',
    ])
  })
})

describe('?newturn=', () => {
  it.each([1, true, '1'])('passes the turn for %j', (newturn) => {
    expect(parseDevSearch({ newturn }).newturn).toBe(true)
  })

  it.each([undefined, 0, '0', '', 'yes', 2, false, null])('leaves the turn alone for %j', (newturn) => {
    expect(parseDevSearch({ newturn }).newturn).toBeUndefined()
  })
})

describe('?bar=', () => {
  it.each([1, true, '1'])('opens the bar for %j', (bar) => {
    expect(parseDevSearch({ bar }).bar).toBe(true)
  })

  it.each([undefined, 0, '0', '', 'yes', 2, false, null])('leaves it folded for %j', (bar) => {
    expect(parseDevSearch({ bar }).bar).toBeUndefined()
  })
})

describe('?preview=', () => {
  it.each([0, 1, 120, 3000, MAX_DEV_PREVIEW_MS])('reads %j', (preview) => {
    expect(parseDevSearch({ seats: 2, preview }).preview).toBe(preview)
  })

  it.each([undefined, -1, 0.5, 1.5, MAX_DEV_PREVIEW_MS + 1, '300', '', Number.NaN, Infinity, null, true])(
    'leaves out %j',
    (preview) => {
      expect(parseDevSearch({ seats: 2, preview }).preview).toBeUndefined()
    },
  )
})

describe('?slow=', () => {
  it.each([1.5, 2, 5, 10, MAX_DEV_SLOW])('reads %j', (slow) => {
    expect(parseDevSearch({ seats: 2, slow }).slow).toBe(slow)
  })

  it.each([undefined, 1, 0.5, 0, -3, MAX_DEV_SLOW + 1, 1000, '5', '', Number.NaN, Infinity, null, true])(
    'leaves out %j',
    (slow) => {
      expect(parseDevSearch({ seats: 2, slow }).slow).toBeUndefined()
    },
  )

  it('goes along with the rest of the address', () => {
    expect(parseDevSearch({ seats: 6, mode: 'focus', fx: 'life', slow: 4 })).toEqual({
      seats: 6,
      mode: 'focus',
      focus: undefined,
      prompt: undefined,
      turn: undefined,
      fx: 'life',
      slow: 4,
    })
  })
})

describe('?board=', () => {
  it.each([
    ['duel', 4, 'duel', 2],
    ['heavy', 6, 'heavy', 2],
    ['Duel', 6, undefined, 6],
    [3, 3, undefined, 3],
  ])('reads %j over %i seats', (board, seats, expected, table) => {
    expect(parseDevSearch({ seats, board })).toMatchObject({ board: expected, seats: table })
  })
})
