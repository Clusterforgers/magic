// Goal: each clause a draft words differently stands out as one row with the printed clause beside the game's words, in order, and a card without such clauses draws nothing.
import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ReadDiffers } from '@/protocol/generated'
import { DiffersList } from './DiffersList'
import { DIFFERS_ENGINE, DIFFERS_PRINTED, DIFFERS_TITLE } from './model'

const row = (start: number, printed: string, written: string): ReadDiffers => ({
  face: 0,
  span: { start, end: start + printed.length },
  printed,
  written,
})

describe('DiffersList', () => {
  it('shows each differing clause as a row with the printed words beside the game’s words', () => {
    render(
      <DiffersList
        differs={[
          row(0, 'Deal 3 damage to any target.', 'Deal 2 damage to any target.'),
          row(30, 'Draw a card.', 'Draw two cards.'),
        ]}
      />,
    )

    const section = screen.getByRole('region', { name: DIFFERS_TITLE })
    const rows = within(section).getAllByRole('listitem')
    expect(rows).toHaveLength(2)
    expect(rows.map((item) => item.textContent)).toEqual([
      `${DIFFERS_PRINTED}“Deal 3 damage to any target.”${DIFFERS_ENGINE}“Deal 2 damage to any target.”`,
      `${DIFFERS_PRINTED}“Draw a card.”${DIFFERS_ENGINE}“Draw two cards.”`,
    ])
    expect(rows[0]).toHaveClass('border-support-partial')
  })

  it('draws nothing for a draft that reads back', () => {
    const { container } = render(<DiffersList differs={[]} />)
    expect(container).toBeEmptyDOMElement()
  })
})
