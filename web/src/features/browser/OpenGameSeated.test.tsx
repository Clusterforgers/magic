// Goal: a game the visitor holds a seat of is never Full to them: its card says "Your seat", opens the game page with an Open link in place of Join (lobby or running, full or not), comes first in the list and stays through every filter, while the same game without the mark keeps the Full, Join and Watch states it always had.
import { screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import type { LobbyGame } from '@/protocol/generated'
import { installFakeClock, removeFakeClock, settle } from '@/test-utils/fakeClock'
import { fakeLobbies, lobbyRow } from '@/test-utils/lobbies'
import { renderInRouter } from '@/test-utils/router'
import { filterGames, isFull, NO_FILTERS } from './format'
import { PlayPage } from './PlayPage'

beforeEach(installFakeClock)
afterEach(removeFakeClock)

async function cardOf(game: LobbyGame): Promise<HTMLElement> {
  await renderInRouter(<PlayPage api={fakeLobbies([game]).api} />)
  await settle()
  return screen.getByRole('article', { name: game.host })
}

describe('the primary control of a card', () => {
  it.each([
    ['a full lobby', { filled: 4, total: 4 }, 'join-disabled', 'Full'],
    ['a full lobby the visitor sits in', { filled: 4, total: 4, seated: true }, 'open', 'Your seat'],
    ['a lobby with room', { filled: 2, total: 4 }, 'join', null],
    ['a lobby with room the visitor sits in', { filled: 2, total: 4, seated: true }, 'open', 'Your seat'],
    ['a running game', { filled: 4, total: 4, running: true }, 'watch', 'Running'],
    [
      'a running game the visitor sits in',
      { filled: 4, total: 4, running: true, seated: true },
      'open',
      'Your seat',
    ],
  ] as const)('%s', async (_, over, control, pill) => {
    const card = await cardOf(lobbyRow({ host: 'Bo', ...over }))
    const item = card.closest('li')
    const join = within(card).queryByRole('button', { name: /^Join/ })
    const open = within(card).queryByRole('link', { name: "Open your seat in Bo's game" })
    const watch = within(card).queryByRole('link', { name: "Watch Bo's game" })
    if (control === 'open') {
      expect(open).toHaveAttribute('href', `/game/${lobbyRow().game}`)
      expect(join).toBeNull()
      expect(watch).toBeNull()
      expect(item).toHaveAttribute('data-seated')
      expect(item).not.toHaveAttribute('data-full')
    } else {
      expect(open).toBeNull()
      expect(item).not.toHaveAttribute('data-seated')
    }
    if (control === 'join') expect(join).toBeEnabled()
    if (control === 'join-disabled') expect(join).toHaveAttribute('aria-disabled', 'true')
    if (control === 'watch') expect(watch).not.toBeNull()
    for (const word of ['Full', 'Your seat', 'Running']) {
      const shown = within(card).queryByText(word, { exact: true }) !== null
      expect(shown, word).toBe(word === pill)
    }
  })
})

describe('the games a visitor sits in', () => {
  const mine = lobbyRow({ game: 'mine', filled: 4, total: 4, format: 'commander', seated: true })
  const rows = [lobbyRow({ game: 'a' }), lobbyRow({ game: 'b', total: 2, filled: 2 }), mine]

  it('are never full to them', () => {
    expect(isFull(mine)).toBe(false)
    expect(isFull({ ...mine, seated: false })).toBe(true)
  })

  it.each([
    ['no filter', NO_FILTERS, ['mine', 'a', 'b']],
    ['hiding full games', { ...NO_FILTERS, hideFull: true }, ['mine', 'a']],
    ['another format', { ...NO_FILTERS, format: 'normal' }, ['mine', 'a', 'b']],
    ['another seat count', { ...NO_FILTERS, seats: 2 }, ['mine', 'b']],
  ] as const)('come first and pass every filter: %s', (_, filters, expected) => {
    expect(filterGames(rows, filters).map((row) => row.game)).toEqual(expected)
  })
})
