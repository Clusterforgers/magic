// Goal: the prompt's corner badge on a compact face narrower than NARROW_PX is a glyph of at most GLYPH_MAX characters that reads whole, its label in the tooltip and for a screen reader; from NARROW_PX the label is written out.
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { permanent } from '@/test-utils/cards'
import { TextFace } from './CardFace'
import { NARROW_PX } from './cardSize'
import { CardWidthContext } from './ImageFace'
import { GLYPH_MAX } from './narrowMarks'

/** Each badge kind a prompt writes, with what its glyph writes beside the icon. */
const KINDS: readonly (readonly [label: string, glyph: string])[] = [
  ['At Ana', ''],
  ['Pay {2}', '{2}'],
  ['Pay {2}{W}{W}', ''],
  ['Pay 2 life', '2'],
  ['Must attack', '!'],
  ['Must block', '!'],
  ['Must be blocked', '!'],
  ['Needs 2 more', '+2'],
  ['Needs 3+ blockers', '3+'],
  ['Ana', 'Ana'],
]

function badge(width: number, label: string): Element | null {
  const box = render(
    <CardWidthContext.Provider value={width}>
      <TextFace object={permanent({})} role={null} size="compact" note={{ label, words: label }} fit />
    </CardWidthContext.Provider>,
  ).container
  return box.querySelector('[data-slot=card-note]')
}

/** What a mark shows, its screen-reader words left out. */
function shown(mark: Element): string {
  return [...mark.children]
    .filter((child) => !child.classList.contains('sr-only') && child.tagName !== 'svg')
    .map((child) => child.textContent ?? '')
    .join('')
}

describe('narrow prompt badge', () => {
  it.each(
    [44, 60, NARROW_PX - 1].flatMap((width) => KINDS.map(([label, glyph]) => [width, label, glyph] as const)),
  )('at %i px draws "%s" as a glyph', (width, label, glyph) => {
    const mark = badge(width, label)
    expect(mark?.hasAttribute('data-glyph')).toBe(true)
    expect(mark?.getAttribute('title')).toBe(label)
    expect(mark?.querySelector('.sr-only')?.textContent).toBe(label)
    expect(mark?.className).toContain('shrink-0')
    expect(mark?.className).not.toContain('truncate')
    const text = mark === null ? '' : shown(mark)
    expect(text).toBe(glyph)
    expect(text.length).toBeLessThanOrEqual(GLYPH_MAX)
    // Every glyph says something visible: an icon or its text.
    expect(text !== '' || mark?.querySelector('svg') !== null).toBe(true)
  })

  it.each(KINDS.flatMap(([label]) => [NARROW_PX, 120].map((width) => [width, label] as const)))(
    'at %i px writes "%s" out',
    (width, label) => {
      const mark = badge(width, label)
      expect(mark?.hasAttribute('data-glyph')).toBe(false)
      expect(mark?.textContent).toBe(label.replace(/^Pay /, ''))
    },
  )
})
