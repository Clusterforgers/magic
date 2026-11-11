// Goal: the deck pane says how many cards are played by hand; its Stats tab shows the figures (lands, spells, average mana value, colour identity), the support counts, colours, mana curve and types; its Played by hand tab lists those cards with their reasons and states worst first; the save bar says what blocks saving; under lg the Show switch keeps to Search and Deck.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { cardHit } from '@/features/decks/fakeDecks'
import { checked, ENTRIES } from './checkedDeck'
import { deckPane, MOUNTAIN, renderBuilder, searchBox } from './renderBuilder'

const pane = () => within(deckPane())

describe('the summary of the deck', () => {
  it('says how many cards are played by hand and how many cannot be played at all', async () => {
    await renderBuilder({ view: checked() })
    expect(deckPane().querySelector('[data-slot="hand-summary"]')).toHaveTextContent(
      '7 cards played by hand, 1 cannot be played',
    )
  })

  it('says the whole deck runs when every card is verified', async () => {
    await renderBuilder({ view: checked([{ card: MOUNTAIN, count: 24, support: 'verified' }]) })
    expect(deckPane().querySelector('[data-slot="hand-summary"]')).toHaveTextContent('The whole deck runs.')
    expect(deckPane().querySelector('button[data-slot="hand-summary"]')).toBeNull()
  })

  it('opens the list of cards played by hand from the summary', async () => {
    const { user } = await renderBuilder({ view: checked() })
    expect(pane().getByRole('tab', { name: /^Played by hand/ })).toHaveAttribute('aria-selected', 'false')
    await user.click(pane().getByRole('button', { name: /cards played by hand/ }))
    expect(pane().getByRole('tab', { name: /^Played by hand/ })).toHaveAttribute('aria-selected', 'true')
    expect(pane().getByRole('list', { name: 'Cards played by hand' })).toBeVisible()
  })
})

describe('the stats', () => {
  it('open on their tab with the lands, the spells, the average mana value and the colour identity as figures', async () => {
    const { user } = await renderBuilder({ view: checked() })
    expect(deckPane().querySelector('[data-slot="deck-stats-panel"]')).toBeNull()
    await user.click(pane().getByRole('tab', { name: /^Stats/ }))
    const figures = deckPane().querySelector<HTMLElement>('[data-slot="deck-figures"]') as HTMLElement
    const value = (label: string) => within(figures).getByText(label).nextElementSibling
    expect(value('Lands')).toHaveTextContent(/^20$/)
    expect(value('Spells')).toHaveTextContent(/^8$/)
    expect(value('Average mana value')).toHaveTextContent(/^\d+\.\d\d$/)
    expect(within(figures).getByRole('img', { name: /^Color identity: / })).toBeInTheDocument()
  })

  it('count the copies by support state, by colour and by type, and draw the mana curve of the spells', async () => {
    const { user } = await renderBuilder({ view: checked() })
    await user.click(pane().getByRole('tab', { name: /^Stats/ }))
    const panel = within(
      deckPane().querySelector<HTMLElement>('[data-slot="deck-stats-panel"]') as HTMLElement,
    )
    const support = panel.getByRole('list', { name: 'Cards by support' })
    for (const [state, count] of [
      ['verified', '22'],
      ['partial', '4'],
      ['manual', '2'],
      ['stale', '1'],
      ['unplayable', '1'],
    ] as const) {
      expect(support.querySelector(`[data-support-count="${state}"]`)).toHaveTextContent(count)
    }
    expect(panel.getByLabelText('Red: 4 cards')).toBeInTheDocument()
    expect(panel.getByLabelText('Green: 3 cards')).toBeInTheDocument()
    expect(panel.getByLabelText('Colorless: 1 card')).toBeInTheDocument()
    expect(panel.getByLabelText('Mana value 1: 8 cards')).toBeInTheDocument()
    expect(
      panel.getByRole('list', { name: 'Cards by type' }).querySelector('[data-kind="land"]'),
    ).toHaveTextContent('20')
    expect(panel.getByRole('img', { name: 'Color identity: Black, Red and Green' })).toBeInTheDocument()
  })
})

