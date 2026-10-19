// Goal: the host gets a Remove button on each player who joined, and on no other seat (not the host's own, an open seat, a seat the game came with, or anything for a guest); the question names the player and sends nothing on No, Yes sends one release for that seat with the host's token and says the result in words with the focus on the seat list, and a refusal keeps the question up in the lobby's words.
import { screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { clearFlash } from '@/lib/flash'
import { saveInvites } from '@/lib/invites'
import { tokenOf } from '@/test-utils/fakeServer'
import { GAME_ID } from '@/test-utils/gameHarness'
import { changesOf, FAILURES, held, openLobby, seatItems } from '@/test-utils/lobbyControls'
import type { LobbyScene } from '@/test-utils/lobbyControls'
import { lobbyMessage } from '@/test-utils/lobbyFrames'
import type { SeatFlags } from '@/test-utils/lobbyFrames'

afterEach(() => {
  clearFlash()
  window.sessionStorage.clear()
})

const dialog = (): HTMLElement | null => screen.queryByRole('alertdialog')
const removeIn = (row: HTMLElement): HTMLElement | null =>
  within(row).queryByRole('button', { name: /^Remove/ })
const stopped = (): Element | null => document.querySelector('[data-slot="lobby-stopped"]')
const removed = (): Element | null => document.querySelector('[data-slot="lobby-removed"]')
const FOUR = ['Ana', 'Bo', 'Cy', '']
const noContent = (): Response => new Response(null, { status: 204 })

/** The lobby with the question about Bo (seat 2) on screen. */
async function askAboutBo(scene: LobbyScene = {}) {
  const h = await openLobby({ names: FOUR, ...scene })
  await h.user.click(screen.getByRole('button', { name: 'Remove Bo from the game' }))
  const question = await screen.findByRole('alertdialog', { name: 'Remove Bo?' })
  return { h, question }
}

describe('who can be removed', () => {
  it('is each player who joined, and not the host, an open seat or the page itself', async () => {
    await openLobby({ names: FOUR })
    expect(seatItems().map((row) => removeIn(row)?.getAttribute('aria-label') ?? null)).toEqual([
      null,
      'Remove Bo from the game',
      'Remove Cy from the game',
      null,
    ])
  })

  it('gives each button the visible word Remove, at least 32 px tall and 44 under a coarse pointer', async () => {
    await openLobby({ names: FOUR })
    const button = screen.getByRole('button', { name: 'Remove Bo from the game' })
    expect(button).toHaveTextContent('Remove')
    expect(button.className).toMatch(/\bh-8\b/)
    expect(button.className).toMatch(/pointer-coarse:h-11/)
  })

  it('leaves out the seats this tab holds a token for, which came with the game and cannot be given back', async () => {
    saveInvites({
      game: GAME_ID,
      seats: [
        { seat: 0, name: 'Ana', token: tokenOf(0) },
        { seat: 1, name: 'Bo', token: tokenOf(1) },
      ],
    })
    await openLobby({ names: FOUR })
    expect(seatItems().map((row) => removeIn(row) !== null)).toEqual([false, false, true, false])
  })

  it('offers nothing to a guest', async () => {
    await openLobby({ names: FOUR, seat: 1 })
    expect(screen.queryByRole('button', { name: /^Remove/ })).toBeNull()
  })

  it('follows the roster: a seat that opens loses its button and one that fills gets it', async () => {
    const h = await openLobby({ names: FOUR })
    const flags = (names: string[]): SeatFlags[] => names.map((name): SeatFlags => [name, name !== '', false])
    h.deliver(lobbyMessage(flags(['Ana', 'Bo', '', 'Di'])))
    expect(seatItems().map((row) => removeIn(row) !== null)).toEqual([false, true, false, true])
  })
})

describe('the question', () => {
  it('names the player and the seat, and sends nothing yet', async () => {
    const { h, question } = await askAboutBo()
    expect(within(question).getByText(/Bo is taken out of seat 2 and sent back to Play/)).toBeInTheDocument()
    expect(within(question).getByText(/The seat opens for someone else to take/)).toBeInTheDocument()
    expect(within(question).getByRole('button', { name: 'Remove Bo' })).toBeInTheDocument()
    expect(changesOf(h.server)).toEqual([])
  })

  it('asks about the player whose button was pressed', async () => {
    const h = await openLobby({ names: FOUR })
    await h.user.click(screen.getByRole('button', { name: 'Remove Cy from the game' }))
    const question = await screen.findByRole('alertdialog', { name: 'Remove Cy?' })
    expect(within(question).getByText(/Cy is taken out of seat 3/)).toBeInTheDocument()
  })

  it('puts the focus on the safe answer, and gives it back to the same button on No', async () => {
    const { h, question } = await askAboutBo()
    await waitFor(() => expect(within(question).getByRole('button', { name: 'Keep Bo' })).toHaveFocus())
    await h.user.click(within(question).getByRole('button', { name: 'Keep Bo' }))
    await waitFor(() => expect(dialog()).toBeNull())
    expect(changesOf(h.server)).toEqual([])
    await waitFor(() => expect(screen.getByRole('button', { name: 'Remove Bo from the game' })).toHaveFocus())
  })

  it('sends nothing on Escape either, and the focus is back on the button', async () => {
    const { h } = await askAboutBo()
    await h.user.keyboard('{Escape}')
    await waitFor(() => expect(dialog()).toBeNull())
    expect(changesOf(h.server)).toEqual([])
    await waitFor(() => expect(screen.getByRole('button', { name: 'Remove Bo from the game' })).toHaveFocus())
  })
})

describe('saying yes', () => {
  it('sends one release for that seat with the host’s token and says the result in words', async () => {
    const { h, question } = await askAboutBo()
    await h.user.click(within(question).getByRole('button', { name: 'Remove Bo' }))
    await waitFor(() => expect(dialog()).toBeNull())
    expect(changesOf(h.server)).toEqual([
      {
        method: 'POST',
        path: `/api/games/${GAME_ID}/seats/1/release`,
        authorization: `Bearer ${tokenOf(0)}`,
        body: null,
      },
    ])
    expect(removed()).toHaveTextContent('Bo was removed. Seat 2 is open.')
    expect(removed()).toHaveAttribute('aria-live', 'polite')
    expect(h.router.state.location.pathname).toBe(`/game/${GAME_ID}`)
  })

  it('puts the focus on the seat list, since the button it was on goes with the player', async () => {
    const { h, question } = await askAboutBo()
    await h.user.click(within(question).getByRole('button', { name: 'Remove Bo' }))
    await waitFor(() => expect(dialog()).toBeNull())
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Seats' })).toHaveFocus())
    // The server then tells the table, and the seat reads as one nobody has joined yet.
    h.deliver(
      lobbyMessage([
        ['Ana', true, false],
        ['', false, false],
        ['Cy', true, false],
        ['', false, false],
      ]),
    )
    expect(seatItems().map((row) => removeIn(row) !== null)).toEqual([false, false, true, false])
  })

  it('makes every button wait while the request is out, and sends it once', async () => {
    const gate = held()
    const { h, question } = await askAboutBo({ change: gate.answer })
    await h.user.click(within(question).getByRole('button', { name: 'Remove Bo' }))
    expect(within(question).getByRole('button', { name: 'Remove Bo' })).toHaveAttribute('aria-busy', 'true')
    expect(within(question).getByRole('button', { name: 'Keep Bo' })).toBeDisabled()
    for (const button of document.querySelectorAll('[data-slot="remove-player"]'))
      expect(button).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Ready', hidden: true })).toBeDisabled()
    await h.user.click(within(question).getByRole('button', { name: 'Remove Bo' }))
    expect(changesOf(h.server)).toHaveLength(1)
    gate.release(noContent())
    await waitFor(() => expect(dialog()).toBeNull())
    for (const button of document.querySelectorAll('[data-slot="remove-player"]'))
      expect(button).toBeEnabled()
  })
})

describe('a refusal', () => {
  it.each([
    ['a player who is not the host (403)', FAILURES.forbidden, 'Only the host can remove a player.'],
    ['a game that is gone (404)', FAILURES.gone, 'This game is no longer open.'],
    [
      'a seat that came with the game (409)',
      FAILURES.notClaimed,
      'not taken through the game browser or a code, so its player cannot be removed.',
    ],
    ['a server error (500)', FAILURES.server, 'The server ran into a problem.'],
    ['a server that is restarting (503)', FAILURES.busy, 'The server is busy or restarting.'],
    [
      'an answer that is not the server’s (502)',
      FAILURES.proxy,
      'The server gave an answer this page could not read',
    ],
    ['no connection', FAILURES.offline, 'Could not reach the server.'],
  ])('keeps the question up with the reason after %s', async (_what, answer, words) => {
    const { h, question } = await askAboutBo({ change: answer })
    await h.user.click(within(question).getByRole('button', { name: 'Remove Bo' }))
    await waitFor(() => expect(within(question).getByRole('alert')).toHaveTextContent(words))
    expect(dialog()).not.toBeNull()
    expect(within(question).getByRole('button', { name: 'Remove Bo' })).toBeEnabled()
    expect(within(question).getByRole('button', { name: 'Keep Bo' })).toBeEnabled()
    expect(removed()).toBeEmptyDOMElement()
    // Bo is still in the list.
    expect(seatItems()[1]).toHaveTextContent('Bo')
  })

  it('can be tried again from the same question', async () => {
    let answer: () => Response | Promise<Response> = FAILURES.server
    const { h, question } = await askAboutBo({ change: () => answer() })
    await h.user.click(within(question).getByRole('button', { name: 'Remove Bo' }))
    await waitFor(() => expect(within(question).getByRole('alert')).toBeInTheDocument())
    answer = noContent
    await h.user.click(within(question).getByRole('button', { name: 'Remove Bo' }))
    await waitFor(() => expect(dialog()).toBeNull())
    expect(removed()).toHaveTextContent('Bo was removed.')
    expect(changesOf(h.server)).toHaveLength(2)
  })

  it('closes the question and takes the controls away when the game has started meanwhile', async () => {
    const { h, question } = await askAboutBo({ change: FAILURES.started })
    await h.user.click(within(question).getByRole('button', { name: 'Remove Bo' }))
    await waitFor(() => expect(dialog()).toBeNull())
    expect(stopped()).toHaveTextContent('The game has already started, so players can no longer be removed.')
    await waitFor(() => expect(stopped()).toHaveFocus())
    expect(screen.queryByRole('button', { name: /^Remove/ })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Close game' })).toBeNull()
    expect(screen.queryByRole('group', { name: 'Who can find this game' })).toBeNull()
    expect(removed()).toBeEmptyDOMElement()
  })
})
