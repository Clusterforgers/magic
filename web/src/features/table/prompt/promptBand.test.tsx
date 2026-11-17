// Goal: the prompt band is one grid across the strip for every prompt kind: the sentence keeps one line, the choices sit beside it or on a line of their own by the room and their size, the primary answer stands at the right end, the stylesheet says what `promptBand.ts` says, and the sheet keeps a floor that an empty board above never squeezes.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { DEV_PROMPTS } from '@/features/table/dev/prompts'
import { buildFixture } from '@/features/table/dev/fixture'
import { must } from '@/test-utils/batches'
import { blockAfter } from '@/test-utils/cssBlocks'
import { renderTable } from '@/test-utils/tableHarness'
import { BAND_MIN_PHONE_PX, BAND_MIN_PX, SHEET_MAX_VH, sheetMost } from '../layout'
import {
  BAND_NARROW_REM,
  BAND_PHONE_REM,
  BAND_WIDE_REM,
  bandAreas,
  isClamped,
  sentenceLines,
} from './promptBand'
import type { BandChoices } from './promptBand'

// Read as text rather than imported, which would hand back what Tailwind builds from it.
const css = readFileSync(join(import.meta.dirname, 'promptBand.css'), 'utf8')
const band = blockAfter(css, '@utility strip-band')

