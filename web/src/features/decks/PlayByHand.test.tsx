// Goal: marking a card "play by hand" in a deck saves the deck as stored with every line of that card marked and the other lines as they were, the button says which deck, turns into a note once saved or when the card is already by hand there, and shows the server's refusal without losing the button.
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { DeckError } from '@/net/decks'
import type { DeckBody, DeckView } from '@/protocol/generated'
import { byHandBody } from './byHand'
import type { ByHandApi } from './byHand'
import { cardHit, deckView, lineOf } from './fakeDecks'
import { PlayByHand } from './PlayByHand'

const BOLT = cardHit('Lightning Bolt', { oracle_id: 'oracle-bolt' })
const MOUNTAIN = cardHit('Mountain', { oracle_id: 'oracle-mountain' })
const ELVES = cardHit('Llanowar Elves', { oracle_id: 'oracle-elves' })

const stored = (): DeckView =>
  deckView({
    id: 'deck-1',
    name: 'Burn',
    cards: [
      lineOf('Mountain', 20, { card: MOUNTAIN, printing_id: 'printing-1' }),
      lineOf('Lightning Bolt', 3, { card: BOLT }),
      lineOf('Lightning Bolt', 1, { card: BOLT, zone: 'sideboard' }),
      lineOf('Llanowar Elves', 4, { card: ELVES, by_hand: true }),
    ],
  })

describe('byHandBody', () => {
  it.each([
    ['a card by the rules', 'oracle-bolt', [undefined, true, true, true]],
    ['a card already by hand', 'oracle-elves', [undefined, undefined, undefined, true]],
    ['a card the deck lacks', 'oracle-none', [undefined, undefined, undefined, true]],
  ])('marks every line of %s and keeps the rest', (_, oracleId, expected) => {
    const body = byHandBody(stored(), oracleId)
    expect(body.cards.map((line) => line.by_hand)).toEqual(expected)
    expect(body.cards.map((line) => [line.zone, line.oracle_id, line.printing_id, line.count])).toEqual([
      ['main', 'oracle-mountain', 'printing-1', 20],
      ['main', 'oracle-bolt', null, 3],
      ['sideboard', 'oracle-bolt', null, 1],
      ['main', 'oracle-elves', null, 4],
    ])
    expect([body.name, body.format, body.notes]).toEqual(['Burn', stored().format, stored().notes])
  })
})

function fakeApi(update: (id: string, body: DeckBody) => Promise<DeckView>) {
  return {
    get: vi.fn(async () => stored()),
    update: vi.fn(update),
  } satisfies ByHandApi
}

describe('PlayByHand', () => {
  it('saves the deck with the card marked, then says it is played by hand there', async () => {
    const api = fakeApi(async () => stored())
    const onMarked = vi.fn()
    const user = userEvent.setup()
    render(
      <PlayByHand
        oracleId="oracle-bolt"
        name="Lightning Bolt"
        api={api}
        onMarked={onMarked}
        decks={[
          { id: 'deck-1', name: 'Burn', byHand: false },
          { id: 'deck-2', name: 'Elves', byHand: true },
        ]}
      />,
    )
    expect(screen.getByText('Played by hand in Elves')).toBeInTheDocument()
    expect(screen.getAllByRole('button')).toHaveLength(1)
    await user.click(screen.getByRole('button', { name: 'Play Lightning Bolt by hand in Burn' }))
    expect(await screen.findByText('Played by hand in Burn')).toBeInTheDocument()
    expect(api.get).toHaveBeenCalledWith('deck-1')
    expect(api.update).toHaveBeenCalledWith('deck-1', byHandBody(stored(), 'oracle-bolt'))
    expect(onMarked).toHaveBeenCalledWith('deck-1')
    expect(screen.queryByRole('button')).toBeNull()
  })

  it('shows a refusal and keeps the button to try again', async () => {
    const api = fakeApi(async () => {
      throw new DeckError('invalid_deck', 400, 'the deck is not valid')
    })
    const user = userEvent.setup()
    render(
      <PlayByHand
        oracleId="oracle-bolt"
        name="Lightning Bolt"
        api={api}
        decks={[{ id: 'deck-1', name: 'Burn', byHand: false }]}
      />,
    )
    await user.click(screen.getByRole('button', { name: 'Play Lightning Bolt by hand in Burn' }))
    expect(await screen.findByRole('alert')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Play Lightning Bolt by hand in Burn' })).toBeEnabled()
  })

  it('draws nothing for a card in none of the owner’s decks', () => {
    const { container } = render(
      <PlayByHand oracleId="oracle-bolt" name="Lightning Bolt" api={fakeApi(async () => stored())} decks={[]} />,
    )
    expect(container).toBeEmptyDOMElement()
  })
})
