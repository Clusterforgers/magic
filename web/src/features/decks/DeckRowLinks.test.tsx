// Goal: each row of the deck list links to the builder at the address of its own deck, by its name and by an Edit button whose name says which deck, and following the link changes the address to the builder's.
import { screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { deckItem } from './fakeDecks'
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

  it('give Edit the size of the other row buttons: 32 px, and 44 px under a coarse pointer', async () => {
    await renderDecks({ decks: DECKS })
    await screen.findByRole('list', { name: 'Your decks' })
    const edit = screen.getByRole('link', { name: 'Edit Burn' })
    expect(edit).toHaveClass('h-8', 'pointer-coarse:h-11')
  })
})
