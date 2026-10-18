// Goal: the dev address takes `?fx=` only for one of the moves the dev table can play and `?slow=` only for a slow-down above 1 and up to 20, and shrugs off every other value, so a mistyped address still shows the plain table.
import { describe, expect, it } from 'vitest'
import { DEV_FX, MAX_DEV_SLOW, parseDevSearch } from './devSearch'

describe('?fx=', () => {
  it.each(DEV_FX)('reads %s', (fx) => {
    expect(parseDevSearch({ seats: 2, fx }).fx).toBe(fx)
  })

  it.each([undefined, '', 'Damage', 'damage ', 'zap', 7, true, null, ['death']])('leaves out %j', (fx) => {
    expect(parseDevSearch({ seats: 2, fx }).fx).toBeUndefined()
  })

  it('names the moves the table buttons are made from', () => {
    expect([...DEV_FX]).toEqual(['attack', 'block', 'damage', 'death', 'life', 'enter'])
  })
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
