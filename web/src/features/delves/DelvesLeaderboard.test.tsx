// Goal: the leaderboard opens on the greatest Heat whatever order the boards come in, offers one tab per Heat from Heroic to Casual that a keyboard reaches, lists each entry with its rank, depth in words (floors cleared, then bosses beaten below them), commanders, party size and day, says when a Heat has no run yet, and says when it could not load.
import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { DelveError } from '@/net/delves'
import { renderInRouter } from '@/test-utils/router'
import { DelvesLeaderboard } from './DelvesLeaderboard'
import { depthWords } from './model'
import { delveView, fakeDelves, LEADERBOARD } from './fakeDelves'

describe('depthWords', () => {
  it.each([
    [0, 1, '1 boss'],
    [0, 3, '3 bosses'],
    [1, 0, '1 floor'],
    [2, 1, '2 floors + 1 boss'],
    [3, 2, '3 floors + 2 bosses'],
  ])('%i floors and %i bosses read "%s"', (floors, bosses, words) => {
    expect(depthWords({ floors, bosses })).toBe(words)
  })
})

describe('DelvesLeaderboard', () => {
  it.each([
    ['as sent', LEADERBOARD.boards],
    ['lowest first', [...LEADERBOARD.boards].reverse()],
  ])('opens on Heroic with the tabs from the greatest Heat (%s)', async (_, boards) => {
    const client = { ...fakeDelves(delveView()), leaderboard: async () => ({ boards }) }
    await renderInRouter(<DelvesLeaderboard client={client} />)
    const tabs = await screen.findAllByRole('tab')
    expect(tabs.map((tab) => tab.textContent)).toEqual(['Heroic', 'Seasoned', 'Casual'])
    expect(screen.getByRole('tab', { name: 'Heroic' })).toHaveAttribute('aria-selected', 'true')
    const list = screen.getByRole('list', { name: 'Heroic leaderboard' })
    const rows = within(list).getAllByRole('listitem')
    expect(rows).toHaveLength(2)
    expect(rows[0]).toHaveTextContent('1Ana')
    expect(rows[0]).toHaveTextContent('2 floors + 1 boss')
    expect(rows[0]).toHaveTextContent('Party of 3')
    expect(rows[0]).toHaveTextContent('2026-11-12')
    expect(rows[1]).toHaveTextContent('Thrasios, Triton Hero & Tymna the Weaver')
    expect(rows[1]).toHaveTextContent('Solo')
  })

  it('switches Heat from the keyboard and says when a Heat has no run', async () => {
    const user = userEvent.setup()
    await renderInRouter(<DelvesLeaderboard client={fakeDelves(delveView())} />)
    await user.click(await screen.findByRole('tab', { name: 'Heroic' }))
    await user.keyboard('{ArrowRight}')
    expect(screen.getByRole('tab', { name: 'Seasoned' })).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(await screen.findByText('No run at Seasoned yet.')).toBeVisible()
    await user.click(screen.getByRole('tab', { name: 'Casual' }))
    expect(within(screen.getByRole('list', { name: 'Casual leaderboard' })).getByText('Cato')).toBeVisible()
  })

  it('says when the leaderboard could not load', async () => {
    const client = {
      ...fakeDelves(delveView()),
      leaderboard: async () => {
        throw new DelveError('unavailable', 503, 'Try again later.')
      },
    }
    await renderInRouter(<DelvesLeaderboard client={client} />)
    expect(await screen.findByRole('alert')).toHaveTextContent('Could not load the leaderboard.')
  })
})
