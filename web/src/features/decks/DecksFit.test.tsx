// Goal: from the sm breakpoint up the decks page is exactly as tall as the window under the header and only its list region scrolls, so the page never scrolls at 1280x720; under sm it is an ordinary page; the scrolling region keeps its gutter and every row can take the focus inside it.
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { deckItem } from './fakeDecks'
import { deckList, renderDecks } from './renderDecks'

const MANY = Array.from({ length: 30 }, (_, index) =>
  deckItem({ id: `deck-${index}`, name: `Deck ${index}` }),
)

describe('the fit of the page', () => {
  it('is as tall as the window under the header from sm up, a column that gives the rest to the list', async () => {
    await renderDecks({ decks: MANY })
    await screen.findByRole('list', { name: 'Your decks' })
    const page = document.querySelector('[data-slot="page"]')
    // The header is 3.5rem and a 1px rule (`AppShell`); the page takes the rest of the window.
    expect(page).toHaveClass('sm:h-[calc(100dvh-3.5rem-1px)]', 'sm:flex', 'sm:flex-col')
    const section = screen.getByRole('region', { name: 'Decks' })
    expect(section).toHaveClass('flex', 'flex-col', 'min-h-0', 'flex-1')
  })

  it('scrolls only the list region from sm up, and keeps its gutter so rows do not change width', async () => {
    await renderDecks({ decks: MANY })
    await screen.findByRole('list', { name: 'Your decks' })
    const region = deckList().parentElement
    expect(region).toHaveClass('min-h-0', 'flex-1', 'sm:overflow-y-auto', 'sm:[scrollbar-gutter:stable]')
    expect(region?.parentElement).toBe(screen.getByRole('region', { name: 'Decks' }))
  })

  it('draws the page and the list as ordinary blocks below sm, which never set a height or scroll', async () => {
    await renderDecks({ decks: MANY })
    await screen.findByRole('list', { name: 'Your decks' })
    const page = document.querySelector('[data-slot="page"]')
    const region = deckList().parentElement
    for (const element of [page, region]) {
      const unprefixed = (element?.className ?? '').split(/\s+/).filter((name) => !name.includes(':'))
      expect(unprefixed.filter((name) => /^(h-|max-h-|overflow-)/.test(name))).toEqual([])
    }
  })

  it('keeps every row of a long list reachable by keyboard through its own buttons', async () => {
    await renderDecks({ decks: MANY })
    await screen.findByRole('list', { name: 'Your decks' })
    expect(screen.getAllByRole('button', { name: /^Export Deck/ })).toHaveLength(MANY.length)
    expect(screen.getAllByRole('button', { name: /^Delete Deck/ })).toHaveLength(MANY.length)
  })
})
