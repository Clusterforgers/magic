// Goal: the host's visibility toggle sends the other choice through setVisibility with the seat's token, shows its pending state with every other button waiting, says the result in words and keeps it, puts the old value back with the reason in the lobby's words when the request fails, and ends with the controls gone when the game has started, while a guest sees the visibility and cannot change it.
import { screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { clearFlash } from '@/lib/flash'
import type { Visibility } from '@/protocol/generated'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { tokenOf } from '@/test-utils/fakeServer'
import { GAME_ID, renderGame } from '@/test-utils/gameHarness'
import { changesOf, FAILURES, held, openLobby } from '@/test-utils/lobbyControls'
import { LOBBY_POLL_MS } from './useGameInfo'

afterEach(clearFlash)

const LISTED = 'Listed in the game browser'
const INVITE = 'Invite only'
const choice = (name: string): HTMLElement => screen.getByRole('button', { name })
const note = (): Element | null => document.querySelector('[data-slot="visibility-note"]')
const problem = (): Element | null => document.querySelector('[data-slot="visibility-problem"]')
const stopped = (): Element | null => document.querySelector('[data-slot="lobby-stopped"]')
const line = (): Element | null => document.querySelector('[data-slot="lobby-visibility"]')

describe('what the host sees', () => {
  it.each<[Visibility, string, string]>([
    ['public', LISTED, INVITE],
    ['invite', INVITE, LISTED],
  ])('presses the choice of a %s game (%s) and not the other (%s)', async (visibility, on, off) => {
    await openLobby({ visibility })
    expect(choice(on)).toHaveAttribute('aria-pressed', 'true')
    expect(choice(off)).toHaveAttribute('aria-pressed', 'false')
    expect(line()).toHaveAttribute('data-visibility', visibility)
    expect(screen.getByRole('group', { name: 'Who can find this game' })).toBeInTheDocument()
  })

  it('offers nothing until the lookup has said what the visibility is', async () => {
    const h = await renderGame()
    h.join()
    await h.settle()
    expect(screen.queryByRole('group', { name: 'Who can find this game' })).toBeNull()
    expect(line()).toBeNull()
  })

  it('reads the lobby in order: the toggle, the players, the decks to choose, Ready, then Close game', async () => {
    await openLobby()
    expect(
      screen.getAllByRole('button').map((button) => button.getAttribute('aria-label') ?? button.textContent),
    ).toEqual([
      LISTED,
      INVITE,
      'Remove Bo from the game',
      'Mono Green StompersNormal, Green, 60 cards',
      'Mono Red GiantsNormal, Red, 60 cards',
      'Mono White WeeniesNormal, White, 60 cards',
      'Ready',
      'Close game',
    ])
  })
})

describe('a choice that goes through', () => {
  it.each<[Visibility, string, string]>([
    ['invite', INVITE, 'The game is now invite only. Only people with your link or code can join.'],
    ['public', LISTED, 'The game is now listed in the game browser.'],
  ])('sends %s with the seat token and says so in words', async (next, name, words) => {
    const h = await openLobby({ visibility: next === 'invite' ? 'public' : 'invite' })
    await h.user.click(choice(name))
    await h.settle()
    expect(changesOf(h.server)).toEqual([
      {
        method: 'PATCH',
        path: `/api/games/${GAME_ID}`,
        authorization: `Bearer ${tokenOf(0)}`,
        body: { visibility: next },
      },
    ])
    expect(choice(name)).toHaveAttribute('aria-pressed', 'true')
    expect(line()).toHaveAttribute('data-visibility', next)
    expect(note()).toHaveTextContent(words)
    expect(note()).toHaveAttribute('aria-live', 'polite')
    expect(problem()).toBeNull()
  })

  it('works from the keyboard', async () => {
    const h = await openLobby()
    choice(INVITE).focus()
    await h.user.keyboard('{Enter}')
    await h.settle()
    expect(changesOf(h.server)).toHaveLength(1)
    expect(choice(INVITE)).toHaveAttribute('aria-pressed', 'true')
  })

  it('sends nothing for the choice that is already made', async () => {
    const h = await openLobby()
    await h.user.click(choice(LISTED))
    expect(changesOf(h.server)).toEqual([])
  })

  it('acts through the session, with no token, for a signed-in visitor without a seat link', async () => {
    const h = await openLobby({ account: 'ana' })
    await h.user.click(choice(INVITE))
    await h.settle()
    const [sent] = changesOf(h.server)
    expect(sent?.authorization).toBeNull()
    expect(sent?.body).toEqual({ visibility: 'invite' })
  })
})

describe('while the request is out', () => {
  it('shows the choice at once with a spinner, and every other button waits', async () => {
    const gate = held()
    const h = await openLobby({ change: gate.answer })
    await h.user.click(choice(INVITE))
    expect(choice(INVITE)).toHaveAttribute('aria-busy', 'true')
    expect(choice(INVITE)).toHaveAttribute('aria-pressed', 'true')
    expect(note()).toHaveTextContent('Making the game invite only…')
    expect(line()).toHaveAttribute('data-pending', 'invite')
    for (const name of [LISTED, 'Ready', 'Close game', 'Remove Bo from the game']) {
      expect(screen.getByRole('button', { name })).toBeDisabled()
    }
    // The button that was pressed keeps the focus, so a keyboard user does not lose their place.
    expect(choice(INVITE)).toHaveFocus()
    gate.release(jsonResponse(200, { visibility: 'invite' }))
    await h.settle()
    expect(choice(INVITE)).not.toHaveAttribute('aria-busy')
    for (const name of [LISTED, 'Ready', 'Close game', 'Remove Bo from the game']) {
      expect(screen.getByRole('button', { name })).toBeEnabled()
    }
  })

  it('sends one request however often the buttons are pressed meanwhile', async () => {
    const gate = held()
    const h = await openLobby({ change: gate.answer })
    await h.user.click(choice(INVITE))
    await h.user.click(choice(INVITE))
    await h.user.click(choice(LISTED))
    expect(changesOf(h.server)).toHaveLength(1)
    gate.release(jsonResponse(200, { visibility: 'invite' }))
    await h.settle()
    expect(choice(INVITE)).toHaveAttribute('aria-pressed', 'true')
  })
})

describe('a choice that is refused', () => {
  it.each([
    [
      'a player who is not the host (403)',
      FAILURES.forbidden,
      'Only the host can change who can find the game.',
    ],
    ['a game that is gone (404)', FAILURES.gone, 'This game is no longer open.'],
    ['a server error (500)', FAILURES.server, 'The server ran into a problem. Try again in a moment.'],
    ['a server that is restarting (503)', FAILURES.busy, 'The server is busy or restarting.'],
    [
      'an answer that is not the server’s (502)',
      FAILURES.proxy,
      'The server gave an answer this page could not read',
    ],
    ['no connection', FAILURES.offline, 'Could not reach the server.'],
  ])('puts the old value back and says why after %s', async (_what, answer, words) => {
    const h = await openLobby({ change: answer })
    await h.user.click(choice(INVITE))
    await h.settle()
    expect(choice(LISTED)).toHaveAttribute('aria-pressed', 'true')
    expect(choice(INVITE)).toHaveAttribute('aria-pressed', 'false')
    expect(line()).toHaveAttribute('data-visibility', 'public')
    expect(problem()).toHaveTextContent('The game is still listed in the game browser.')
    expect(problem()).toHaveTextContent(words)
    expect(note()).toBeEmptyDOMElement()
    expect(choice(INVITE)).toBeEnabled()
  })

  it('says an invite only game is still invite only', async () => {
    const h = await openLobby({ visibility: 'invite', change: FAILURES.server })
    await h.user.click(choice(LISTED))
    await h.settle()
    expect(choice(INVITE)).toHaveAttribute('aria-pressed', 'true')
    expect(problem()).toHaveTextContent('The game is still invite only.')
  })

  it('takes the words away and tries again when the host presses once more', async () => {
    let answer: () => Response | Promise<Response> = FAILURES.server
    const h = await openLobby({ change: () => answer() })
    await h.user.click(choice(INVITE))
    await h.settle()
    expect(problem()).not.toBeNull()
    answer = () => jsonResponse(200, { visibility: 'invite' })
    await h.user.click(choice(INVITE))
    await h.settle()
    expect(problem()).toBeNull()
    expect(choice(INVITE)).toHaveAttribute('aria-pressed', 'true')
    expect(changesOf(h.server)).toHaveLength(2)
  })

  it('ends the lobby’s controls, and says why, when the game has started meanwhile', async () => {
    const h = await openLobby({ change: FAILURES.started })
    await h.user.click(choice(INVITE))
    await h.settle()
    expect(stopped()).toHaveTextContent(
      'The game has already started, so who can find it can no longer change.',
    )
    expect(stopped()).toHaveFocus()
    expect(screen.queryByRole('group', { name: 'Who can find this game' })).toBeNull()
    for (const name of ['Close game', 'Remove Bo from the game']) {
      expect(screen.queryByRole('button', { name })).toBeNull()
    }
    // The status line stays, in words.
    expect(line()).toHaveTextContent(LISTED)
    expect(problem()).toBeNull()
  })
})

describe('the lookup and the toggle', () => {
  it('keeps the answer until a look says the same, then follows the lookup again', async () => {
    let served: Visibility = 'public'
    const h = await openLobby({ visibility: () => served })
    await h.user.click(choice(INVITE))
    await h.settle()
    // A look that was already on its way still says listed: it cannot flip the toggle back.
    await h.advance(LOBBY_POLL_MS)
    await h.settle()
    expect(choice(INVITE)).toHaveAttribute('aria-pressed', 'true')
    served = 'invite'
    await h.advance(LOBBY_POLL_MS)
    await h.settle()
    expect(choice(INVITE)).toHaveAttribute('aria-pressed', 'true')
    // A change made from another tab shows too.
    served = 'public'
    await h.advance(LOBBY_POLL_MS)
    await h.settle()
    expect(choice(LISTED)).toHaveAttribute('aria-pressed', 'true')
  })

  it('takes the controls away when a look says the game has started', async () => {
    await openLobby({ started: true })
    expect(screen.queryByRole('group', { name: 'Who can find this game' })).toBeNull()
    expect(stopped()).toHaveTextContent(
      'This game has already started, so the lobby can no longer be changed.',
    )
    expect(line()).toHaveTextContent(LISTED)
  })
})

describe('a guest', () => {
  it.each<Visibility>(['public', 'invite'])(
    'sees a %s game in words and cannot change it',
    async (visibility) => {
      await openLobby({ seat: 1, visibility })
      expect(line()).toHaveTextContent(visibility === 'public' ? LISTED : INVITE)
      expect(line()).toHaveAttribute('data-visibility', visibility)
      expect(screen.queryByRole('group', { name: 'Who can find this game' })).toBeNull()
      expect(screen.queryByRole('button', { name: LISTED })).toBeNull()
      expect(screen.queryByRole('button', { name: INVITE })).toBeNull()
    },
  )
})
