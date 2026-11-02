// Goal: the companion chip finds its card's picture through one card search per name, however often the tooltip opens.
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { TooltipProvider } from '@/components/ui/tooltip'
import type { CardsApi } from '@/net/cards'
import type { PlayerView } from '@/protocol/generated'
import { CardsApiContext } from './cardsContext'
import { CompanionChip } from './CompanionChip'

describe('CompanionChip', () => {
  it('searches once for a name, however many times the card is shown', async () => {
    const search = vi.fn(() =>
      Promise.resolve({ cards: [{ oracle_id: 'o-lurrus', name: 'Lurrus of the Dream-Den' }] }),
    )
    const api = { search } as unknown as CardsApi
    const player = {
      companion: { stage: 'Revealed', name: 'Lurrus of the Dream-Den' },
    } as unknown as PlayerView
    const user = userEvent.setup()
    render(
      <CardsApiContext.Provider value={api}>
        <TooltipProvider delay={0}>
          <CompanionChip owner="Ann" player={player} />
        </TooltipProvider>
      </CardsApiContext.Provider>,
    )
    const chip = screen.getByRole('img', { name: /companion/ })
    for (let round = 0; round < 3; round += 1) {
      await user.hover(chip)
      await waitFor(() => expect(screen.getAllByText('Lurrus of the Dream-Den').length).toBeGreaterThan(0))
      await user.unhover(chip)
    }
    expect(search).toHaveBeenCalledTimes(1)
  })
})
