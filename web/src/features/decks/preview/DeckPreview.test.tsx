// Goal: the deck preview shows a deck at a glance and nothing it was not given: commanders, name, legality, identity, size, the support counts with the not-validated slot only for a deck with hand-played cards, the bars, the cards grouped by type with count badges and a support mark, and a large view on a click.
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { DeckPreview } from './DeckPreview'
import type { DeckPreviewProps } from './DeckPreview'
import type { PreviewCard } from './model'

const card = (name: string, over: Partial<PreviewCard> = {}): PreviewCard => ({
  oracleId: `o-${name.toLowerCase().replace(/\W+/g, '-')}`,
  name,
  count: 1,
  typeLine: 'Instant',
  manaValue: 1,
  manaCost: '{R}',
  identity: ['R'],
  support: 'verified',
  reasons: [],
  ...over,
})

const DECK: DeckPreviewProps['deck'] = {
  name: 'Dragons',
  source: 'Precon C18, Commander Deck',
  format: 'commander',
  commanders: [card('The Ur-Dragon', { typeLine: 'Legendary Creature — Dragon', identity: ['W', 'U', 'B', 'R', 'G'] })],
  cards: [
    card('Sol Ring', { typeLine: 'Artifact' }),
    card('Bolt', { count: 4 }),
    card('Odd Dragon', {
      typeLine: 'Creature — Dragon',
      manaValue: 5,
      support: 'manual',
      reasons: ['Resolve the second ability by hand.'],
    }),
    card('Forest', { typeLine: 'Basic Land — Forest', manaValue: 0, count: 40 }),
  ],
  sideboard: [],
}

describe('the deck preview', () => {
  it('names the deck with its source, format, identity and size', () => {
    render(<DeckPreview deck={DECK} />)
    expect(screen.getByRole('heading', { name: 'Dragons' })).toBeInTheDocument()
    expect(screen.getByText('Precon C18, Commander Deck, Commander')).toBeInTheDocument()
    expect(screen.getByRole('img', { name: /Colour identity: .*White.*Green/ })).toBeInTheDocument()
    expect(screen.getByText('47 cards')).toBeInTheDocument()
    expect(within(screen.getByRole('list', { name: 'Commanders' })).getAllByRole('listitem')).toHaveLength(1)
  })

  it('counts the cards by support and says how many are played by hand', () => {
    render(<DeckPreview deck={DECK} notValidated={<a href="/x">Not validated cards</a>} />)
    const summary = screen.getByRole('region', { name: 'Support' })
    expect(within(summary).getByText('1 of 47 cards are played by hand.')).toBeInTheDocument()
    expect(summary.querySelector('[data-support-count="verified"]')).toHaveTextContent('46')
    expect(summary.querySelector('[data-support-count="manual"]')).toHaveTextContent('1')
    expect(summary.querySelector('[data-support-count="partial"]')).toHaveTextContent('0')
    expect(summary.querySelector('[data-support-count="stale"]')).toBeNull()
    expect(screen.getByRole('link', { name: 'Not validated cards' })).toBeInTheDocument()
  })

  it('says the whole deck runs and leaves the not-validated link out when nothing is played by hand', () => {
    render(
      <DeckPreview
        deck={{ ...DECK, cards: DECK.cards.filter((entry) => entry.support === 'verified') }}
        notValidated={<a href="/x">Not validated cards</a>}
      />,
    )
    expect(screen.getByText('The whole deck runs.')).toBeInTheDocument()
    expect(screen.queryByRole('link', { name: 'Not validated cards' })).toBeNull()
  })

  it('groups the cards by type with counts, and marks the ones played by hand', () => {
    render(<DeckPreview deck={DECK} />)
    const groups = screen.getAllByRole('region').filter((region) => region.hasAttribute('data-group'))
    expect(groups.map((group) => group.getAttribute('data-group'))).toEqual(['creature', 'instant', 'artifact', 'land'])
    const instants = screen.getByRole('region', { name: 'Instants' })
    expect(within(instants).getByRole('heading', { name: /Instants/ })).toHaveTextContent('4')
    expect(within(instants).getByRole('button', { name: '4 copies of Bolt' })).toBeInTheDocument()
    expect(instants.querySelector('[data-slot="count-badge"]')).toHaveTextContent('×4')
    const creatures = screen.getByRole('region', { name: 'Creatures' })
    const odd = within(creatures).getByRole('button', { name: 'Odd Dragon, played by hand' })
    expect(odd.querySelector('[data-slot="support-badge"]')).not.toBeNull()
    expect(within(instants).getByRole('button', { name: '4 copies of Bolt' }).querySelector('[data-slot="support-badge"]')).toBeNull()
  })

  it('draws the mana curve and the type bars', () => {
    render(<DeckPreview deck={DECK} />)
    const curve = screen.getByRole('list', { name: 'Spells by mana value' })
    expect(within(curve).getByRole('listitem', { name: 'Mana value 1: 5 cards' })).toBeInTheDocument()
    expect(within(curve).getByRole('listitem', { name: 'Mana value 5: 1 card' })).toBeInTheDocument()
    expect(within(screen.getByRole('list', { name: 'Cards by type' })).getAllByRole('listitem')).toHaveLength(4)
  })

  it.each([
    ['legal', 'Legal in Commander', undefined],
    ['illegal', 'Not legal in Commander', ['A Commander deck holds one copy of each card.']],
    ['checking', 'Checking the deck for Commander…', undefined],
  ] as const)('shows the verdict %s as a line with its reasons', (verdict, text, reasons) => {
    render(<DeckPreview deck={DECK} legality={{ verdict, format: 'Commander', reasons }} />)
    expect(screen.getByText(text)).toBeInTheDocument()
    if (reasons !== undefined) expect(screen.getByText(reasons[0] ?? '')).toBeInTheDocument()
  })

  it('shows no legality line when the verdict is not known, and the actions under the header', () => {
    render(<DeckPreview deck={DECK} legality={{ verdict: 'unknown', format: 'Commander' }} actions={<button>Use</button>} />)
    expect(document.querySelector('[data-slot="legality"]')).toBeNull()
    expect(screen.getByRole('button', { name: 'Use' })).toBeInTheDocument()
  })

  it('opens a card large with its support and reasons, and closes it again', async () => {
    const user = userEvent.setup()
    render(<DeckPreview deck={DECK} />)
    await user.click(screen.getByRole('button', { name: 'Odd Dragon, played by hand' }))
    const zoom = await screen.findByRole('dialog')
    expect(zoom).toHaveTextContent('Odd Dragon')
    expect(within(zoom).getByText('Resolve the second ability by hand.')).toBeInTheDocument()
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('draws a body of its own under the header while the cards load', () => {
    render(<DeckPreview deck={{ ...DECK, cards: [] }} body={<p>Loading</p>} />)
    expect(screen.getByText('Loading')).toBeInTheDocument()
    expect(document.querySelector('[data-slot="support-summary"]')).toBeNull()
  })
})
