// Goal: the stored decks of /dev/lobby hold a deck that runs whole on approved reads and one with drafts waiting, whose list entry, cards and table check agree on the state and the cards to read.
import { describe, expect, it } from 'vitest'
import { lobbyDecks } from './deckApis'

describe('the stored decks on reads', () => {
  it.each([
    ['stored-omnath', 0],
    ['stored-kenrith', 2],
  ] as const)('%s is on reads with %i cards to read in the list, its cards and its check', async (id, toRead) => {
    const api = lobbyDecks()
    const listed = (await api.list()).find((deck) => deck.id === id)
    expect([listed?.state, listed?.summary.to_read]).toEqual(['read', toRead])
    const view = await api.get(id)
    expect([view.state, view.report?.summary.to_read]).toEqual(['read', toRead])
    const check = await api.check({ deck: id, format: 'commander', cards: null })
    expect([check.summary.state, check.summary.to_read]).toEqual(['read', toRead])
  })
})
