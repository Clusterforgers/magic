// Goal: the lobby tells everybody who may find the game, and hands the host the invite link and the code kept in this tab, each with a copy button that confirms in words; it says plainly when the tab does not have the code, never shows a guest the code, and keeps working when the browser stores nothing.
import { screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { saveInvites } from '@/lib/invites'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { MADE_CODE, tokenOf } from '@/test-utils/fakeServer'
import { GAME_ID, renderGame } from '@/test-utils/gameHarness'
import type { Visibility } from '@/protocol/generated'
import { LOBBY_POLL_MS } from './useGameInfo'

afterEach(() => {
  vi.restoreAllMocks()
  window.sessionStorage.clear()
})

const origin = window.location.origin
const LINK = `${origin}/join/${MADE_CODE}`

/** The lookup of a two-seat game whose second seat is open (an empty name), as a created game looks. */
const info = (visibility: Visibility, names = ['Ana', '']) => ({
  started: false,
  seats: names.map((name, seat) => ({ seat, name })),
  connected: [0],
  ready: [],
  result: null,
  visibility,
})

const remember = (code: string | undefined = MADE_CODE) =>
  saveInvites({
    game: GAME_ID,
    ...(code === undefined ? {} : { code }),
    seats: [{ seat: 0, name: 'Ana', token: tokenOf(0) }],
  })

async function hostLobby(visibility: Visibility = 'public') {
  const h = await renderGame({ script: { game: () => jsonResponse(200, info(visibility)) } })
  h.join()
  await h.settle()
  return h
}

const invite = () => document.querySelector('[data-slot="join-invite"]') as HTMLElement | null
const visibilityLine = () => document.querySelector('[data-slot="lobby-visibility"]')

describe('who may find the game', () => {
  it.each([
    ['public', 'Listed in the game browser'],
    ['invite', 'Invite only'],
  ] as const)('says a %s game is: %s', async (visibility, words) => {
    await hostLobby(visibility)
    expect(visibilityLine()).toHaveTextContent(words)
    expect(visibilityLine()).toHaveAttribute('data-visibility', visibility)
  })

  it('says nothing about it until the server has said', async () => {
    const h = await renderGame()
    h.join()
    await h.settle()
    expect(visibilityLine()).toBeNull()
  })

  it('is told the same to a guest', async () => {
    const h = await renderGame({
      link: { seat: 1, token: tokenOf(1) },
      script: { game: () => jsonResponse(200, info('public', ['Ana', 'Bo'])) },
    })
    h.join(1)
    await h.settle()
    expect(visibilityLine()).toHaveTextContent('Listed in the game browser')
  })

  it('follows a change the host made, at the next look', async () => {
    let answer: Visibility = 'public'
    const h = await renderGame({ script: { game: () => jsonResponse(200, info(answer)) } })
    h.join()
    await h.settle()
    expect(visibilityLine()).toHaveTextContent('Listed in the game browser')
    answer = 'invite'
    await h.advance(LOBBY_POLL_MS)
    await h.settle()
    expect(visibilityLine()).toHaveTextContent('Invite only')
  })

  it('keeps it, and the invite, across a reload of the page', async () => {
    remember()
    const first = await hostLobby('invite')
    expect(visibilityLine()).toHaveTextContent('Invite only')
    first.unmount()
    await hostLobby('invite')
    expect(visibilityLine()).toHaveTextContent('Invite only')
    expect(within(invite() as HTMLElement).getByRole('link')).toHaveTextContent(LINK)
  })
})

describe('the invite of the host', () => {
  it('shows the link and the code this tab kept', async () => {
    remember()
    await hostLobby()
    const section = within(invite() as HTMLElement)
    expect(section.getByRole('heading', { name: 'Invite players' })).toBeInTheDocument()
    expect(section.getByRole('link')).toHaveAttribute('href', LINK)
    expect(section.getByRole('link')).toHaveTextContent(LINK)
    expect(document.querySelector('[data-slot="invite-code"]')).toHaveTextContent('K7M2-9QXA')
  })

  it('copies the link and says so in words', async () => {
    remember()
    const h = await hostLobby()
    const copy = screen.getByRole('button', { name: 'Copy invite link' })
    expect(copy).toHaveTextContent('Copy')
    await h.user.click(copy)
    expect(await navigator.clipboard.readText()).toBe(LINK)
    expect(copy).toHaveTextContent('Copied')
    expect(within(invite() as HTMLElement).getAllByRole('status')[0]).toHaveTextContent(
      'Copied to the clipboard.',
    )
  })

  it('copies the code the way it is shown, and says so in words', async () => {
    remember()
    const h = await hostLobby()
    const copy = screen.getByRole('button', { name: 'Copy code' })
    await h.user.click(copy)
    expect(await navigator.clipboard.readText()).toBe('K7M2-9QXA')
    expect(copy).toHaveTextContent('Copied')
    expect(within(invite() as HTMLElement).getAllByRole('status')[1]).toHaveTextContent(
      'Copied to the clipboard.',
    )
  })

  it('says it was refused, and to copy by hand, when the browser will not copy', async () => {
    remember()
    const h = await hostLobby()
    // jsdom has no `execCommand`, so the page's fallback fails too.
    vi.spyOn(navigator.clipboard, 'writeText').mockRejectedValue(
      new DOMException('denied', 'NotAllowedError'),
    )
    await h.user.click(screen.getByRole('button', { name: 'Copy code' }))
    expect(screen.getByRole('button', { name: 'Copy code' })).toHaveTextContent('Copy failed')
  })

  it('says plainly that this tab does not have the code when it was never kept', async () => {
    await hostLobby()
    expect(invite()).toHaveTextContent('The code is shown only in the browser tab that made the game')
    expect(within(invite() as HTMLElement).queryByRole('link')).toBeNull()
    expect(screen.queryByRole('button', { name: 'Copy code' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Copy invite link' })).toBeNull()
  })

  it('says the same, and keeps the lobby working, when the browser will not let the page read its storage', async () => {
    remember()
    vi.spyOn(window, 'sessionStorage', 'get').mockImplementation(() => {
      throw new DOMException('blocked', 'SecurityError')
    })
    await hostLobby()
    expect(invite()).toHaveTextContent('does not have it')
    expect(screen.getByRole('button', { name: 'Ready' })).toBeEnabled()
    expect(screen.getByRole('list', { name: 'Seats' })).toBeInTheDocument()
  })

  it('has no section for a game with a link for every seat, whose links are listed instead', async () => {
    saveInvites({
      game: GAME_ID,
      seats: [
        { seat: 0, name: 'Ana', token: tokenOf(0) },
        { seat: 1, name: 'Bo', token: tokenOf(1) },
      ],
    })
    await hostLobby()
    expect(invite()).toBeNull()
    expect(screen.getByRole('list', { name: 'Seat links' })).toBeInTheDocument()
  })
})

describe('a guest', () => {
  it('sees the seats and the visibility but never the code, even in the tab that holds it', async () => {
    remember()
    const h = await renderGame({
      link: { seat: 1, token: tokenOf(1) },
      script: { game: () => jsonResponse(200, info('invite', ['Ana', 'Bo'])) },
    })
    h.join(1)
    await h.settle()
    expect(invite()).toBeNull()
    expect(document.body).not.toHaveTextContent('K7M2-9QXA')
    expect(document.body).not.toHaveTextContent(MADE_CODE)
    expect(screen.queryByRole('link', { name: LINK })).toBeNull()
    expect(visibilityLine()).toHaveTextContent('Invite only')
    expect(within(screen.getByRole('list', { name: 'Seats' })).getAllByRole('listitem')).toHaveLength(2)
  })
})

describe('the seats of a game with an open seat', () => {
  it('shows the open seat as a seat nobody has joined yet, not as a blank name', async () => {
    await hostLobby()
    const rows = within(screen.getByRole('list', { name: 'Seats' })).getAllByRole('listitem')
    expect(rows[0]).toHaveTextContent('Seat 1Anayou')
    expect(rows[1]).toHaveTextContent('Seat 2Seat 2')
    expect(rows[1]).toHaveTextContent('not joined yet')
    expect(rows[1]).toHaveTextContent('Not here')
  })
})
