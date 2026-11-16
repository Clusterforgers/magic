// Goal: a phone held upright keeps its table to the essentials: the combat answers keep their words, the controls dock shows the auto-skip switch without its label (the switch keeps its name), the pod drops the untapped sources the lands already show, the hand dock is 104 to 140 px, the boards fold their lands into chips and draw no card under the phone floor, the strip keeps to two lines, and a row of choice cards never widens the sheet past the screen.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'
import { HAND_PHONE_CLAMP } from './layout'
import { must } from '@/test-utils/batches'

const read = (path: string): string => readFileSync(join(import.meta.dirname, path), 'utf8')

describe('the phone table', () => {
  it('keeps the word of every combat answer on a phone: Attack and No attack, Block and No blocks', () => {
    for (const [file, labels] of [
      ['prompt/kinds/DeclareAttackers.tsx', ['Confirm attack', 'No attack']],
      ['prompt/kinds/DeclareBlockers.tsx', ['Confirm blocks', 'No blocks']],
    ] as const) {
      const source = read(file)
      for (const label of labels) {
        const tag = must(source.split('<BarButton').find((part) => part.includes(`label="${label}"`)))
        expect(tag.slice(0, tag.indexOf('/>'))).toMatch(/\bkeepWord\b/)
      }
    }
  })

  it('names the auto-skip switch for a screen reader and hides its label on a phone', () => {
    const toggle = read('stops/AutoSkipToggle.tsx')
    expect(toggle).toContain("cn('max-sm:sr-only'")
    expect(toggle).toMatch(/aria-label="Auto-skip triggers"/)
  })

  it('drops the untapped sources from a one-line pod on a phone only', () => {
    const { container } = renderTable({ seats: 2, options: { viewport: { width: 390, height: 844 } } })
    const sources = [...container.querySelectorAll('[data-variant="line"] [data-slot="mana-sources"]')]
    expect(sources.length).toBeGreaterThan(0)
    for (const element of sources) expect(element).toHaveClass('max-sm:hidden')
  })

  it('gives the hand dock 104 to 140 px', () => {
    expect(HAND_PHONE_CLAMP).toEqual({ min: 104, vh: 16, max: 140 })
  })

  it('folds the lands of the focused board and of yours into chips, so the boards keep their room for the rest', () => {
    const { container } = renderTable({
      seats: 4,
      options: { viewport: { width: 390, height: 844 }, foldLands: true },
    })
    expect(container.querySelector('[data-slot="board-lands"] [data-slot="land-summary"]')).not.toBeNull()
    const own = must(container.querySelector('[data-seat-area="0"]'))
    expect(own.querySelector('[data-slot="land-summary"]')).not.toBeNull()
    expect(container.querySelector('[data-lane="lands"]')).toBeNull()
  })

  it('keeps the strip to two lines without a prompt: the stack, then the waiting line beside the note', () => {
    const strip = read('SharedStrip.tsx')
    expect(strip).toContain('max-sm:has-data-[slot=stack-chips]:basis-full')
    expect(strip).toContain('max-sm:grow max-sm:basis-0')
  })

  it('keeps a row of choice cards inside the sheet', () => {
    expect(read('prompt/CardChoices.tsx')).toContain('max-w-full min-w-0 basis-full items-center')
  })
})
