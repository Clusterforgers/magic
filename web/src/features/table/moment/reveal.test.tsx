// Goal: a reveal over the table is a bounded set of compact tiles, a picture and a one-line name each and no rules text, at any number of cards; the card's preview, shown by a hover or a click on its tile, holds the whole text.
import { act, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { renderTable } from '@/test-utils/tableHarness'
import { CardsApiContext } from '../cardsContext'
import { applyBatchToStore } from '../dev/applyBatch'
import { momentsBatch, revealApi, SHOWN } from '../dev/momentScene'
import { GameTable } from '../GameTable'
import { REVEAL_MAX_PX, REVEAL_MIN_PX } from './moments'

/** A table whose cards API answers with long rules text, and a reveal of `count` cards on it. */
async function revealOf(count: number) {
  const table = renderTable({
    seats: 2,
    options: { effects: true },
    ui: (
      <CardsApiContext value={revealApi}>
        <GameTable />
      </CardsApiContext>
    ),
  })
  const batch = momentsBatch(table.store.getState(), count)
  if (batch === null) throw new Error('no batch')
  act(() => void applyBatchToStore(table.store, batch))
  // The rules text of every card is asked for and answered, so a card that drew it would show it by now.
  await act(async () => {
    await Promise.resolve()
  })
  const panel = screen.getByRole('region', { name: /reveals$/ })
  return { ...table, panel, tiles: [...panel.querySelectorAll<HTMLElement>('[data-slot="moment-card"]')] }
}

describe('the tiles of a reveal', () => {
  it.each([1, 3, 8, 12])('%i cards are tiles with no rules text under them', async (count) => {
    const { panel, tiles } = await revealOf(count)
    expect(tiles).toHaveLength(count)
    expect(panel.querySelector('[data-slot="preview-oracle"]')).toBeNull()
    for (const spec of SHOWN) expect(panel.textContent).not.toContain(spec.text.slice(0, 24))
    for (const tile of tiles) expect(tile.querySelector('p')).toBeNull()
  })

  it.each([3, 8])('%i cards have one name line each, cut with an ellipsis', async (count) => {
    const { tiles } = await revealOf(count)
    tiles.forEach((tile, at) => {
      const names = tile.querySelectorAll('[data-slot="moment-card-name"]')
      expect(names).toHaveLength(1)
      expect(names[0]).toHaveTextContent(SHOWN[at % SHOWN.length]?.name ?? '')
      expect(names[0]).toHaveClass('truncate', 'block')
      expect(names[0]).not.toHaveClass('whitespace-normal', 'break-words')
    })
  })

  it.each([1, 3, 8, 12])(
    '%i cards stand in a list that wraps and scrolls inside a bounded height',
    async (count) => {
      const { panel } = await revealOf(count)
      const list = panel.querySelector('[data-slot="moment-cards"]')
      expect(list).toHaveClass('flex-wrap', 'overflow-y-auto', 'overflow-x-hidden')
      expect(list?.className).toMatch(/\bmax-h-\[\d+dvh\]/)
      expect(panel.className).toMatch(/\bmax-h-\[calc\(100dvh-/)
    },
  )

  it.each([1, 3, 8, 12])(
    "%i cards are as wide as the table's own cards, each tile as wide as its picture",
    async (count) => {
      const { tiles } = await revealOf(count)
      for (const tile of tiles) {
        const width = Number.parseInt(tile.style.width, 10)
        expect(width).toBeGreaterThanOrEqual(REVEAL_MIN_PX)
        expect(width).toBeLessThanOrEqual(REVEAL_MAX_PX)
        expect(tile.querySelector<HTMLElement>('button')?.style.getPropertyValue('--card-w')).toBe(
          `${width}px`,
        )
      }
    },
  )
})

describe('the preview of a revealed card', () => {
  it('shows the whole rules text on a hover over its tile, though the view holds no such card', async () => {
    const { panel, user, store } = await revealOf(3)
    const tile = within(panel).getByRole('button', { name: `${SHOWN[1]?.name}, revealed` })
    expect(store.getState().objectsById[Number(tile.getAttribute('data-object-id'))]).toBeUndefined()
    await user.hover(tile)
    await act(async () => {
      await Promise.resolve()
    })
    const preview = screen.getByTestId('card-preview')
    expect(within(preview).getByText(SHOWN[1]?.name ?? '', { selector: 'p.font-semibold' })).toBeVisible()
    expect(preview.querySelector('[data-slot="preview-oracle"]')).toHaveTextContent(
      'Create three 1/1 white Soldier',
    )
    expect(preview).not.toHaveAttribute('data-pinned')
  })

  it('stays on a click and goes with a second one', async () => {
    const { panel, user } = await revealOf(3)
    const tile = within(panel).getByRole('button', { name: `${SHOWN[2]?.name}, revealed` })
    await user.click(tile)
    expect(screen.getByTestId('card-preview')).toHaveAttribute('data-pinned', 'true')
    await user.click(tile)
    expect(screen.queryByTestId('card-preview')).toBeNull()
  })
})
