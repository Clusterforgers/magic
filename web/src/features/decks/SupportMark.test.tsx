// Goal: a support mark says its state in a word beside an icon of its own and an edge in its own colour, hides the icon from a screen reader, and no two states share a word, an icon or an edge, so colour never carries the meaning alone.
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { Support } from '@/protocol/generated'
import { SUPPORT_LABEL } from './format'
import { SupportMark, supportEdge } from './SupportMark'

const STATES: readonly Support[] = ['verified', 'partial', 'manual', 'stale', 'unplayable']

function mark(support: Support): HTMLElement {
  const { container } = render(<SupportMark support={support} />)
  const found = container.querySelector<HTMLElement>('[data-support]')
  if (found === null) throw new Error('no mark')
  return found
}

describe('SupportMark', () => {
  it.each(STATES)(
    'shows %s as its word, an icon hidden from a screen reader and its own colours',
    (support) => {
      const found = mark(support)
      expect(found).toHaveAttribute('data-support', support)
      expect(found).toHaveTextContent(new RegExp(`^${SUPPORT_LABEL[support]}$`))
      expect(found.querySelector('svg')).toHaveAttribute('aria-hidden', 'true')
      for (const part of ['border', 'bg', 'text']) {
        expect(found.className, part).toContain(`${part}-support-${support}`)
      }
    },
  )

  it('gives the five states five words, five icons and five edges', () => {
    const icons = STATES.map((support) => mark(support).querySelector('svg')?.getAttribute('class'))
    expect(icons.every((icon) => (icon ?? '').includes('lucide-'))).toBe(true)
    expect(new Set(icons).size).toBe(STATES.length)
    expect(new Set(STATES.map((support) => SUPPORT_LABEL[support])).size).toBe(STATES.length)
    expect(new Set(STATES.map(supportEdge)).size).toBe(STATES.length)
  })

  it.each(STATES)('draws the edge of a %s line in the colour of its state', (support) => {
    expect(supportEdge(support)).toBe(`border-support-${support}`)
  })

  it('takes a class for placement', () => {
    const { container } = render(<SupportMark support="manual" className="ml-2" />)
    expect(container.querySelector('[data-support]')).toHaveClass('ml-2')
  })
})
