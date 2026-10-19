// Goal: no text on a card face is under 11 px at any card size, so shrinking a zone to fit more permanents never shrinks the type; the sizes keep the 5:7 shape of a card.
import { describe, expect, it } from 'vitest'
import { MIN_TEXT_REM, SIZE } from './cardSize'
import type { CardSize } from './cardSize'

const NAMED_REM: Record<string, number> = { xs: 0.75, sm: 0.875, base: 1, lg: 1.125, xl: 1.25 }

/** The rem a Tailwind text class sets: `text-[0.72rem]` or a named size such as `text-xs`. */
function textRem(classes: string): number {
  const match = /(?:^|\s)text-(?:\[([\d.]+)rem\]|(xs|sm|base|lg|xl))(?:\s|$)/.exec(classes)
  if (match === null) throw new Error(`no text size in "${classes}"`)
  return match[1] === undefined ? (NAMED_REM[match[2] ?? ''] ?? 0) : Number(match[1])
}

const SIZES = Object.keys(SIZE) as CardSize[]

describe('the card sizes', () => {
  it.each(SIZES.flatMap((size) => (['name', 'text', 'pip'] as const).map((part) => [size, part] as const)))(
    'set the %s %s text at 11 px or more',
    (size, part) => {
      expect(textRem(SIZE[size][part])).toBeGreaterThanOrEqual(MIN_TEXT_REM)
    },
  )

  it.each(SIZES)('keep the 5:7 shape of a card at the %s size', (size) => {
    const match = /--card-w:([\d.]+)rem\].*--card-h:([\d.]+)rem\]/.exec(SIZE[size].vars)
    expect(match).not.toBeNull()
    expect(Number(match?.[2]) / Number(match?.[1])).toBeCloseTo(1.4, 5)
  })
})
