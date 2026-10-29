// Goal: a card's preview offers "Mark important to fix" for any card once it is given the fix-mark endpoints (a card the game does not run whole and a fully supported one alike), shows the account's mark as pressed, and offers nothing without them.
import { render, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fixMarksClient } from '@/features/fixmarks/client'
import type { CardsApi } from '@/net/cards'
import type { FixMarksApi } from '@/net/fixMarks'
import type { CardHit, FixMarkView } from '@/protocol/generated'
import { CardPreview } from './CardPreview'
import { cardDetail, cardHit } from './fakeCards'

const cardsFor = (hit: CardHit): CardsApi => ({
  search: () => Promise.reject(new Error('no search here')),
  get: () => Promise.resolve(cardDetail(hit.name, { card: hit })),
})

function marks(initial: FixMarkView[] = []) {
  return {
    list: vi.fn(() => Promise.resolve(initial)),
    mark: vi.fn(),
    unmark: vi.fn(),
  } satisfies FixMarksApi
}

const SOL: CardHit = cardHit('Sol Ring', { oracle_id: 'oracle-sol', support: 'partial' })
const BOLT: CardHit = cardHit('Lightning Bolt', { oracle_id: 'oracle-bolt', support: 'verified' })

function open(card: CardHit, fixMarks?: FixMarksApi) {
  return render(
    <CardPreview card={card} open api={cardsFor(card)} fixMarks={fixMarks} onClose={() => undefined} />,
  )
}

beforeEach(() => {
  fixMarksClient.clear()
})

describe('CardPreview with fix marks', () => {
  it.each([
    ['a card the game runs in part', SOL],
    ['a fully supported card', BOLT],
  ])('offers the button for %s', async (_name, card) => {
    const api = marks()
    open(card, api)
    const button = await screen.findByRole('button', { name: 'Mark important to fix' })
    expect(button).toHaveAttribute('aria-pressed', 'false')
    await waitFor(() => expect(api.list).toHaveBeenCalledTimes(1))
  })

  it('shows the account’s mark on the card as pressed', async () => {
    const mark: FixMarkView = { oracle_id: 'oracle-sol', name: 'Sol Ring', note: null, created_at: 'x' }
    open(SOL, marks([mark]))
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Mark important to fix' })).toHaveAttribute(
        'aria-pressed',
        'true',
      ),
    )
  })

  it('offers nothing without the endpoints', async () => {
    open(SOL)
    await screen.findByRole('dialog', { name: 'Sol Ring' })
    expect(screen.queryByRole('button', { name: 'Mark important to fix' })).toBeNull()
  })
})
