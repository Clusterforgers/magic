// Goal: a deck row asks for its pictures when it is in view, only as many at once as the queue allows, and shows a tile where it has no card to ask for.
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { createImageQueue } from '@/features/cards/imageQueue'
import { entryOfBuiltIn } from './deckEntries'
import type { DeckEntry } from './deckEntries'
import { DeckPictures } from './DeckPictures'

const entry = (n: number, pictures: number, set: string | null = null): DeckEntry => ({
  ...entryOfBuiltIn({ id: `d${n}`, name: `Deck ${n}`, size: 60 } as never),
  pictures: Array.from({ length: pictures }, (_, i) => ({ oracleId: `o-${n}-${i}`, name: `Card ${n}-${i}` })),
  set,
})

describe('the pictures of a deck row', () => {
  it.each([
    [2, 6, 2, 4],
    [4, 3, 3, 0],
    [1, 2, 1, 1],
  ])('with %i slots and %i pictures asks for %i and queues %i', (slots, rows, active, waiting) => {
    const queue = createImageQueue(slots)
    render(
      <>
        {Array.from({ length: rows }, (_, n) => (
          <DeckPictures key={n} entry={entry(n, 1)} fallback={[]} queue={queue} />
        ))}
      </>,
    )
    expect(queue.active).toBe(active)
    expect(queue.waiting).toBe(waiting)
    expect(document.querySelectorAll('[data-slot="deck-picture-wait"]').length).toBe(waiting)
  })

  it('shows at most two pictures for a deck, the fallback where the list knows none', () => {
    const { container } = render(
      <DeckPictures
        entry={entry(1, 0)}
        fallback={[0, 1, 2].map((i) => ({ oracleId: `f${i}`, name: `F${i}` }))}
        queue={createImageQueue(8)}
      />,
    )
    expect(container.querySelector('[data-slot="deck-pictures"]')?.getAttribute('data-count')).toBe('2')
  })

  it.each([
    ['the set of a precon', 'c19', 'C19'],
    ['an icon for a stored deck', null, ''],
  ])('shows a tile with %s when no card is known', (_what, set, text) => {
    render(<DeckPictures entry={entry(1, 0, set)} fallback={[]} queue={createImageQueue(2)} />)
    const tile = document.querySelector('[data-slot="deck-tile"]')
    expect(tile).not.toBeNull()
    expect(tile?.textContent).toBe(text)
    expect(screen.queryByRole('img')).toBeNull()
  })
})
