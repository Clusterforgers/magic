// Goal: the deck pane shows its cards, its stats and its cards played by hand as three tabs with the whole width of the pane, at every window size; the cards played by hand quote the words each reason covers.
import { within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { checked } from './checkedDeck'
import { cardDetail } from './fakeCards'
import { BOLT, deckPane, renderBuilder } from './renderBuilder'

const pane = () => within(deckPane())

const base = cardDetail()
const boltText = cardDetail('Lightning Bolt', {
  card: { ...BOLT, oracle_id: 'oracle-bolt' },
  faces: base.faces.map((face) => ({ ...face, oracle_text: 'Lightning Bolt deals 3 damage to any target.' })),
})

describe('the words of the cards played by hand', () => {
  it('quote the span each reason covers beside the reason', async () => {
    const { user } = await renderBuilder({ view: checked(), cards: { details: [boltText] } })
    await user.click(pane().getByRole('tab', { name: /^Played by hand/ }))
    const hand = pane().getByRole('list', { name: 'Cards played by hand' })
    const bolt = hand.querySelector<HTMLElement>('li[data-hand="oracle-bolt"]') as HTMLElement
    const quote = await within(bolt).findByText('“Lightning”')
    expect(quote.tagName).toBe('BLOCKQUOTE')
    expect(within(bolt).getByText('The engine does not run X yet')).toBeInTheDocument()
  })

  it('show the reason alone for a card whose text cannot be read', async () => {
    const { user } = await renderBuilder({ view: checked(), cards: { details: [] } })
    await user.click(pane().getByRole('tab', { name: /^Played by hand/ }))
    const hand = pane().getByRole('list', { name: 'Cards played by hand' })
    const bolt = hand.querySelector<HTMLElement>('li[data-hand="oracle-bolt"]') as HTMLElement
    expect(await within(bolt).findByText('The engine does not run X yet')).toBeInTheDocument()
    expect(bolt.querySelector('blockquote')).toBeNull()
  })
})

describe('the views', () => {
  const SHOWN = {
    cards: '[data-line]',
    stats: '[data-slot="deck-stats-panel"]',
    hand: '[data-slot="hand-list"]',
  }

  it.each([
    ['Cards', 'cards'],
    ['Stats', 'stats'],
    ['Played by hand', 'hand'],
  ] as const)('the %s tab shows its view alone', async (tab, view) => {
    const { user } = await renderBuilder({ view: checked() })
    const trigger = pane().getByRole('tab', { name: new RegExp(`^${tab}`) })
    await user.click(trigger)
    expect(trigger).toHaveAttribute('aria-selected', 'true')
    expect(deckPane()).toHaveAttribute('data-view', view)
    for (const [other, selector] of Object.entries(SHOWN)) {
      expect(deckPane().querySelector(selector) !== null).toBe(other === view)
    }
  })

  it('keeps the check bar to the Cards view', async () => {
    const { user } = await renderBuilder({ view: checked() })
    expect(pane().getByRole('switch', { name: 'Issues only' })).toBeInTheDocument()
    await user.click(pane().getByRole('tab', { name: /^Stats/ }))
    expect(pane().queryByRole('switch', { name: 'Issues only' })).toBeNull()
  })

  it('counts the cards and the copies played by hand on the tabs', async () => {
    await renderBuilder({ view: checked() })
    expect(pane().getByRole('tab', { name: /^Cards/ })).toHaveTextContent('Cards30')
    expect(pane().getByRole('tab', { name: /^Played by hand/ })).toHaveTextContent('Played by hand7')
  })
})
