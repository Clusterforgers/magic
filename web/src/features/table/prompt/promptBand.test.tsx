// Goal: the prompt band is one line of one grid for every prompt kind (two lines on a phone): the lead, the sentence, the meter, short choices, the clock and the answers, the primary answer at the right end; what a prompt chooses among is no cell of it but drawn in the Stack column; the stylesheet says what `promptBand.ts` says; and the band's height is the strip's, a phone's two lines, whatever the prompt.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { DEV_PROMPTS } from '@/features/table/dev/prompts'
import { buildFixture } from '@/features/table/dev/fixture'
import { must } from '@/test-utils/batches'
import { blockAfter } from '@/test-utils/cssBlocks'
import { renderTable } from '@/test-utils/tableHarness'
import {
  BAND_TICKER_PX,
  BAND_MIN_PHONE_PX,
  BAND_MIN_PX,
  BAND_PHONE_PX,
  SHARED_CLAMP,
  SHEET_MAX_VH,
  bandHeight,
  clampedHeight,
  regionSizes,
  sheetMost,
} from '../layout'
import {
  BAND_HINTS_REM,
  BAND_NARROW_REM,
  BAND_PHONE_REM,
  SENTENCE_LINES,
  bandAreas,
  isClamped,
} from './promptBand'
import type { BandChoices } from './promptBand'

// Read as text rather than imported, which would hand back what Tailwind builds from it.
const css = readFileSync(join(import.meta.dirname, 'promptBand.css'), 'utf8')
const band = blockAfter(css, '@utility strip-band')

/** The kinds of the dev table, what each keeps in the band's middle and whether its choices go to the column. */
const KINDS: Array<[string, BandChoices, boolean]> = [
  ['priority', 'none', false],
  ['yesno', 'none', false],
  ['x', 'inline', false],
  ['targets', 'none', true],
  ['cards', 'none', true],
  ['mode-two', 'none', true],
  ['order', 'none', true],
  ['pay', 'none', true],
]

/** The table with the dev prompt `name` owed. */
function owe(name: string) {
  const fixture = buildFixture({ seats: 2 })
  const pending = must(DEV_PROMPTS[name])(fixture)
  return renderTable({ fixture, state: (state) => ({ ...state, pending }) })
}

/** A grid-template-areas value as rows of cells. */
const rowsOf = (value: string): string[][] =>
  [...value.matchAll(/'([^']*)'/g)].map((match) => (match[1] ?? '').trim().split(/\s+/))

/** The grid-template-areas of the first rule in `block` whose head contains `head` (or the block's own). */
function areasIn(block: string, head?: string): string[][] {
  // The block's own declarations come before its first nested rule.
  const scope = head === undefined ? block.slice(0, block.indexOf('{')) : blockAfter(block, head)
  return rowsOf(must(/grid-template-areas:([^;]*);/.exec(scope))[1] ?? '')
}

const CHOICES: BandChoices[] = ['none', 'inline', 'fallback']

describe('the band rules', () => {
  it.each(CHOICES)('keeps the answers at the right end of their line for %s at every width', (choices) => {
    for (const rem of [24, 39, 40, 64, 90, 150]) {
      for (const quiet of [false, true]) {
        const rows = bandAreas(rem, choices, { quiet })
        // A narrow band's second line is the foot, whose right end the answers have.
        if (rem < BAND_PHONE_REM) expect(new Set(rows.at(-1))).toEqual(new Set(['foot']))
        else expect(must(rows.find((row) => row.includes('actions'))).at(-1)).toBe('actions')
        expect(rows.flat().filter((cell) => cell === 'lead')).toHaveLength(1)
      }
    }
  })

  it('is one line from a tablet up and two on a phone, unless choices have nowhere else to go', () => {
    for (const choices of ['none', 'inline'] as const) {
      expect(bandAreas(BAND_PHONE_REM, choices)).toHaveLength(1)
      expect(bandAreas(150, choices)).toHaveLength(1)
      expect(bandAreas(BAND_PHONE_REM - 1, choices)).toHaveLength(2)
    }
    expect(bandAreas(64, 'fallback')).toHaveLength(2)
    expect(bandAreas(24, 'fallback')).toHaveLength(3)
  })

  it('gives a quiet sentence`s place to the meter', () => {
    expect(bandAreas(64, 'none', { quiet: true })[0]).not.toContain('text')
    // A narrow band moves the meter into the sentence's cell by a rule of its own; the foot is then the answers' alone.
    expect(bandAreas(24, 'none', { quiet: true })).toEqual(bandAreas(24, 'none'))
  })

  it('tells a cut sentence from one that fits', () => {
    expect(isClamped({ scrollHeight: 20, clientHeight: 20, scrollWidth: 300, clientWidth: 300 })).toBe(false)
    expect(isClamped({ scrollHeight: 60, clientHeight: 20, scrollWidth: 300, clientWidth: 300 })).toBe(true)
    expect(isClamped({ scrollHeight: 20, clientHeight: 20, scrollWidth: 500, clientWidth: 300 })).toBe(true)
  })
})

