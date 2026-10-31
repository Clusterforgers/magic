// Goal: the shared support bar draws one segment per state that has copies with the state's mark and count (Stale and Unplayable only when present), and the colour counts name each colour in words with its number, colourless included.
import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { counts } from '@/features/decks/fakeDecks'
import { ColourCounts, SupportBar } from './PreviewStats'

describe('SupportBar', () => {
  it('shows Verified, Partial and Manual always and the other states only when the deck has copies of them', () => {
    const { container, rerender } = render(<SupportBar counts={counts({ total: 10, verified: 10 })} />)
    const states = () =>
      [...container.querySelectorAll('[data-support-count]')].map((el) =>
        el.getAttribute('data-support-count'),
      )
    expect(states()).toEqual(['verified', 'partial', 'manual'])
    rerender(<SupportBar counts={counts({ total: 10, verified: 7, manual: 1, stale: 1, unplayable: 1 })} />)
    expect(states()).toEqual(['verified', 'partial', 'manual', 'stale', 'unplayable'])
    rerender(<SupportBar counts={counts({ total: 10, verified: 7, read: 3 })} />)
    expect(states()).toEqual(['verified', 'read', 'partial', 'manual'])
  })

  it('draws a segment as wide as the share of each state and none for a state without copies', () => {
    const { container } = render(<SupportBar counts={counts({ total: 10, verified: 8, manual: 2 })} />)
    const widths = [...container.querySelectorAll<HTMLElement>('[data-slot="support-bar"] > span')].map(
      (el) => el.style.width,
    )
    expect(widths).toEqual(['80%', '20%'])
  })

  it('draws a read segment beside the verified one and names its count', () => {
    const { container } = render(
      <SupportBar counts={counts({ total: 10, verified: 6, read: 3, manual: 1 })} />,
    )
    const widths = [...container.querySelectorAll<HTMLElement>('[data-slot="support-bar"] > span')].map(
      (el) => el.style.width,
    )
    expect(widths).toEqual(['60%', '30%', '10%'])
    const list = screen.getByRole('list', { name: 'Cards by support' })
    expect(
      within(list)
        .getAllByRole('listitem')
        .map((item) => item.textContent),
    ).toEqual(['Verified6', 'Read3', 'Partial0', 'Manual1'])
  })

  it('says the count beside each mark', () => {
    render(<SupportBar counts={counts({ total: 10, verified: 8, manual: 2 })} />)
    const list = screen.getByRole('list', { name: 'Cards by support' })
    expect(
      within(list)
        .getAllByRole('listitem')
        .map((item) => item.textContent),
    ).toEqual(['Verified8', 'Partial0', 'Manual2'])
  })
})

describe('ColourCounts', () => {
  it('names each colour and its copies in words, with one card or many, and the colourless', () => {
    render(
      <ColourCounts
        colours={[
          { letter: 'W', count: 1 },
          { letter: 'U', count: 12 },
          { letter: 'C', count: 3 },
        ]}
      />,
    )
    expect(screen.getByLabelText('White: 1 card')).toBeInTheDocument()
    expect(screen.getByLabelText('Blue: 12 cards')).toBeInTheDocument()
    expect(screen.getByLabelText('Colorless: 3 cards')).toBeInTheDocument()
  })

  it('puts a letter in each disc, so colour is not the only sign', () => {
    render(<ColourCounts colours={[{ letter: 'R', count: 2 }]} />)
    expect(screen.getByLabelText('Red: 2 cards')).toHaveTextContent('R2')
  })
})
