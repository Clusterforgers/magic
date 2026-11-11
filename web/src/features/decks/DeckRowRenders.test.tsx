// Goal: the rows of a long deck list are drawn once per load: opening and closing a dialog over the list, or a reload that brings the same decks, draws no row again.
import { screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { deckItem } from './fakeDecks'
import { deckRows, listed, renderDecks } from './renderDecks'

// Every row draws exactly one marker, so the marker's draws count the rows'.
const draws = vi.hoisted(() => ({ count: 0 }))
vi.mock('./DeckMarker', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./DeckMarker')>()
  return {
    ...actual,
    DeckMarker: (props: Parameters<typeof actual.DeckMarker>[0]) => {
      draws.count += 1
      return actual.DeckMarker(props)
    },
  }
})

afterEach(() => {
  draws.count = 0
})

const DECKS = Array.from({ length: 40 }, (_, i) => deckItem({ id: `deck-${i}`, name: `Deck ${i}` }))

describe('a list of 40 decks', () => {
  it('draws each row once, and not again for a dialog opened and closed over it', async () => {
    const app = await renderDecks({ decks: DECKS })
    await listed()
    expect(deckRows()).toHaveLength(40)
    const drawn = draws.count
    expect(drawn).toBeGreaterThanOrEqual(40)
    expect(drawn).toBeLessThanOrEqual(80)

    await app.user.click(screen.getByRole('button', { name: 'New deck' }))
    expect(await screen.findByRole('dialog')).toBeInTheDocument()
    await app.user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())

    await app.user.click(screen.getByRole('button', { name: 'Delete Deck 3' }))
    expect(await screen.findByRole('alertdialog')).toBeInTheDocument()
    await app.user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull())

    expect(draws.count).toBe(drawn)
  })
})