/** The kinds of the dev table and what each puts in the band's middle. */
const KINDS: Array<[string, BandChoices]> = [
  ['priority', 'none'],
  ['yesno', 'none'],
  ['x', 'inline'],
  ['targets', 'wide'],
  ['cards', 'wide'],
  ['mode-two', 'wide'],
  ['order', 'wide'],
  ['pay', 'wide'],
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

describe('the band rules', () => {
  it.each(KINDS)(
    'keeps the answers at the right end of the sentence row for %s at every width',
    (_kind, choices) => {
      for (const rem of [24, 64, 90, 110, 150]) {
        const rows = bandAreas(rem, choices)
        const withText = must(rows.find((row) => row.includes('text') || row.includes('actions')))
        const answers = must(rows.find((row) => row.includes('actions')))
        expect(answers.at(-1)).toBe('actions')
        if (rem >= BAND_PHONE_REM) expect(withText).toBe(answers)
      }
    },
  )

  it('puts choices of any size on a line of their own, cards beside the sentence from 100 rem; short ones always beside', () => {
    expect(bandAreas(BAND_WIDE_REM - 1, 'wide')).toHaveLength(2)
    expect(bandAreas(BAND_WIDE_REM, 'wide')).toHaveLength(2)
    expect(bandAreas(BAND_WIDE_REM - 1, 'wide', { cards: true })).toHaveLength(2)
    expect(bandAreas(BAND_WIDE_REM, 'wide', { cards: true })).toHaveLength(1)
    expect(bandAreas(BAND_PHONE_REM, 'inline')).toEqual([
      ['stack', 'status', 'text', 'choices', 'clock', 'actions'],
    ])
    expect(bandAreas(BAND_WIDE_REM, 'none')).toEqual([['stack', 'status', 'text', 'clock', 'actions']])
  })

  it('keeps the sentence to one line, three only beside cards in a wide band', () => {
    expect(sentenceLines(150, 'none')).toBe(1)
    expect(sentenceLines(150, 'inline')).toBe(1)
    expect(sentenceLines(90, 'wide', true)).toBe(1)
    expect(sentenceLines(150, 'wide')).toBe(1)
    expect(sentenceLines(150, 'wide', true)).toBe(3)
  })

  it('tells a cut sentence from one that fits', () => {
    expect(isClamped({ scrollHeight: 20, clientHeight: 20, scrollWidth: 300, clientWidth: 300 })).toBe(false)
    expect(isClamped({ scrollHeight: 60, clientHeight: 20, scrollWidth: 300, clientWidth: 300 })).toBe(true)
    expect(isClamped({ scrollHeight: 20, clientHeight: 20, scrollWidth: 500, clientWidth: 300 })).toBe(true)
  })
})

describe('the stylesheet', () => {
  it('draws the areas the rules name', () => {
    expect(areasIn(band)).toEqual(bandAreas(64, 'none'))
    expect(areasIn(band, "&:has([data-slot='prompt-choices']:not(:empty))")).toEqual(
      bandAreas(64, 'wide', { cards: true }),
    )
    expect(areasIn(band, `@container band (width >= ${BAND_WIDE_REM}rem)`)).toEqual(
      bandAreas(BAND_WIDE_REM, 'wide', { cards: true }),
    )
    expect(areasIn(band, "&:has([data-slot='prompt-choices'][data-inline]:not(:empty))")).toEqual(
      bandAreas(64, 'inline'),
    )
    expect(areasIn(band, `@container band (width < ${BAND_PHONE_REM}rem)`)).toEqual(bandAreas(24, 'wide'))
    const mid = blockAfter(band, `@container band (width >= ${BAND_PHONE_REM}rem)`)
    expect(areasIn(mid, '&:not([data-cards])')).toEqual(bandAreas(64, 'wide'))
    expect(areasIn(mid, "[data-slot='strip-status']:not(:empty)")).toEqual(
      bandAreas(64, 'wide', { status: true }),
    )
  })

  it('gives the answers of a prompt with choices the first line from 1024 px: the status starts the choices line', () => {
    for (const status of [false, true]) {
      const [first, second] = bandAreas(64, 'wide', { status })
      expect(first).toEqual(['stack', 'text', 'clock', 'actions'])
      expect(second?.[0]).toBe(status ? 'status' : 'choices')
    }
    // Cards keep the whole line under the head row from 64 rem; under it they give the first line to the answers too.
    expect(bandAreas(64, 'wide', { status: true, cards: true })[1]).toEqual(Array(5).fill('choices'))
    for (const status of [false, true]) {
      expect(bandAreas(50, 'wide', { status, cards: true })).toEqual(bandAreas(50, 'wide', { status }))
    }
    const cardsNarrow = blockAfter(
      band,
      `@container band (${BAND_PHONE_REM}rem <= width < ${BAND_NARROW_REM}rem)`,
    )
    expect(areasIn(cardsNarrow, "&[data-cards]:has([data-slot='prompt-choices']")).toEqual(
      bandAreas(50, 'wide', { cards: true }),
    )
    expect(areasIn(cardsNarrow, "[data-slot='strip-status']:not(:empty)")).toEqual(
      bandAreas(50, 'wide', { status: true, cards: true }),
    )
    const narrow = blockAfter(band, `@container band (width < ${BAND_NARROW_REM}rem)`)
    expect(blockAfter(narrow, "[data-slot='prompt-actions'] [data-slot='undo-rest']")).toMatch(
      /display:\s*none/,
    )
    expect(narrow).toMatch(/\[data-slot='prompt-actions'\] \[data-slot='key-hint'\],/)
  })

  it('clamps the sentence to one line, and to three only in a wide band beside cards', () => {
    expect(blockAfter(css, ".strip-band [data-slot='prompt-sentence'] .prompt-sentence-clamp")).toMatch(
      /line-clamp:\s*1;/,
    )
    const wide = blockAfter(
      css,
      `@container band (width >= ${BAND_WIDE_REM}rem) {\n  .strip-band[data-cards]`,
    )
    expect(wide).toMatch(/:not\(\[data-inline\]\)/)
    expect(wide).toMatch(/line-clamp:\s*3;/)
  })

  it('lays the answers out right to left, so the first, the primary, sits at the right end', () => {
    const answers = blockAfter(css, '@utility prompt-answers')
    expect(answers).toMatch(/flex-direction:\s*row-reverse/)
    expect(answers).toMatch(/justify-content:\s*flex-start/)
  })

  it('wraps the answers downward, so a short sheet that cuts them keeps the primary on the first line', () => {
    const answers = blockAfter(css, '@utility prompt-answers')
    expect(answers).toMatch(/flex-wrap:\s*wrap;/)
    expect(answers).not.toMatch(/wrap-reverse/)
  })

  it('keeps a narrow band\'s stack room for the top chip: the label to a reader and "+2 more" as "+2"', () => {
    const narrow = blockAfter(band, '@container band (width < 64rem)')
    expect(blockAfter(narrow, "[data-slot='stack-label']")).toMatch(/clip-path:\s*inset\(50%\)/)
    expect(blockAfter(narrow, "[data-slot='stack-more-word']")).toMatch(/display:\s*none/)
  })

  it('gives a phone answers a row of their own across the band, opaque over the choices that scroll under them', () => {
    const phone = bandAreas(BAND_PHONE_REM - 1, 'wide')
    expect(phone.at(-1)).toEqual(['actions', 'actions'])
    expect(phone.flat().filter((cell) => cell === 'actions')).toHaveLength(2)
    const rule = blockAfter(
      band.slice(band.lastIndexOf(`@container band (width < ${BAND_PHONE_REM}rem)`)),
      "[data-slot='prompt-actions']",
    )
    expect(rule).toMatch(/background:\s*var\(--surface\)/)
  })
})

describe('the band in the strip', () => {
  it.each(KINDS)(
    'draws %s as cells of one grid, the sentence one line and the primary answer first',
    (kind, choices) => {
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
      const answers = line.querySelector<HTMLElement>('[data-slot="prompt-actions"]')
      if (kind === 'priority') return
      expect(answers).toHaveClass('prompt-answers')
      // The primary answer comes first, so the right-to-left row puts it at the right end and Tab reaches it first.
      const first = must(answers?.querySelector('button'))
      expect(first).toHaveAccessibleName(/^(Confirm|Choose|Yes)/)
    },
  )

  it('keeps the stack and the status as cells beside the band, and the keep-clear mark on the sheet', () => {
    owe('targets')
    const row = must(document.querySelector<HTMLElement>('[data-slot="strip-row"]'))
    expect([...row.children].map((child) => child.getAttribute('data-slot'))).toEqual([
      'strip-stack',
      'strip-prompt',
      'strip-status',
    ])
    expect(must(document.querySelector('[data-slot="shared-sheet"]'))).toHaveClass('band-room')
    expect(document.querySelector('[data-slot="shared-sheet"]')).toHaveAttribute('data-keep-clear')
  })

  it('leaves out "You have priority" beside the seat\'s own prompt, which says it already', () => {
    owe('priority')
    expect(screen.getByRole('region', { name: 'Turn and priority' })).not.toHaveTextContent(/has priority/)
  })
})

describe('the sheet floor', () => {
  it.each([
    [1830, 855],
    [2550, 1380],
    [1400, 900],
    [1280, 720],
  ])(
    'never squeezes the band under a sentence and a row of answers over an empty board at %ix%i',
    (_width, height) => {
      // Nothing on the opponents' boards: their row is down to its pods, which a sheet may not cover.
      const most = sheetMost(height, 56, 40, false, 120)
      expect(most).toBeGreaterThanOrEqual(BAND_MIN_PX)
      expect(most).toBeLessThanOrEqual((height * SHEET_MAX_VH) / 100)
    },
  )

  it('keeps a phone its own floor, with the stack chips on a line of their own', () => {
    expect(sheetMost(844, 56, 100, true)).toBe(BAND_MIN_PHONE_PX)
  })

  it('still grows to its room where the boards leave it, and never past 40 % of the window', () => {
    expect(sheetMost(900, 63, 400, false, 120)).toBe(343)
    expect(sheetMost(900, 63, 900, false)).toBe(360)
  })
})

describe("a payment's band", () => {
  it('cuts a status line that does not fit, whole in its tooltip, so it never runs into the sentence beside it', () => {
    const fixture = buildFixture({ seats: 2 })
    const pending = must(DEV_PROMPTS['pay'])(fixture)
    const { container } = renderTable({
      fixture,
      state: (state) => ({
        ...state,
        pending,
        view:
          state.view === null || state.view.priority === null
            ? state.view
            : { ...state.view, priority: { ...state.view.priority, holder: 1 } },
      }),
    })
    const status = must(container.querySelector<HTMLElement>('[data-slot="strip-status"]'))
    expect(status).toHaveClass('min-w-0', 'overflow-hidden')
    const line = must(status.querySelector<HTMLElement>('[data-slot="priority"]'))
    expect(line.textContent).not.toBe('')
    expect(line.title).toBe(line.textContent)
    expect(status.className).toContain('[&_[data-slot=priority]]:truncate')
    // The payment's choices take a line of their own at every width from a tablet up, never a column beside the sentence.
    for (const rem of [50, 90, 120, 160]) expect(bandAreas(rem, 'wide', { status: true })).toHaveLength(2)
  })
})
