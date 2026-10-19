// Goal: every page sits in one centred column of one of three widths with the same gutters, and the three widths are the ascending tokens of the stylesheet, so pages cannot drift apart in width again.
import { render } from '@testing-library/react'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { PAGE_WIDTHS } from './pageWidths'
import { PageShell } from './PageShell'
import type { PageWidth } from './PageShell'

const WIDTHS: ReadonlyArray<[PageWidth, string]> = [
  ['narrow', 'max-w-narrow'],
  ['page', 'max-w-page'],
  ['wide', 'max-w-wide'],
]

const STYLESHEET = readFileSync(join(import.meta.dirname, '..', 'index.css'), 'utf8')
const rem = (name: string): number => {
  const match = new RegExp(`--container-${name}:\\s*([0-9.]+)rem`).exec(STYLESHEET)
  return match?.[1] === undefined ? Number.NaN : Number(match[1])
}

describe('PageShell', () => {
  it.each(WIDTHS)('keeps the %s width to %s', (width, expected) => {
    const { container } = render(<PageShell width={width}>x</PageShell>)
    const page = container.firstElementChild
    expect(page).toHaveClass(expected, 'mx-auto', 'w-full', 'px-4', 'sm:px-6')
    expect(page).toHaveAttribute('data-width', width)
  })

  it('offers exactly the widths of the scale, narrowest first', () => {
    expect(PAGE_WIDTHS).toEqual(WIDTHS.map(([width]) => width))
  })

  it('has a token in the stylesheet for each width, each wider than the one before', () => {
    const sizes = PAGE_WIDTHS.map(rem)
    expect(sizes.every(Number.isFinite)).toBe(true)
    expect(sizes).toEqual([...sizes].sort((a, b) => a - b))
    expect(new Set(sizes).size).toBe(PAGE_WIDTHS.length)
  })

  it('is the page width unless told otherwise', () => {
    const { container } = render(<PageShell>x</PageShell>)
    expect(container.firstElementChild).toHaveAttribute('data-width', 'page')
  })

  it('lets a page add to its classes and attributes', () => {
    const { container } = render(
      <PageShell className="grid gap-6" id="here">
        x
      </PageShell>,
    )
    expect(container.firstElementChild).toHaveClass('grid', 'gap-6', 'max-w-page')
    expect(container.firstElementChild).toHaveAttribute('id', 'here')
  })
})
