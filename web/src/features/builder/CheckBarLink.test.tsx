// Goal: the builder's check bar links a saved deck with cards played by hand or drafts waiting to its not-validated list ("Cards to check", `/decks/$deckId/unvalidated`), and draws no link for a deck the game runs whole or a deck that is not saved.
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { counts } from '@/features/decks/fakeDecks'
import { renderInRouter } from '@/test-utils/router'
import { CheckBar } from './CheckBar'
import { checked, ENTRIES } from './checkedDeck'
import type { Entry } from './checkedDeck'
import { BOLT, deckPane, GROWTH, MOUNTAIN, renderBuilder } from './renderBuilder'

const link = (): Element | null => deckPane().querySelector('[data-slot="check-unvalidated"]')

const CASES: [string, Entry[], boolean][] = [
  ['a card of each state the game does not run whole', ENTRIES, true],
  [
    'a draft waiting for a read',
    [
      { card: MOUNTAIN, count: 20, support: 'verified' },
      { card: GROWTH, count: 2, support: 'unverified', draft: 'pending' },
    ],
    true,
  ],
  [
    'only verified cards and approved reads',
    [
      { card: MOUNTAIN, count: 20, support: 'verified' },
      { card: BOLT, count: 4, support: 'read', draft: 'approved' },
    ],
    false,
  ],
]

describe('the link of a saved deck', () => {
  it.each(CASES)('for a deck with %s: %s', async (_, entries, shown) => {
    await renderBuilder({ view: checked(entries) })
    if (!shown) {
      expect(link()).toBeNull()
      return
    }
    expect(link()).toHaveTextContent(/^Cards to check$/)
    expect(link()).toHaveAttribute('href', '/decks/deck-1/unvalidated')
  })
})

describe('a deck that is not saved', () => {
  it('draws no link, however many cards are played by hand', async () => {
    await renderInRouter(
      <CheckBar
        counts={counts({ total: 10, verified: 7, manual: 3 })}
        toRead={2}
        asSaved={false}
        status="current"
        error={null}
        source="draft"
        onRetry={() => undefined}
        issuesOnly={false}
        onIssuesOnly={() => undefined}
        sort="name"
        onSort={() => undefined}
      />,
    )
    expect(await screen.findByRole('combobox', { name: 'Sort cards' })).toBeInTheDocument()
    expect(document.querySelector('[data-slot="check-counts"]')).toHaveTextContent(/3 played by hand/)
    expect(screen.queryByRole('link', { name: 'Cards to check' })).toBeNull()
  })
})
