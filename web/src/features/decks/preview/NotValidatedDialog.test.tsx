// Goal: the link to a deck's not-validated cards names how many copies are played by hand (and how many are unplayable), in the words of the decks list, counts commanders as well as the main deck and leaves the sideboard out, and opens the list of those cards.
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { unvalidatedLink } from '@/features/decks/format'
import { NotValidatedDialog, notValidatedWords } from './NotValidatedDialog'
import type { DeckPreviewData, PreviewCard } from './model'
import { supportCounts } from './model'

const card = (name: string, support: PreviewCard['support'], count = 1): PreviewCard => ({
  oracleId: `o-${name}`,
  name,
  count,
  typeLine: 'Sorcery',
  manaValue: 2,
  manaCost: '{1}{R}',
  identity: ['R'],
  support,
  reasons: support === 'verified' ? [] : [`${name} needs a hand`],
})

const deck = (over: Partial<DeckPreviewData> = {}): DeckPreviewData => ({
  name: 'Burn',
  source: 'Your deck',
  format: 'commander',
  commanders: [],
  cards: [],
  sideboard: [],
  ...over,
})

describe('the words of the link to the not-validated cards', () => {
  it.each<[string, DeckPreviewData, string]>([
    ['a deck that runs whole', deck({ cards: [card('Bolt', 'verified', 4)] }), ''],
    ['one partial card', deck({ cards: [card('Odd', 'partial')] }), 'Show the 1 card played by hand'],
    [
      'partial, manual and stale copies',
      deck({ cards: [card('A', 'partial', 2), card('B', 'manual', 3), card('C', 'stale')] }),
      'Show the 6 cards played by hand',
    ],
    [
      'a commander played by hand',
      deck({ commanders: [card('Boss', 'manual')], cards: [card('Odd', 'partial', 2)] }),
      'Show the 3 cards played by hand',
    ],
    [
      'cards played by hand and an unplayable one',
      deck({ cards: [card('Odd', 'manual', 7), card('Void', 'unplayable')] }),
      'Show the 7 cards played by hand and 1 unplayable card',
    ],
    [
      'a sideboard card played by hand, which the game does not count',
      deck({ cards: [card('Bolt', 'verified')], sideboard: [card('Odd', 'manual', 3)] }),
      '',
    ],
  ])('says %s', (_what, data, words) => {
    expect(notValidatedWords(data)).toBe(words)
  })

  it('is the words the decks list says for the same counts', () => {
    const data = deck({ commanders: [card('Boss', 'stale')], cards: [card('Odd', 'manual', 4)] })
    expect(notValidatedWords(data)).toBe(unvalidatedLink(supportCounts([...data.commanders, ...data.cards])))
  })
})

describe('the button of the not-validated cards', () => {
  it('carries the words and opens the cards played by hand with their reasons', async () => {
    render(
      <NotValidatedDialog deck={deck({ cards: [card('Odd', 'manual', 2), card('Bolt', 'verified')] })} />,
    )
    const button = screen.getByRole('button', { name: 'Show the 2 cards played by hand' })
    await userEvent.click(button)
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByText('Odd')).toBeInTheDocument()
    expect(within(dialog).getByText('Odd needs a hand')).toBeInTheDocument()
    expect(within(dialog).queryByText('Bolt')).toBeNull()
  })

  it('says on each card whether its draft waits for a read or was found wrong, and how many cards wait', async () => {
    render(
      <NotValidatedDialog
        deck={deck({
          cards: [
            { ...card('Waiting', 'manual'), draft: 'pending' },
            { ...card('Wrong', 'manual'), draft: 'wrong' },
            card('Plain', 'manual'),
            { ...card('Read', 'read'), draft: 'approved' },
          ],
          toRead: 1,
          readTarget: { precon: 'c18-x' },
        })}
      />,
    )
    await userEvent.click(screen.getByRole('button', { name: 'Show the 3 cards played by hand' }))
    const dialog = await screen.findByRole('dialog')
    const noteOf = (name: string): Element | null =>
      within(dialog).getByText(name).closest('li')?.querySelector('[data-slot="draft-note"]') ?? null
    expect(noteOf('Waiting')).toHaveAttribute('data-draft', 'pending')
    expect(noteOf('Wrong')).toHaveAttribute('data-draft', 'wrong')
    expect(noteOf('Plain')).toBeNull()
    expect(within(dialog).queryByText('Read')).toBeNull()
    expect(dialog.querySelector('[data-slot="to-read"]')).toHaveAttribute(
      'data-read-path',
      '/read?precon=c18-x',
    )
    expect(dialog.querySelector('[data-slot="deck-to-read"]')).toHaveTextContent('1 card to read')
  })
})
