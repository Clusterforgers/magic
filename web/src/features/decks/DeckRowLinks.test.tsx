// Goal: each row of the deck list links to the builder at the address of its own deck, by its name and by an Edit button whose name says which deck, and following the link changes the address to the builder's; a deck that holds a card the game does not run whole links to its list of those cards, and the page header links to the list of every deck.
import { screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { counts, deckIn, deckItem, summary } from './fakeDecks'
import { deckRows, renderDecks } from './renderDecks'

const DECKS = [
  deckItem({ id: 'deck-1', name: 'Atraxa Superfriends' }),
  deckItem({ id: 'deck-2', name: 'Burn', format: 'modern' }),
]

describe('the rows', () => {
  it('link to the builder of their own deck, by the name and by Edit', async () => {
    await renderDecks({ decks: DECKS })
    await screen.findByRole('list', { name: 'Your decks' })
    const [first, second] = deckRows()
    const one = within(first as HTMLElement)
    const two = within(second as HTMLElement)
    expect(one.getByRole('link', { name: 'Atraxa Superfriends' })).toHaveAttribute('href', '/decks/deck-1')
    expect(one.getByRole('link', { name: 'Edit Atraxa Superfriends' })).toHaveAttribute(
      'href',
      '/decks/deck-1',
    )
    expect(two.getByRole('link', { name: 'Burn' })).toHaveAttribute('href', '/decks/deck-2')
    expect(two.getByRole('link', { name: 'Edit Burn' })).toHaveAttribute('href', '/decks/deck-2')
  })

  it('open the builder when Edit or the name is followed', async () => {
    const { user, router } = await renderDecks({ decks: DECKS })
    await screen.findByRole('list', { name: 'Your decks' })
    await user.click(screen.getByRole('link', { name: 'Edit Burn' }))
    await waitFor(() => expect(router.state.location.pathname).toBe('/decks/deck-2'))
  })

  it('keep the name as the title of the row, so a list is read by names', async () => {
    await renderDecks({ decks: DECKS })
    await screen.findByRole('list', { name: 'Your decks' })
    expect(deckRows()[0]).toHaveTextContent('Atraxa Superfriends')
    expect(screen.getAllByRole('link', { name: /^Edit / })).toHaveLength(2)
  })

  it.each([
    ['a manual card', { manual: 2, verified: 98 }, 'Show the 2 cards played by hand'],
    ['a partial card', { partial: 1, verified: 99 }, 'Show the 1 card played by hand'],
    ['a stale card', { stale: 1, verified: 99 }, 'Show the 1 card played by hand'],
    ['an unplayable card', { unplayable: 1, verified: 99 }, 'Show the 1 unplayable card'],
    [
      'both',
      { manual: 7, unplayable: 1, verified: 92 },
      'Show the 7 cards played by hand and 1 unplayable card',
    ],
  ])(
    'link a deck with %s to the list of its cards that are not validated, in the words of the count',
    async (_, held, words) => {
      const flagged = deckIn('manual_play', {
        id: 'deck-9',
        name: 'Chaos',
        summary: summary({ counts: counts(held) }),
      })
      const { user, router } = await renderDecks({
        decks: [flagged, deckItem({ id: 'deck-1', name: 'Whole' })],
      })
      await screen.findByRole('list', { name: 'Your decks' })
      expect(screen.getByRole('link', { name: `${words} in Chaos` })).toHaveAttribute(
        'href',
        '/decks/deck-9/unvalidated',
      )
      expect(screen.queryByRole('link', { name: /^Show the .* in Whole$/ })).toBeNull()
      await user.click(screen.getByRole('link', { name: `${words} in Chaos` }))
      await waitFor(() => expect(router.state.location.pathname).toBe('/decks/deck-9/unvalidated'))
    },
  )

  it('link the page header to the list of every deck', async () => {
    const { user, router } = await renderDecks({ decks: DECKS })
    await screen.findByRole('list', { name: 'Your decks' })
    const link = screen.getByRole('link', { name: 'Not validated cards' })
    expect(link).toHaveAttribute('href', '/decks/unvalidated')
    await user.click(link)
    await waitFor(() => expect(router.state.location.pathname).toBe('/decks/unvalidated'))
  })

  it('give Edit the size of the other row buttons: 32 px, and 44 px under a coarse pointer', async () => {
    await renderDecks({ decks: DECKS })
    await screen.findByRole('list', { name: 'Your decks' })
    const edit = screen.getByRole('link', { name: 'Edit Burn' })
    expect(edit).toHaveClass('h-8', 'pointer-coarse:h-11')
  })
})
