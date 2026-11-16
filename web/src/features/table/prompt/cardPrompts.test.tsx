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
import { CARD_PROMPTS, laysOutCards } from './cardPrompts'
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
