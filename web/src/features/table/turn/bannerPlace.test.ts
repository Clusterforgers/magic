// Goal: the turn banner stands in the free room of the top bar when the room holds its words, drops the turn number (which the badge says) when the room is narrow, and hangs under the bar when the room has no space for it, so it never lies over the step ladder or the turn badge.
import { describe, expect, it } from 'vitest'
import { BANNER_BAR_MIN_PX, BANNER_TURN_MIN_PX, bannerPlace } from './bannerPlace'

describe('where the banner stands for the free room of the bar', () => {
  it.each([
    // [free room in px, place, turn number written]
    [null, 'bar', true],
    [0, 'below', true],
    [1, 'below', true],
    [BANNER_BAR_MIN_PX - 1, 'below', true],
    [BANNER_BAR_MIN_PX, 'bar', false],
    [150, 'bar', false],
    [BANNER_TURN_MIN_PX - 1, 'bar', false],
    [BANNER_TURN_MIN_PX, 'bar', true],
    [273, 'bar', true],
  ] as const)('a room of %s px: %s, turn number %s', (room, place, turn) => {
    expect(bannerPlace(room)).toEqual({ place, turn })
  })

  it('writes the turn number only where the bar holds the whole band', () => {
    expect(BANNER_TURN_MIN_PX).toBeGreaterThan(BANNER_BAR_MIN_PX)
  })
})
