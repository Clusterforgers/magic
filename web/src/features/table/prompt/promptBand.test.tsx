// Goal: a decision's home lays its parts out as `docs/ui-design.md` (Decisions) says: the dock and the bar put the chip and the reading on one line, the question under it and the answers at the foot, a phone's answers sharing the bar's width; the panel's answers stand last and sticky, the primary alone on a full row when they wrap; a board choice dims the rest of the table and the hand; a cut question is told from one that fits.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { blockAfter } from '@/test-utils/cssBlocks'
import { buildFixture } from '../dev/fixture'
import { DEV_PROMPTS } from '../dev/prompts'
import { must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { isClamped } from './promptBand'

const css = readFileSync(join(import.meta.dirname, 'promptBand.css'), 'utf8')
const home = blockAfter(css, '@utility decision-home')

describe('the homes', () => {
  it('stacks the dock and the bar: chip and reading, question, choices, answers', () => {
    const dock = blockAfter(home, "&[data-home='dock'],\n  &[data-home='bar'] {")
    expect(dock).toContain("'chip reading clock stops'")
    expect(dock).toContain("'question question question question'")
    expect(dock).toContain("'answers answers answers answers'")
    expect(blockAfter(home, "& > [data-slot='home-stops'] {")).toContain('grid-area: stops')
  })

  it("gives a phone's bar its Stack chip and its answers the bar's width", () => {
    const bar = blockAfter(home, "A phone's bar: the chip and the Stack chip")
    expect(bar).toContain("'chip clock stops stack'")
    expect(blockAfter(home, "&[data-home='bar'] > [data-slot='prompt-actions'] > button {")).toMatch(
      /flex:\s*1 1 0/,
    )
  })

  it("keeps the panel's answers last, the primary growing to the row's width", () => {
    expect(blockAfter(home, "&[data-home='panel'] > [data-slot='prompt-actions'] {")).toMatch(
      /position:\s*sticky;[\s\S]*flex-wrap:\s*wrap-reverse/,
    )
    expect(
      blockAfter(home, "&[data-home='panel'] > [data-slot='prompt-actions'] > button:first-child {"),
    ).toMatch(/flex-grow:\s*1/)
  })
})

describe('a board choice', () => {
  it('dims every card of the table and the hand outside the choice to 0.4 and less colour', () => {
    const rule = blockAfter(css, '[data-board-choice]')
    expect(rule).toMatch(/filter:\s*saturate\(0\.5\)[\s\S]*opacity:\s*0\.4/)
    expect(css).toContain(":is([data-area='opponents'], [data-area='own'], [data-area='hand'])")
    expect(css).toContain(":not([data-highlight='legal'], [data-highlight='selected'])")
  })

  it.each([
    ['targets', true],
    ['pay', true],
    ['attack', true],
    ['block', true],
    ['priority', false],
    ['yesno', false],
    ['mode-two', false],
  ])('marks the table during %s: %s', (prompt, board) => {
    const fixture = buildFixture({ seats: 2 })
    renderTable({ fixture, state: (state) => ({ ...state, pending: must(DEV_PROMPTS[prompt])(fixture) }) })
    expect(document.querySelector('[data-slot="table-grid"]')?.hasAttribute('data-board-choice')).toBe(board)
  })
})

describe('isClamped', () => {
  it('tells a cut sentence from one that fits', () => {
    expect(isClamped({ scrollHeight: 20, clientHeight: 20, scrollWidth: 300, clientWidth: 300 })).toBe(false)
    expect(isClamped({ scrollHeight: 60, clientHeight: 20, scrollWidth: 300, clientWidth: 300 })).toBe(true)
    expect(isClamped({ scrollHeight: 20, clientHeight: 20, scrollWidth: 500, clientWidth: 300 })).toBe(true)
  })
})
