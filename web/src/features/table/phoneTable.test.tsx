// Goal: a phone held upright keeps its table to the essentials: the controls dock shows the auto-skip switch without its label (the switch keeps its name), the pod drops the untapped sources the lands already show, the hand dock is 104 to 140 px, and a row of choice cards never widens the sheet past the screen.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'
import { HAND_PHONE_CLAMP } from './layout'

const read = (path: string): string => readFileSync(join(import.meta.dirname, path), 'utf8')

describe('the phone table', () => {
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

  it('keeps a row of choice cards inside the sheet', () => {
    expect(read('prompt/CardChoices.tsx')).toContain('max-w-full min-w-0 basis-full flex-col')
  })
})