describe('the stylesheet', () => {
  const phone = blockAfter(band, `@container band (width < ${BAND_PHONE_REM}rem)`)
  const fallback =
    "&:has([data-slot='decision-line'] > [data-slot='prompt-choices']:not([data-inline]):not(:empty))"
  const quiet = "&:has([data-slot='prompt-sentence'][data-quiet])"

  it('draws the areas the rules name', () => {
    expect(areasIn(band)).toEqual(bandAreas(64, 'none'))
    expect(areasIn(band, fallback)).toEqual(bandAreas(64, 'fallback'))
    expect(areasIn(band, quiet)).toEqual(bandAreas(64, 'none', { quiet: true }))
    expect(areasIn(phone, '& {')).toEqual(bandAreas(24, 'none'))
    expect(areasIn(phone, fallback)).toEqual(bandAreas(24, 'fallback'))
    expect(areasIn(phone, quiet)).toEqual(bandAreas(24, 'none', { quiet: true }))
    // The foot: the meter, the status and short choices from its left, the answers from its right.
    const foot = blockAfter(band, 'On a phone the second line is the foot')
    expect(blockAfter(foot, "[data-slot='prompt-choices'][data-inline] {")).toMatch(
      /grid-area:\s*foot;\s*justify-self:\s*start/,
    )
    expect(blockAfter(foot, "& [data-slot='decision-line'] > [data-slot='prompt-actions'] {")).toMatch(
      /grid-area:\s*foot;\s*justify-self:\s*end/,
    )
    expect(blockAfter(foot, "[data-slot='prompt-sentence'][data-quiet]")).toMatch(/grid-area:\s*text/)
  })

  it('places every part of a prompt in its cell', () => {
    for (const [slot, area] of [
      ['prompt-sentence', 'text'],
      ['prompt-meter', 'meter'],
      ['prompt-choices', 'choices'],
      ['response-clock', 'clock'],
      ['prompt-actions', 'actions'],
    ]) {
      expect(blockAfter(band, `& [data-slot='decision-line'] > [data-slot='${slot}'] {`)).toMatch(
        new RegExp(`grid-area:\\s*${area};`),
      )
    }
    // The asking card's chip and the stack chips share the lead: one of them is drawn.
    expect(band).toMatch(
      /& > \[data-slot='strip-stack'\],\s*& \[data-slot='decision-line'\] > \[data-slot='prompt-asker'\] \{\s*grid-area:\s*lead;/,
    )
  })

  it('keeps the answers and short choices to one row that scrolls, never a second line', () => {
    const answers = blockAfter(band, "& [data-slot='decision-line'] > [data-slot='prompt-actions'] {")
    expect(answers).toMatch(/flex-wrap:\s*nowrap/)
    expect(answers).toMatch(/overflow-x:\s*auto/)
    expect(blockAfter(band, "[data-slot='prompt-choices'][data-inline] {")).toMatch(/flex-wrap:\s*nowrap/)
  })

  it(`clamps the sentence to ${SENTENCE_LINES} lines`, () => {
    expect(blockAfter(css, ".strip-band [data-slot='prompt-sentence'] .prompt-sentence-clamp")).toMatch(
      new RegExp(`line-clamp:\\s*${SENTENCE_LINES};`),
    )
  })

  it('lays the answers out right to left, so the first, the primary, sits at the right end', () => {
    const answers = blockAfter(css, '@utility prompt-answers')
    expect(answers).toMatch(/flex-direction:\s*row-reverse/)
    expect(answers).toMatch(/justify-content:\s*flex-start/)
  })

  it("keeps a narrow band's lead for the top chip and drops the key hints only where the answers need the room", () => {
    const narrow = blockAfter(band, `@container band (width < ${BAND_NARROW_REM}rem)`)
    expect(blockAfter(narrow, "[data-slot='stack-label']")).toMatch(/clip-path:\s*inset\(50%\)/)
    expect(blockAfter(narrow, "[data-slot='stack-more-word']")).toMatch(/display:\s*none/)
    const hints = blockAfter(band, `@container band (width < ${BAND_HINTS_REM}rem)`)
    expect(hints).toMatch(/\[data-slot='prompt-actions'\] \[data-slot='key-hint'\],/)
    expect(blockAfter(hints, "[data-slot='prompt-actions'] [data-slot='undo-rest']")).toMatch(
      /display:\s*none/,
    )
  })

  it('draws every list one column wide in the Stack column, three cards to a row', () => {
    const column = blockAfter(css, "\n[data-slot='decision-choices'] {")
    expect(blockAfter(column, "& [data-slot='mode-list']")).toMatch(
      /grid-template-columns:\s*minmax\(0, 1fr\)/,
    )
    expect(blockAfter(column, "& [class*='max-h-']")).toMatch(/max-height:\s*none/)
    expect(blockAfter(column, '> [data-candidate]')).toMatch(/--card-w:\s*calc\(\(100cqi - 1rem\) \/ 3/)
  })

  it('moves nothing under reduced motion', () => {
    expect(blockAfter(css, '@utility decision-sheet')).toMatch(
      /@media \(prefers-reduced-motion: no-preference\)\s*\{\s*animation:/,
    )
    expect(blockAfter(css, '@utility asker-chip')).toMatch(
      /@media \(prefers-reduced-motion: reduce\)\s*\{\s*transition:\s*none/,
    )
    expect(blockAfter(css, '@utility detail-row')).toMatch(
      /@media \(prefers-reduced-motion: reduce\)\s*\{\s*transition:\s*none/,
    )
    expect(css).toMatch(/@media \(prefers-reduced-motion: no-preference\) \{\s*\.detail-row\[data-picked\]/)
  })
})

describe('the band in the strip', () => {
  it.each(KINDS)(
    'draws %s as cells of one grid, the primary answer first and its choices where they belong',
    (kind, choices, column) => {
      owe(kind)
      const row = must(document.querySelector<HTMLElement>('[data-slot="strip-row"]'))
      expect(row).toHaveClass('strip-band')
      const line = must(row.querySelector<HTMLElement>('[data-slot="decision-line"]'))
      // No box of its own: its parts are the grid's cells.
      expect(line).toHaveClass('contents')
      expect(line.parentElement).toHaveClass('contents')
      expect(must(line.querySelector('[data-slot="prompt-text"]'))).toHaveClass('prompt-sentence-clamp')
      const middle = line.querySelector('[data-slot="prompt-choices"]')
      if (choices === 'none') expect(middle).toBeNull()
      else expect(middle?.hasAttribute('data-inline')).toBe(choices === 'inline')
      // What the prompt chooses among is in the Stack column, once.
      const aside = document.querySelectorAll('[data-slot="decision-choices"] [data-slot="prompt-choices"]')
      expect(aside).toHaveLength(column ? 1 : 0)
      const answers = line.querySelector<HTMLElement>('[data-slot="prompt-actions"]')
      if (kind === 'priority') return
      expect(answers).toHaveClass('prompt-answers')
      // The primary answer comes first, so the right-to-left row puts it at the right end and Tab reaches it first.
      const first = must(answers?.querySelector('button'))
      expect(first).toHaveAccessibleName(/^(Confirm|Choose|No)/)
    },
  )

  it('keeps the stack and the status as cells beside the band, hidden while a card asks', () => {
    owe('targets')
    const row = must(document.querySelector<HTMLElement>('[data-slot="strip-row"]'))
    expect([...row.children].map((child) => child.getAttribute('data-slot'))).toEqual([
      'strip-stack',
      'strip-prompt',
      'strip-status',
    ])
    expect(row.querySelector('[data-slot="strip-stack"]')).toHaveAttribute('hidden')
    expect(row.querySelector('[data-slot="strip-status"]')).toHaveAttribute('hidden')
    expect(must(document.querySelector('[data-slot="shared-sheet"]'))).toHaveClass('band-room')
    expect(document.querySelector('[data-slot="shared-sheet"]')).toHaveAttribute('data-keep-clear')
  })

  it('keeps them in view for a decision nothing asks', () => {
    owe('priority')
    const row = must(document.querySelector<HTMLElement>('[data-slot="strip-row"]'))
    expect(row.querySelector('[data-slot="strip-stack"]')).not.toHaveAttribute('hidden')
    expect(row.querySelector('[data-slot="strip-status"]')).not.toHaveAttribute('hidden')
    expect(row.querySelector('[data-slot="prompt-asker"]')).toBeNull()
  })

  it('leaves out "You have priority" beside the seat\'s own prompt, which says it already', () => {
    owe('priority')
    expect(screen.getByRole('region', { name: 'Turn and priority' })).not.toHaveTextContent(/has priority/)
  })
})

describe('the band`s height', () => {
  it.each([
    [1830, 855],
    [2550, 1380],
    [1400, 900],
    [1280, 720],
    [1100, 800],
  ])("is the strip and the ticker's line at %ix%i, whatever the prompt", (width, height) => {
    const regions = regionSizes({ width, height })
    expect(regions.sheet).toBe(clampedHeight(SHARED_CLAMP, height) + BAND_TICKER_PX)
    expect(regions.sheet).toBe(bandHeight(height, false))
    expect(regions.sheetRoom).toBe(BAND_TICKER_PX)
    // One row of 44 px controls, the ticker, the padding and the edge fit in it.
    expect(regions.sheet).toBeGreaterThanOrEqual(44 + BAND_TICKER_PX + 8 + 4)
    expect(regions.sheet).toBeLessThanOrEqual(88)
  })

  it.each([
    [390, 844],
    [360, 640],
    [430, 932],
    // A tablet's band beside a pod column is under 40 rem wide: two lines as well.
    [768, 1024],
    [1000, 700],
  ])('is two lines in a narrow window at %ix%i, the second kept above the strip', (width, height) => {
    const regions = regionSizes({ width, height })
    expect(regions.sheet).toBe(BAND_PHONE_PX)
    expect(regions.sheetRoom).toBe(BAND_PHONE_PX - regions.shared)
  })
})

describe('the sheet of a prompt with no column to draw in', () => {
  it.each([
    [1830, 855],
    [2550, 1380],
    [1400, 900],
    [1280, 720],
  ])(
    'never squeezes under a sentence and a row of answers over an empty board at %ix%i',
    (_width, height) => {
      // Nothing on the opponents' boards: their row is down to its pods, which a sheet may not cover.
      const most = sheetMost(height, 56, 40, false, 120)
      expect(most).toBeGreaterThanOrEqual(BAND_MIN_PX)
      expect(most).toBeLessThanOrEqual((height * SHEET_MAX_VH) / 100)
    },
  )

  it('keeps a phone its own floor', () => {
    expect(sheetMost(844, 56, 100, true)).toBe(BAND_MIN_PHONE_PX)
  })

  it('still grows to its room where the boards leave it, and never past 40 % of the window', () => {
    expect(sheetMost(900, 63, 400, false, 120)).toBe(343)
    expect(sheetMost(900, 63, 900, false)).toBe(360)
  })
})
