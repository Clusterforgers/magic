// Goal: the lobby's seat list is drawn as it is up to 12 seats and as a scrolling list of tighter rows from 13, for any number of seats.
import { describe, expect, it } from 'vitest'
import { COMPACT_LIST_FROM_SEATS, seatListClasses } from './seatListShape'

describe('seatListClasses', () => {
  it.each([2, 3, 4, 8, 12])('leaves a list of %i seats as it is', (seats) => {
    expect(seatListClasses(seats)).toBe('')
  })

  it.each([13, 16, 24, 40, 64, 1000])(
    'scrolls a list of %i seats in a box and tightens its rows',
    (seats) => {
      const classes = seatListClasses(seats).split(' ')
      expect(classes).toContain('overflow-y-auto')
      expect(classes.some((name) => name.startsWith('max-h-'))).toBe(true)
      expect(classes).toContain('[&>li]:py-2')
    },
  )

  it('turns compact at the number it names', () => {
    expect(seatListClasses(COMPACT_LIST_FROM_SEATS - 1)).toBe('')
    expect(seatListClasses(COMPACT_LIST_FROM_SEATS)).not.toBe('')
  })
})
