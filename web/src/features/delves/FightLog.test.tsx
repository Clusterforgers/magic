// Goal: the run page keeps a fight log: each fight whose game ended, newest first, with its boss, won or lost, and a link to its recorded game; no log before a fight has ended.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { renderInRouter } from '@/test-utils/router'
import { DelvePage } from './DelvePage'
import { delveView, fakeDelves } from './fakeDelves'
import { pickerDecks } from './fakePickerDecks'
import type { DelveView } from '@/protocol/generated'

async function show(view: DelveView) {
  await renderInRouter(
    <DelvePage id="d1" client={fakeDelves(view)} loadDecks={pickerDecks} find={async () => null} />,
  )
}

describe('the fight log', () => {
  it('lists each finished fight newest first with its result and a link to its game', async () => {
    await show(
      delveView(
        {},
        {
          status: 'Room',
          at: 0,
          cleared: [0],
          fights: [
            { game: 'g-lost', room: 0, floor: 1, boss: null, won: false },
            { game: 'g-won', room: 0, floor: 1, boss: null, won: true },
          ],
        },
      ),
    )
    const log = await screen.findByRole('region', { name: 'Fight log' })
    const rows = within(log).getAllByRole('listitem')
    expect(rows.map((row) => row.textContent)).toEqual([
      'Skrix, the Rat QueenWonFight log',
      'Skrix, the Rat QueenLostFight log',
    ])
    expect(
      within(log).getByRole('link', { name: 'Fight log of fight 2 against Skrix, the Rat Queen' }),
    ).toHaveAttribute('href', '/replay/g-won')
    expect(
      within(log).getByRole('link', { name: 'Fight log of fight 1 against Skrix, the Rat Queen' }),
    ).toHaveAttribute('href', '/replay/g-lost')
  })

  it('is not there before a fight has ended', async () => {
    await show(delveView({}, { status: { Fight: { game: null } }, at: 0 }))
    await screen.findByRole('heading', { name: 'Chronicle' })
    expect(screen.queryByRole('region', { name: 'Fight log' })).toBeNull()
  })
})
