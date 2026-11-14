// Goal: a prompt that lays out cards takes the strip's whole width, larger cards and a sheet that may grow over the opponents' boards (never over yours), while priority and other prompts keep the narrower sheet; a player offered as a target shows their seat's face.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { initialPlayState } from '@/play/store'
import { decisionFor } from '@/test-utils/decisions'
import { renderTable } from '@/test-utils/tableHarness'
import { regionSizes, SHEET_KEEP_PX } from '../layout'
import { withSplit } from '../boardSplit'
import { CARD_PROMPTS, laysOutCards } from './cardPrompts'
import { PROMPT_CARD_BOX } from './PromptCard'
import { buildFixture } from '../dev/fixture'
import { DEV_PROMPTS } from '../dev/prompts'

describe('laysOutCards', () => {
  it.each([
    ['no decision', null, false],
    ['priority', 'Priority', false],
    ['a mode', 'ChooseMode', false],
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
  it('reaches over the opponents boards that the plain sheet keeps, and no further than the window allows', () => {
    const regions = regionSizes({ width: 1400, height: 900 })
    const split = withSplit(regions, { opponents: 320, own: 300, ownCard: 0, theirCard: 0 }, 200)
    expect(split.sheetWide).toBeGreaterThan(split.sheet)
    expect(split.sheetWide).toBeLessThanOrEqual(regions.main.height * 0.4)
    expect(SHEET_KEEP_PX).toBeGreaterThan(0)
  })

  it("gives a row of cards or the opening hand the strip's whole width", () => {
    const strip = readFileSync(join(import.meta.dirname, '../SharedStrip.tsx'), 'utf8')
    expect(strip).toContain('has-[[data-slot=card-choices]]:basis-full')
    expect(strip).toContain('has-[[data-slot=opening-hand]]:basis-full')
  })

  it('draws a whole card up to 7 rem wide, following the window height', () => {
    expect(PROMPT_CARD_BOX.normal).toContain('[--card-w:min(7rem,11dvh)]')
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
