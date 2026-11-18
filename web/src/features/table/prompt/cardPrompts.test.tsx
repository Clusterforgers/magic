// Goal: a prompt that lays out cards takes the strip's whole width and larger cards in a sheet that may grow past the band over the opponents' boards, always leaving their pods and a row of their cards and never covering your board; a player offered as a target shows their seat's face.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { initialPlayState } from '@/play/store'
import { decisionFor } from '@/test-utils/decisions'
import { renderTable } from '@/test-utils/tableHarness'
import { regionSizes, SHEET_KEEP_PX } from '../layout'
import { BOARD_KEEP_PX, withSplit } from '../boardSplit'
import {
  BAND_HEAD_REM,
  CARD_PROMPTS,
  laysOutCards,
  QUICK_ROW_REM,
  STRIP_WRAP_FROM,
  stripCardWidth,
  stripFit,
  stripRows,
} from './cardPrompts'
import { PROMPT_CARD_BOX } from './PromptCard'
import { buildFixture } from '../dev/fixture'
import { DEV_PROMPTS } from '../dev/prompts'

describe('laysOutCards', () => {
  it.each([
    ['no decision', null, false],
    ['priority', 'Priority', false],
    ['a mode beside its card', 'ChooseMode', true],
    ['an option', 'ChooseOption', false],
    ['a choice of cards', 'ChooseCards', true],
    ['targets', 'ChooseTargets', true],
    ['a scry', 'Scry', true],
    ['a mulligan', 'Mulligan', true],
  ] as const)('reads %s as %s', (_, kind, cards) => {
    const pending = kind === null ? null : { id: 2, decision: decisionFor(kind) }
    expect(laysOutCards({ ...initialPlayState(), pending })).toBe(cards)
  })

  it('names only decision kinds', () => {
    expect([...CARD_PROMPTS].every((name) => /^[A-Z][A-Za-z]+$/.test(name))).toBe(true)
  })
})

describe('the wide sheet', () => {
  it("reaches over the opponents' boards past the band, leaving their pods and a row of their cards", () => {
    const regions = regionSizes({ width: 1400, height: 900 })
    const keep = 120 + BOARD_KEEP_PX
    const split = withSplit(regions, { opponents: 420, own: 300, ownCard: 0, theirCard: 0 }, keep)
    expect(split.sheet).toBeLessThanOrEqual(regions.shared + regions.sheetRoom)
    expect(split.sheetWide).toBeGreaterThan(split.sheet)
    expect(split.sheetWide).toBeLessThanOrEqual(regions.main.height * 0.4)
    expect(420 - (split.sheetWide - regions.shared)).toBeGreaterThanOrEqual(keep - 1e-9)
    expect(SHEET_KEEP_PX).toBeGreaterThan(0)
  })

  it("gives a row of cards or the opening hand the strip's whole width below a wide band", () => {
    const band = readFileSync(join(import.meta.dirname, 'promptBand.css'), 'utf8')
    expect(band).toContain("'choices choices choices choices choices'")
  })

  it('draws a whole card up to 7 rem wide, following the window height', () => {
    expect(PROMPT_CARD_BOX.normal).toContain('min(7rem,11dvh,calc((var(--choice-room,20rem)_-_2rem)*5/7))')
    // In the band's row of cards the row's own fit wins (`stripCardWidth`).
    expect(PROMPT_CARD_BOX.normal).toContain('[--card-w:var(--choice-fit,')
  })
})

/** The band's room for its row of cards at 1400x900 on a grid of six: with the side panel open, and folded. */
const GRID_1400 = { room: 233 - 16 * (BAND_HEAD_REM + QUICK_ROW_REM), width: 704, windowHeight: 900 }
const GRID_1400_FOLDED = { ...GRID_1400, room: 265 - 16 * BAND_HEAD_REM, width: 936 }
/** A tall band beside a wide strip (a duel at 1920x1200). */
const TALL = { room: 420 - 16 * BAND_HEAD_REM, width: 900, windowHeight: 1200 }

describe('a row of cards in the band', () => {
  it.each([1, 4, 6, 8, 9, 12])('never runs %i cards past the row or the room, at 3 rem or more', (count) => {
    for (const room of [GRID_1400, GRID_1400_FOLDED, TALL]) {
      const { card, rows } = stripFit(count, room)
      const per = Math.ceil(count / rows)
      expect(card).toBeGreaterThanOrEqual(48)
      expect(card).toBeLessThanOrEqual(112)
      // Each row fits the row's width inside its padding, and the rows fit the room's height.
      expect(per * card + (per - 1) * 8).toBeLessThanOrEqual(room.width - 12)
      expect(rows * card * (7 / 5) + (rows - 1) * 8).toBeLessThanOrEqual(room.room - 32 + 1e-9)
    }
  })

  it('keeps eight cards or fewer on one row and wraps more onto two where that makes them larger', () => {
    for (const count of [1, 5, 8]) {
      expect(stripRows(count)).toEqual([1])
      expect(stripFit(count, TALL).rows).toBe(1)
    }
    expect(stripRows(STRIP_WRAP_FROM - 1)).toEqual([1])
    expect(stripRows(STRIP_WRAP_FROM)).toEqual([1, 2])
    // A tall band holds twelve cards larger on two rows than on one.
    const two = stripFit(12, TALL)
    expect(two.rows).toBe(2)
    expect(two.card).toBeGreaterThan(stripFit(12, { ...TALL, room: 0 }).card)
    // At 1400x900 two rows would fall under the floor: the twelve stand on one row, each whole.
    expect(stripFit(12, GRID_1400).rows).toBe(1)
  })

  it('draws the six cards of a target prompt at 1400x900 larger than before, beside a column of players', () => {
    // The players' columns take about 10 rem of the row (`player-targets`).
    const { card, rows } = stripFit(6, { ...GRID_1400, width: GRID_1400.width - 165 })
    expect(rows).toBe(1)
    expect(card).toBeGreaterThan(70)
  })

  it('writes the CSS the px mirror reads: one row up to eight cards, the larger of one or two rows from nine', () => {
    expect(stripCardWidth(6)).not.toContain('max(min(')
    expect(stripCardWidth(6)).toContain('(100cqi - 2.5rem) / 6')
    const twelve = stripCardWidth(12)
    expect(twelve).toMatch(/^max\(3rem, max\(min\(/)
    expect(twelve).toContain('(100cqi - 5.5rem) / 12')
    expect(twelve).toContain('(100cqi - 2.5rem) / 6')
    expect(twelve).toContain('- 0.5rem) / 2 * 5 / 7')
  })
})

describe('a player offered as a target', () => {
  it('shows the face of the seat beside the name', () => {
    const fixture = buildFixture({ seats: 4 })
    const pending = must(DEV_PROMPTS.targets)(fixture)
    renderTable({ fixture, state: (state) => ({ ...state, pending }) })
    for (const name of ['Bo', 'Cy', 'Dee']) {
      const button = screen.getByRole('button', { name })
      expect(button.querySelector('[data-slot="player-face"]')).not.toBeNull()
    }
  })
})

function must<T>(value: T | undefined): T {
  if (value === undefined) throw new Error('missing')
  return value
}