describe('the cards played by hand', () => {
  it('are listed with their reasons, the worst state first, and the cards the game cannot play apart', async () => {
    const { user } = await renderBuilder({ view: checked() })
    await user.click(pane().getByRole('tab', { name: /^Played by hand/ }))
    const hand = pane().getByRole('list', { name: 'Cards played by hand' })
    expect(
      within(hand)
        .getAllByRole('listitem', { hidden: false })
        .filter((li) => li.dataset.hand),
    ).toHaveLength(3)
    const states = [...hand.querySelectorAll<HTMLElement>('li[data-hand]')].map((li) => li.dataset.support)
    expect(states).toEqual(['manual', 'stale', 'partial'])
    const bolt = hand.querySelector<HTMLElement>('li[data-hand="oracle-bolt"]') as HTMLElement
    expect(within(bolt).getByRole('button', { name: /^Lightning Bolt/ })).toBeInTheDocument()
    expect(within(bolt).getByText('×4')).toBeInTheDocument()
    expect(within(bolt).getByText('Partial')).toBeInTheDocument()
    expect(within(bolt).getByText('The engine does not run X yet')).toBeInTheDocument()
    const blocked = pane().getByRole('list', { name: 'Cards the game cannot play' })
    expect(within(blocked).getByRole('button', { name: /^Llanowar Elves/ })).toBeInTheDocument()
    expect(within(blocked).getByText('Unplayable')).toBeInTheDocument()
  })

  it('open the card preview from the name', async () => {
    const { user } = await renderBuilder({ view: checked() })
    await user.click(pane().getByRole('tab', { name: /^Played by hand/ }))
    const hand = pane().getByRole('list', { name: 'Cards played by hand' })
    await user.click(within(hand).getByRole('button', { name: /^Giant Growth/ }))
    expect(await screen.findByRole('dialog')).toBeInTheDocument()
  })

  it('say that a deck the game runs whole has none', async () => {
    const { user } = await renderBuilder({
      view: checked([{ card: MOUNTAIN, count: 24, support: 'verified' }]),
    })
    await user.click(pane().getByRole('tab', { name: /^Played by hand/ }))
    expect(pane().getByText('Every card runs whole.')).toBeInTheDocument()
  })

  it('say that the reasons come with the check for a card the check has not seen', async () => {
    const stranger = cardHit('Stranger', { oracle_id: 'oracle-stranger', support: 'manual' })
    const { user } = await renderBuilder({
      view: checked([{ card: MOUNTAIN, count: 24, support: 'verified' }]),
      cards: { cards: [stranger] },
      check: { cards: [] },
    })
    await user.type(searchBox(), 'Stranger')
    await user.click(await screen.findByRole('button', { name: 'Add Stranger to main deck' }))
    await user.click(pane().getByRole('tab', { name: /^Played by hand/ }))
    expect(await pane().findByText('Stranger')).toBeInTheDocument()
  })
})

describe('the save bar', () => {
  it('says there is nothing to save, then that changes are unsaved', async () => {
    const { user } = await renderBuilder({ view: checked() })
    const bar = () => deckPane().querySelector('[data-slot="save-bar"]') as HTMLElement
    expect(bar()).toHaveAttribute('data-status', 'saved')
    expect(bar()).toHaveTextContent('All changes saved.')
    await user.click(screen.getByRole('button', { name: 'Add one Lightning Bolt' }))
    expect(bar()).toHaveAttribute('data-status', 'ready')
    expect(bar()).toHaveTextContent('Unsaved changes.')
  })

  it('names what blocks saving, and ties it to the Save button', async () => {
    const { user } = await renderBuilder({ view: checked() })
    await user.clear(pane().getByRole('textbox', { name: 'Deck name' }))
    expect(pane().getByRole('button', { name: 'Save' })).toHaveAccessibleDescription(
      'Cannot save yet. Enter a name for the deck.',
    )
  })

  it('says the legality issues of the draft and that they do not stop a save', async () => {
    const { user } = await renderBuilder({
      view: checked(ENTRIES, ['Too few cards.', 'Too many copies.']),
    })
    await user.click(screen.getByRole('button', { name: 'Add one Lightning Bolt' }))
    expect(deckPane().querySelector('[data-slot="save-bar"]')).toHaveTextContent(
      /^Unsaved changes. Not legal in Modern: 2 cards not in the format; it can still be saved and played./,
    )
  })
})

describe('the Show switch under lg', () => {
  it('switches between the search and the deck, whose tabs hold the stats', async () => {
    const { user } = await renderBuilder({ view: checked() })
    const show = screen.getByRole('group', { name: 'Show' })
    expect(
      within(show)
        .getAllByRole('button')
        .map((button) => button.textContent),
    ).toEqual(['Search', 'Deck 30'])
    await user.click(within(show).getByRole('button', { name: 'Search' }))
    await user.click(pane().getByRole('button', { name: /cards played by hand/ }))
    expect(within(show).getByRole('button', { name: /^Deck/ })).toHaveAttribute('aria-pressed', 'true')
    expect(deckPane()).toHaveAttribute('data-view', 'hand')
  })
})
