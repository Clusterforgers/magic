/** Goal: the Mulligan prompt offers "Exile hand and draw" only while a card like Serum Powder is in the hand, and its click answers the exile of the hand by that card; the generic choices offer the same answer. */
import { act, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer, MulliganOptions, ObjectId } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { buildFixture } from '../dev/fixture'
import { genericChoices } from './genericChoices'

const hand = must(buildFixture({ seats: 2 }).view.players[0]?.hand.cards)
const options = (powders: ObjectId[]): MulliganOptions => ({
  hand,
  mulligans_taken: 0,
  can_mulligan: true,
  free_mulligans: 0,
  bottom_if_keep: 0,
  bottom_if_mulligan: 1,
  ...(powders.length === 0 ? {} : { r115_powders: powders }),
})

function renderPowder(given: MulliganOptions) {
  const answer = vi.fn((_answer: Answer) => true)
  renderTable({
    seats: 2,
    session: { answer },
    options: { viewport: { width: 390, height: 844 } },
    state: (state) => {
      const view = clone(must(state.view))
      view.stage = 'Mulligan'
      must(view.players[0]).mulligan = 'Deciding'
      return { ...state, view, pending: { id: 3, decision: { player: 0, kind: { Mulligan: given } } } }
    },
  })
  return answer
}

describe('Serum Powder at the mulligan', () => {
  it.each([{ powders: [] as ObjectId[] }, { powders: [must(hand[0])] }])(
    'offers the exile only with a powder in hand ($powders)',
    ({ powders }) => {
      renderPowder(options(powders))
      const offered = screen.queryAllByRole('button', { name: 'Exile hand and draw' })
      expect(offered.length > 0).toBe(powders.length > 0)
    },
  )

  it('answers the exile by the powder', () => {
    const powder = must(hand[0])
    const answer = renderPowder(options([powder]))
    act(() => must(screen.getAllByRole('button', { name: 'Exile hand and draw' })[0]).click())
    expect(answer).toHaveBeenCalledWith({ Mulligan: { R115ExileHand: powder } })
  })

  it('is among the generic choices', () => {
    const powder = must(hand[0])
    const kind = { Mulligan: options([powder]) }
    const offer = genericChoices(kind, {} as Parameters<typeof genericChoices>[1])
    expect(JSON.stringify(offer.choices)).toContain('R115ExileHand')
  })
})
