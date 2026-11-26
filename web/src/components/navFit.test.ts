// Goal: the main navigation's row shows every tab that fits, from the first, and leaves room for More only when a tab goes into it; a tab is current at its address and under it unless it is exact.
import { describe, expect, it } from 'vitest'
import { isCurrentTab, tabsThatFit } from './navFit'

describe('tabsThatFit', () => {
  it.each([
    ['all fit exactly, so no More', [100, 100, 100], 40, 300, 3],
    ['all fit within half a pixel', [100.1, 100.1, 100.1], 40, 300, 3],
    ['one too many: More takes its room and the last', [100, 100, 100], 40, 299, 2],
    ['More needs room beside the shown tabs', [100, 100, 100], 120, 299, 1],
    ['nothing fits beside More', [100, 100], 80, 150, 0],
    ['a narrow tab after a wide one stays in More', [50, 200, 10], 40, 200, 1],
    ['no tabs', [], 40, 0, 0],
    ['an unmeasured row (a test) shows all', [0, 0, 0], 0, 0, 3],
  ])('%s', (_case, widths, more, room, shown) => {
    expect(tabsThatFit(widths, more, room)).toBe(shown)
  })
})

describe('isCurrentTab', () => {
  it.each([
    ['/', '/', false, true],
    ['/', '/decks', false, false],
    ['/decks', '/decks', false, true],
    ['/decks', '/decks/abc', false, true],
    ['/decks', '/decks/unvalidated', true, false],
    ['/decks', '/decksx', false, false],
    ['/delves', '/delves/run-1', false, true],
  ])('%s at %s (exact %s) is %s', (to, pathname, exact, current) => {
    expect(isCurrentTab(to, pathname, exact)).toBe(current)
  })
})
