//! Goal: a board fits afresh for `RESIZE_SETTLE_MS` after a window resize and holds its size again afterwards, so growing the window back gives the size a fresh load has.
import { describe, expect, it } from 'vitest'
import { noteResize, RESIZE_SETTLE_MS, settling } from './resizeSettle'

describe('the settle after a resize', () => {
  it.each([
    [0, true],
    [1, true],
    [RESIZE_SETTLE_MS - 1, true],
    [RESIZE_SETTLE_MS, false],
    [RESIZE_SETTLE_MS * 10, false],
  ])('%i ms after a resize it is settling: %s', (after, expected) => {
    noteResize(10_000)
    expect(settling(10_000 + after)).toBe(expected)
  })
})
