// Goal: the cards played by hand quote the words each reason covers, and from 1440 px the stats and that list sit in a rail beside the deck instead of two closed disclosures.
import { within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { checked } from './checkedDeck'
import { cardDetail } from './fakeCards'
import { BOLT, deckPane, renderBuilder } from './renderBuilder'

const pane = () => within(deckPane())

/** A window as wide as `wide` says: `(min-width: 1440px)` matches only then. */
function stubWidth(wide: boolean): void {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: wide && query === '(min-width: 1440px)',
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }))
}

beforeEach(() => {
  Element.prototype.scrollIntoView = vi.fn()
})
afterEach(() => {
  vi.unstubAllGlobals()
})

const base = cardDetail()
const boltText = cardDetail('Lightning Bolt', {
  card: { ...BOLT, oracle_id: 'oracle-bolt' },
  faces: base.faces.map((face) => ({ ...face, oracle_text: 'Lightning Bolt deals 3 damage to any target.' })),
})

describe('the words of the cards played by hand', () => {
  it('quote the span each reason covers beside the reason', async () => {
    const { user } = await renderBuilder({ view: checked(), cards: { details: [boltText] } })
    await user.click(pane().getByRole('button', { name: /^Played by hand/ }))
    const hand = pane().getByRole('list', { name: 'Cards played by hand' })
    const bolt = hand.querySelector<HTMLElement>('li[data-hand="oracle-bolt"]') as HTMLElement
    const quote = await within(bolt).findByText('“Lightning”')
    expect(quote.tagName).toBe('BLOCKQUOTE')
    expect(within(bolt).getByText('The engine does not run X yet')).toBeInTheDocument()
  })

  it('show the reason alone for a card whose text cannot be read', async () => {
    const { user } = await renderBuilder({ view: checked(), cards: { details: [] } })
    await user.click(pane().getByRole('button', { name: /^Played by hand/ }))
    const hand = pane().getByRole('list', { name: 'Cards played by hand' })
    const bolt = hand.querySelector<HTMLElement>('li[data-hand="oracle-bolt"]') as HTMLElement
    expect(await within(bolt).findByText('The engine does not run X yet')).toBeInTheDocument()
    expect(bolt.querySelector('blockquote')).toBeNull()
  })
})

describe('the rail', () => {
  it('holds the stats and the cards played by hand open from 1440 px, with no disclosure', async () => {
    stubWidth(true)
    await renderBuilder({ view: checked() })
    const rail = deckPane().querySelector<HTMLElement>('[data-slot="deck-rail"]')
    expect(rail).not.toBeNull()
    const inside = within(rail as HTMLElement)
    expect(inside.getByRole('heading', { name: /^Stats/ })).toBeVisible()
    expect(inside.getByRole('heading', { name: /^Played by hand/ })).toBeVisible()
    expect(inside.getByRole('list', { name: 'Cards played by hand' })).toBeVisible()
    expect(deckPane().querySelector('[data-slot="deck-stats-panel"]')).toBeVisible()
    expect(pane().queryByRole('button', { name: /^Stats/ })).toBeNull()
    expect(pane().queryByRole('button', { name: /^Played by hand/ })).toBeNull()
  })

  it('leaves the two closed disclosures below 1440 px', async () => {
    stubWidth(false)
    await renderBuilder({ view: checked() })
    expect(deckPane().querySelector('[data-slot="deck-rail"]')).toBeNull()
    expect(pane().getByRole('button', { name: /^Stats/ })).toHaveAttribute('aria-expanded', 'false')
    expect(pane().getByRole('button', { name: /^Played by hand/ })).toHaveAttribute('aria-expanded', 'false')
  })
})
