// Goal: every page sits in one centred column of one of three widths with the same gutters, so pages cannot drift apart in width again.
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { PageShell } from './PageShell'
import type { PageWidth } from './PageShell'

const WIDTHS: ReadonlyArray<[PageWidth, string]> = [
  ['narrow', 'max-w-narrow'],
  ['page', 'max-w-page'],
  ['wide', 'max-w-wide'],
]

describe('PageShell', () => {
  it.each(WIDTHS)('keeps the %s width to %s', (width, expected) => {
    const { container } = render(<PageShell width={width}>x</PageShell>)
    const page = container.firstElementChild
    expect(page).toHaveClass(expected, 'mx-auto', 'w-full', 'px-4', 'sm:px-6')
    expect(page).toHaveAttribute('data-width', width)
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
