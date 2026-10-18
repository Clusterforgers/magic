// Goal: the real route tree serves the create page at / and the game page at /game/$gameId, reading the seat and token from the address fragment, opening one socket per seat link and starting over when the link changes.
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { act, render, screen } from '@testing-library/react'
import { StrictMode } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { startServer, tokenOf } from '@/test-utils/fakeServer'
import { routeTree } from './routeTree.gen'

/** The part of a browser WebSocket the client uses, recording every socket the page opens. */
class RecordingSocket {
  static opened: RecordingSocket[] = []
  readonly url: string
  closed: { code: number; reason: string } | null = null
  onopen: (() => void) | null = null
  onmessage: ((event: { data: unknown }) => void) | null = null
  onclose: ((event: { code: number; reason: string }) => void) | null = null

  constructor(url: string) {
    this.url = url
    RecordingSocket.opened.push(this)
  }

  send(): void {}

  close(code: number, reason: string): void {
    this.closed = { code, reason }
  }
}

beforeEach(() => {
  RecordingSocket.opened = []
  vi.stubGlobal('WebSocket', RecordingSocket)
  vi.stubGlobal('fetch', startServer().fetch)
})

afterEach(() => {
  vi.unstubAllGlobals()
})

async function visit(url: string, strict = false) {
  const router = createRouter({ routeTree, history: createMemoryHistory({ initialEntries: [url] }) })
  const tree = <RouterProvider router={router} />
  render(strict ? <StrictMode>{tree}</StrictMode> : tree)
  await router.load()
  await act(async () => {
    await Promise.resolve()
  })
  return router
}

const GAME = '8f2c1a52-31de-4c77-9a35-0d1d3f6a7b10'

describe('/', () => {
  it('is the create-game page inside the app shell', async () => {
    await visit('/')
    expect(await screen.findByRole('form', { name: 'Create a game' })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Magic' })).toHaveAttribute('href', '/')
    expect(RecordingSocket.opened).toHaveLength(0)
  })
})

describe('/game/$gameId', () => {
  it('asks for the seat link when the address has no fragment, and opens no socket', async () => {
    await visit(`/game/${GAME}`)
    expect(await screen.findByRole('heading', { name: 'Open your seat link' })).toBeInTheDocument()
    expect(RecordingSocket.opened).toHaveLength(0)
  })

  it.each([
    ['only a seat', '#seat=1'],
    ['only a token', `#token=${tokenOf(1)}`],
    ['a seat that is not a number', `#seat=x&token=${tokenOf(1)}`],
    ['a seat past the last', `#seat=12&token=${tokenOf(1)}`],
    ['an empty token', '#seat=1&token='],
  ])('asks for the seat link when the fragment has %s', async (_what, fragment) => {
    await visit(`/game/${GAME}${fragment}`)
    expect(await screen.findByRole('heading', { name: 'Open your seat link' })).toBeInTheDocument()
    expect(RecordingSocket.opened).toHaveLength(0)
  })

  it('connects at the seat address with the token from the fragment, and shows it nowhere', async () => {
    await visit(`/game/${GAME}#seat=1&token=${tokenOf(1)}`)
    expect(await screen.findByRole('heading', { name: 'Connecting to the game…' })).toBeInTheDocument()
    expect(RecordingSocket.opened.map((socket) => socket.url)).toEqual([
      `ws://localhost:3000/ws/${GAME}/${tokenOf(1)}`,
    ])
    expect(document.body.innerHTML).not.toContain(tokenOf(1))
  })

  it('opens one socket when React mounts everything twice', async () => {
    await visit(`/game/${GAME}#seat=1&token=${tokenOf(1)}`, true)
    await screen.findByRole('heading', { name: 'Connecting to the game…' })
    expect(RecordingSocket.opened).toHaveLength(1)
  })

  it('reads a token that needed escaping exactly as it was written', async () => {
    await visit(`/game/${GAME}#seat=0&token=${encodeURIComponent('a+b/c=d')}`)
    await screen.findByRole('heading', { name: 'Connecting to the game…' })
    expect(
      RecordingSocket.opened.map((socket) =>
        decodeURIComponent(new URL(socket.url).pathname.split('/').at(-1) ?? ''),
      ),
    ).toEqual(['a+b/c=d'])
  })

  it('closes the first socket and opens another when the same page gets a different seat link', async () => {
    const router = await visit(`/game/${GAME}#seat=0&token=${tokenOf(0)}`)
    await screen.findByRole('heading', { name: 'Connecting to the game…' })
    await act(async () => {
      await router.navigate({
        to: '/game/$gameId',
        params: { gameId: GAME },
        hash: `seat=1&token=${tokenOf(1)}`,
      })
    })
    await act(async () => {
      await Promise.resolve()
    })
    expect(RecordingSocket.opened.map((socket) => socket.url)).toEqual([
      `ws://localhost:3000/ws/${GAME}/${tokenOf(0)}`,
      `ws://localhost:3000/ws/${GAME}/${tokenOf(1)}`,
    ])
    expect(RecordingSocket.opened[0]?.closed).toEqual({ code: 1000, reason: 'client closing' })
    expect(RecordingSocket.opened[1]?.closed).toBeNull()
  })

  it('closes its socket when the visitor leaves for the create page', async () => {
    const router = await visit(`/game/${GAME}#seat=0&token=${tokenOf(0)}`)
    await screen.findByRole('heading', { name: 'Connecting to the game…' })
    await act(async () => {
      await router.navigate({ to: '/' })
    })
    expect(RecordingSocket.opened).toHaveLength(1)
    expect(RecordingSocket.opened[0]?.closed).toEqual({ code: 1000, reason: 'client closing' })
    expect(await screen.findByRole('form', { name: 'Create a game' })).toBeInTheDocument()
  })
})
