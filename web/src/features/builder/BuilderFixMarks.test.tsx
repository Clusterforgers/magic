// Goal: the builder offers "Mark important to fix" as a small flag on the deck lines, the picture tiles, the cards played by hand and the search hits, only for a card that is not Verified and only when it is given the fix-mark endpoints; a press marks the card at once and takes the mark back on the next.
import { screen, waitFor, within } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fixMarksClient } from '@/features/fixmarks/client'
import type { FixMarksApi } from '@/net/fixMarks'
import type { FixMarkView } from '@/protocol/generated'
import { checked } from './checkedDeck'
import { BOLT, deckLine, deckPane, GROWTH, POOL, renderBuilder, searchBox, searchPane } from './renderBuilder'

const pane = () => within(deckPane())

function marks(initial: FixMarkView[] = []) {
  const kept = [...initial]
  return {
    list: vi.fn(async () => [...kept]),
    mark: vi.fn(async (oracleId: string, note?: string | null): Promise<FixMarkView> => ({
      oracle_id: oracleId,
      name: oracleId,
      note: note ?? null,
      created_at: '2026-10-29T12:00:00Z',
    })),
    unmark: vi.fn(async () => undefined),
  } satisfies FixMarksApi
}

const flag = (scope: HTMLElement, name: string) =>
  within(scope).queryByRole('button', { name: `Mark ${name} important to fix` })

beforeEach(() => {
  fixMarksClient.clear()
})

describe('the flag on a deck line', () => {
  it('is on each line whose card is not verified and on no other', async () => {
    await renderBuilder({ view: checked(), fixMarks: marks() })
    for (const name of ['Lightning Bolt', 'Giant Growth', 'Sol Ring', 'Llanowar Elves']) {
      expect(flag(deckLine(name), name), name).toBeInTheDocument()
    }
    expect(flag(deckLine('Mountain'), 'Mountain')).toBeNull()
    expect(flag(deckLine('Duress'), 'Duress')).toBeNull()
  })

  it('is not drawn without the fix-mark endpoints', async () => {
    await renderBuilder({ view: checked() })
    expect(deckPane().querySelector('[data-slot="fix-mark"]')).toBeNull()
  })

  it('marks the card at once with a press and takes the mark back with the next', async () => {
    const api = marks()
    const { user } = await renderBuilder({ view: checked(), fixMarks: api })
    const button = flag(deckLine('Giant Growth'), 'Giant Growth') as HTMLElement
    await waitFor(() => expect(button).toHaveAttribute('aria-disabled', 'false'))
    expect(button).toHaveAttribute('aria-pressed', 'false')
    await user.click(button)
    expect(button).toHaveAttribute('aria-pressed', 'true')
    await waitFor(() => expect(api.mark).toHaveBeenCalledWith('oracle-growth', null))
    await user.click(button)
    expect(button).toHaveAttribute('aria-pressed', 'false')
    await waitFor(() => expect(api.unmark).toHaveBeenCalledWith('oracle-growth'))
  })

  it('shows a card the account has marked as pressed', async () => {
    const mark: FixMarkView = {
      oracle_id: 'oracle-growth',
      name: 'Giant Growth',
      note: null,
      created_at: 'x',
    }
    await renderBuilder({ view: checked(), fixMarks: marks([mark]) })
    const button = flag(deckLine('Giant Growth'), 'Giant Growth') as HTMLElement
    await waitFor(() => expect(button).toHaveAttribute('aria-pressed', 'true'))
  })
})

describe('the flag on a picture', () => {
  it('is over the picture of a card that is not verified, in the tile', async () => {
    await renderBuilder({ view: checked(), fixMarks: marks(), look: 'pictures' })
    expect(flag(deckLine('Lightning Bolt', 'main'), 'Lightning Bolt')).toBeInTheDocument()
    expect(flag(deckLine('Mountain', 'main'), 'Mountain')).toBeNull()
  })
})

describe('the flag on a card played by hand', () => {
  it('is beside the name in the list, for every card of it', async () => {
    const { user } = await renderBuilder({ view: checked(), fixMarks: marks() })
    await user.click(pane().getByRole('button', { name: /^Played by hand/ }))
    const hand = pane().getByRole('list', { name: 'Cards played by hand' })
    expect(within(hand).getAllByRole('button', { name: /important to fix$/ })).toHaveLength(3)
  })
})

describe('the flag on a search hit', () => {
  const pool = POOL.map((card) =>
    card.oracle_id === BOLT.oracle_id ? { ...card, support: 'manual' as const } : card,
  )

  it('is on a hit that is not verified and on no other', async () => {
    const { user } = await renderBuilder({ cards: { cards: pool }, fixMarks: marks() })
    await user.type(searchBox(), 'o')
    await screen.findByRole('button', { name: 'Add Giant Growth to main deck' })
    expect(flag(searchPane(), 'Lightning Bolt')).toBeInTheDocument()
    expect(flag(searchPane(), GROWTH.name)).toBeNull()
    const row = searchPane().querySelector<HTMLElement>(`li[data-card="${BOLT.oracle_id}"]`) as HTMLElement
    expect(within(row).getByRole('img', { name: 'Manual' })).toBeInTheDocument()
  })

  it('marks the card without adding it to the deck', async () => {
    const api = marks()
    const { user } = await renderBuilder({ cards: { cards: pool }, fixMarks: api })
    await user.type(searchBox(), 'Lightning')
    const button = (await waitFor(() => {
      const found = flag(searchPane(), 'Lightning Bolt')
      if (found === null) throw new Error('not yet')
      return found
    })) as HTMLElement
    await waitFor(() => expect(button).toHaveAttribute('aria-disabled', 'false'))
    await user.click(button)
    await waitFor(() => expect(api.mark).toHaveBeenCalledWith('oracle-bolt', null))
    expect(deckPane().querySelectorAll('li[data-line^="main:oracle-bolt"]')).toHaveLength(1)
    expect(within(deckLine('Lightning Bolt')).getByText('4')).toBeInTheDocument()
  })
})
