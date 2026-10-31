// Goal: a card that runs on a player's read shows a mark with the word "Read" (the icon alone on a small face) in its own colour, hidden from a screen reader and explained by a tooltip; a card that does not run on one shows nothing.
import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { card } from '@/test-utils/cards'
import { ReadMark } from './ReadMark'

function mark(read: boolean | undefined, compact = false, spot: 'corner' | 'chip' = 'corner') {
  const object = card(read === undefined ? {} : { read })
  const { container } = render(<ReadMark object={object} spot={spot} compact={compact} text="text-xs" />)
  return container.querySelector<HTMLElement>('[data-slot="card-read"]')
}

describe('ReadMark', () => {
  it('says Read beside an icon hidden from a screen reader, in the colour of the state', () => {
    const found = mark(true)
    expect(found).toHaveTextContent(/^Read$/)
    expect(found).toHaveAttribute('aria-hidden', 'true')
    expect(found).toHaveAttribute('title', "runs on a player's read")
    expect(found?.className).toContain('bg-support-read')
    expect(found?.querySelector('svg')).toHaveAttribute('aria-hidden', 'true')
  })

  it('keeps the icon alone on a small face', () => {
    const found = mark(true, true)
    expect(found).toHaveTextContent(/^$/)
    expect(found?.querySelector('svg')).not.toBeNull()
  })

  it.each([
    ['corner', 'border border-background'],
    ['chip', 'px-2'],
  ] as const)('sits as a %s mark', (spot, classes) => {
    expect(mark(true, false, spot)).toHaveAttribute('data-spot', spot)
    expect(mark(true, false, spot)?.className).toContain(classes)
  })

  it.each([
    ['an object with read false', false],
    ['an object that leaves the member out', undefined],
  ])('shows nothing for %s', (_what, read) => {
    expect(mark(read)).toBeNull()
  })
})
