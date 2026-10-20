// Goal: Close game (the host) and Leave game (a guest) ask first and name what happens, send nothing on No or Escape and give the focus back to their button, send one request with the seat's token on Yes and then take the visitor to Play with a notice, keep the question up in the lobby's words when the request is refused, make every button wait while it is out, and lose their controls when the game has started meanwhile.
import { screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { clearFlash } from '@/lib/flash'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { tokenOf } from '@/test-utils/fakeServer'
import { GAME_ID } from '@/test-utils/gameHarness'
import { changesOf, FAILURES, held, openLobby } from '@/test-utils/lobbyControls'
import type { LobbyScene } from '@/test-utils/lobbyControls'

afterEach(clearFlash)

const dialog = (): HTMLElement | null => screen.queryByRole('alertdialog')
const stopped = (): Element | null => document.querySelector('[data-slot="lobby-stopped"]')
const noContent = (): Response => new Response(null, { status: 204 })

/** A refusal: what the player meets, how the server answers, and the words that must come of it. */
type Refusal = [what: string, answer: () => Response | Promise<Response>, words: string]

/** One of the two buttons, its question, what it sends, and the refusals it must put in words. */
interface Case {
  who: string
  button: string
  scene: LobbyScene
  title: string
  says: string
  keep: string
  method: string
  path: string
  token: string
  notice: string
  refused: Refusal[]
  started: string
}

const CASES: Case[] = [
  {
    who: 'the host',
    button: 'Close game',
    scene: { seat: 0 },
    title: 'Close this game?',
    says: 'Everyone in the lobby is sent back to Play.',
    keep: 'Keep game open',
    method: 'DELETE',
    path: `/api/games/${GAME_ID}`,
    token: tokenOf(0),
    notice: 'You closed the game. Everyone who was in the lobby is back on Play.',
    refused: [
      ['a player who is not the host (403)', FAILURES.forbidden, 'Only the host can close the game.'],
      ['a game that is gone (404)', FAILURES.gone, 'This game is already closed or gone.'],
      ['a server error (500)', FAILURES.server, 'The server ran into a problem.'],
      ['a server that is restarting (503)', FAILURES.busy, 'The server is busy or restarting.'],
      [
        'an answer that is not the server’s (502)',
        FAILURES.proxy,
        'The server gave an answer this page could not read',
      ],
      ['no connection', FAILURES.offline, 'Could not reach the server.'],
    ],
    started: 'The game has already started, so it can no longer be closed from the lobby.',
  },
  {
    who: 'a guest',
    button: 'Leave game',
    scene: { seat: 1 },
    title: 'Leave this game?',
    says: 'Your seat opens for someone else and you go back to Play.',
    keep: 'Stay in the game',
    method: 'POST',
    path: `/api/games/${GAME_ID}/leave`,
    token: tokenOf(1),
    notice: 'You left the game.',
    refused: [
      [
        'a seat that came with the game (409)',
        FAILURES.notClaimed,
        "This seat came with the game's own link",
      ],
      ['a seat the guest no longer has (404)', FAILURES.gone, 'You no longer have a seat in this game.'],
      ['a server error (500)', FAILURES.server, 'The server ran into a problem.'],
      ['a server that is restarting (503)', FAILURES.busy, 'The server is busy or restarting.'],
      [
        'an answer that is not the server’s (502)',
        FAILURES.proxy,
        'The server gave an answer this page could not read',
      ],
      ['no connection', FAILURES.offline, 'Could not reach the server.'],
    ],
    started: 'The game has already started, so a seat can no longer be given back.',
  },
]

describe.each(CASES)('$button for $who', (kase) => {
  /** The lobby with the question for this button on screen. */
  async function ask(scene: LobbyScene = {}) {
    const h = await openLobby({ ...kase.scene, ...scene })
    await h.user.click(screen.getByRole('button', { name: kase.button }))
    const question = await screen.findByRole('alertdialog', { name: kase.title })
    return { h, question }
  }
  const yes = (question: HTMLElement): HTMLElement =>
    within(question).getByRole('button', { name: kase.button })
  const no = (question: HTMLElement): HTMLElement => within(question).getByRole('button', { name: kase.keep })

  it('asks first, says what happens, and sends nothing yet', async () => {
    const { h, question } = await ask()
    expect(within(question).getByText(new RegExp(kase.says))).toBeInTheDocument()
    expect(changesOf(h.server)).toEqual([])
    expect(h.router.state.location.pathname).toBe(`/game/${GAME_ID}`)
  })

  it('puts the focus on the safe answer', async () => {
    const { question } = await ask()
    await waitFor(() => expect(no(question)).toHaveFocus())
  })

  it('sends nothing on No, and gives the focus back to the button', async () => {
    const { h, question } = await ask()
    await h.user.click(no(question))
    await waitFor(() => expect(dialog()).toBeNull())
    expect(changesOf(h.server)).toEqual([])
    await waitFor(() => expect(screen.getByRole('button', { name: kase.button })).toHaveFocus())
  })

  it('sends nothing on Escape either', async () => {
    const { h } = await ask()
    await h.user.keyboard('{Escape}')
    await waitFor(() => expect(dialog()).toBeNull())
    expect(changesOf(h.server)).toEqual([])
    await waitFor(() => expect(screen.getByRole('button', { name: kase.button })).toHaveFocus())
  })

  it('sends one request with the seat token on Yes, then takes the visitor to Play with a notice', async () => {
    const { h, question } = await ask()
    await h.user.click(yes(question))
    await waitFor(() => expect(h.router.state.location.pathname).toBe('/'))
    expect(changesOf(h.server)).toEqual([
      { method: kase.method, path: kase.path, authorization: `Bearer ${kase.token}`, body: null },
    ])
    expect(await screen.findByRole('heading', { name: 'Play' })).toBeInTheDocument()
    expect(screen.getByRole('alert')).toHaveTextContent(kase.notice)
    expect(screen.queryByRole('heading', { name: 'Waiting for players' })).toBeNull()
  })

  it('acts through the session, with no token, for a signed-in visitor without a seat link', async () => {
    const { h, question } = await ask({ account: 'ana' })
    await h.user.click(yes(question))
    await waitFor(() => expect(h.router.state.location.pathname).toBe('/'))
    expect(changesOf(h.server).map((sent) => [sent.method, sent.path, sent.authorization])).toEqual([
      [kase.method, kase.path, null],
    ])
  })

  it('makes everything wait while the request is out, and sends it once', async () => {
    const gate = held()
    const { h, question } = await ask({ change: gate.answer })
    await h.user.click(yes(question))
    expect(yes(question)).toHaveAttribute('aria-busy', 'true')
    expect(no(question)).toBeDisabled()
    // The page behind the dialog waits as well.
    expect(screen.getByRole('button', { name: 'Ready', hidden: true })).toBeDisabled()
    expect(document.querySelector('[data-slot="close-game"], [data-slot="leave-game"]')).toBeDisabled()
    await h.user.click(yes(question))
    await h.user.keyboard('{Escape}')
    expect(dialog()).not.toBeNull()
    expect(changesOf(h.server)).toHaveLength(1)
    gate.release(noContent())
    await waitFor(() => expect(h.router.state.location.pathname).toBe('/'))
  })

  it.each(kase.refused)('keeps the question up with the reason after %s', async (_what, answer, words) => {
    const { h, question } = await ask({ change: answer })
    await h.user.click(yes(question))
    await waitFor(() => expect(within(question).getByRole('alert')).toHaveTextContent(words))
    expect(dialog()).not.toBeNull()
    expect(yes(question)).not.toHaveAttribute('aria-busy')
    expect(yes(question)).toBeEnabled()
    expect(no(question)).toBeEnabled()
    expect(h.router.state.location.pathname).toBe(`/game/${GAME_ID}`)
    expect(screen.queryByText(kase.notice)).toBeNull()
  })

  it('tries again from the same question, and the reason goes while it does', async () => {
    let answer: () => Response | Promise<Response> = FAILURES.server
    const { h, question } = await ask({ change: () => answer() })
    await h.user.click(yes(question))
    await waitFor(() => expect(within(question).getByRole('alert')).toBeInTheDocument())
    answer = noContent
    await h.user.click(yes(question))
    await waitFor(() => expect(h.router.state.location.pathname).toBe('/'))
    expect(changesOf(h.server)).toHaveLength(2)
  })

  it('closes the question and takes the controls away when the game has started meanwhile', async () => {
    const { h, question } = await ask({ change: FAILURES.started })
    await h.user.click(yes(question))
    await waitFor(() => expect(dialog()).toBeNull())
    expect(stopped()).toHaveTextContent(kase.started)
    await waitFor(() => expect(stopped()).toHaveFocus())
    expect(screen.queryByRole('button', { name: kase.button })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Remove Bo from the game' })).toBeNull()
    expect(h.router.state.location.pathname).toBe(`/game/${GAME_ID}`)
  })

  it('has no button once a look says the game has started', async () => {
    await openLobby({ ...kase.scene, started: true })
    expect(screen.queryByRole('button', { name: kase.button })).toBeNull()
    expect(stopped()).toHaveTextContent(
      'This game has already started, so the lobby can no longer be changed.',
    )
  })
})

describe('who gets which button', () => {
  it('gives the host Close game and no Leave game', async () => {
    await openLobby()
    expect(screen.getByRole('button', { name: 'Close game' })).toBeEnabled()
    expect(screen.queryByRole('button', { name: 'Leave game' })).toBeNull()
  })

  it('gives a guest Leave game and no Close game, no Remove and no toggle', async () => {
    await openLobby({ seat: 1 })
    expect(screen.getByRole('button', { name: 'Leave game' })).toBeEnabled()
    expect(screen.queryByRole('button', { name: 'Close game' })).toBeNull()
    expect(screen.queryByRole('button', { name: /^Remove/ })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Invite only' })).toBeNull()
  })

  it.each([
    ['the host', 0],
    ['a guest', 1],
  ])('gives every button of %s a 32 px target, and 44 px under a coarse pointer', async (_who, seat) => {
    await openLobby({ seat, names: ['Ana', 'Bo', 'Cy', ''] })
    const sizes = (buttons: HTMLElement[]): string[] =>
      buttons.map((button) => {
        const tall = /\bh-(8|9|11)\b/.exec(button.className)?.[1]
        const coarse = /\bpointer-coarse:h-11\b/.test(button.className) || tall === '11'
        return `${button.textContent}:${tall === undefined ? 'none' : 'ok'}:${coarse ? 'coarse' : 'small'}`
      })
    for (const entry of sizes(screen.getAllByRole('button'))) expect(entry).toMatch(/:ok:coarse$/)
  })

  it('gives the buttons of the question the same targets', async () => {
    const h = await openLobby()
    await h.user.click(screen.getByRole('button', { name: 'Close game' }))
    const question = await screen.findByRole('alertdialog', { name: 'Close this game?' })
    for (const button of within(question).getAllByRole('button')) {
      expect(button.className).toMatch(/\bh-9\b/)
      expect(button.className).toMatch(/pointer-coarse:h-11/)
    }
  })

  it('puts Leave game after Ready in the reading order', async () => {
    await openLobby({ seat: 1 })
    expect(screen.getAllByRole('button').map((button) => button.textContent)).toEqual(['Ready', 'Leave game'])
  })

  it('shows the same result in a lobby whose server answers 200 with a body to a close', async () => {
    // A server that answers anything but 204 is not the server: the page says it could not read the answer.
    const h = await openLobby({ change: () => jsonResponse(200, {}) })
    await h.user.click(screen.getByRole('button', { name: 'Close game' }))
    const question = await screen.findByRole('alertdialog', { name: 'Close this game?' })
    await h.user.click(within(question).getByRole('button', { name: 'Close game' }))
    await waitFor(() => expect(within(question).getByRole('alert')).toHaveTextContent('could not read'))
    expect(h.router.state.location.pathname).toBe(`/game/${GAME_ID}`)
  })
})
