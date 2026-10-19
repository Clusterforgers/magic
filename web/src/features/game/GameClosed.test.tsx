// Goal: when the host closes the game the socket stops for good (close code 4002) and the page says "The host closed this game." with a way back to Play, while a removed seat (4001) still hears that it was removed and a plain drop (1001) still reconnects.
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { renderGame } from '@/test-utils/gameHarness'

describe('a game the host closed', () => {
  it('says so as the whole page, with a way back to Play and to a new game', async () => {
    const h = await renderGame()
    h.join()
    expect(screen.getByRole('heading', { name: 'Waiting for players' })).toBeInTheDocument()
    h.close(4002)
    expect(screen.getByRole('heading', { level: 1, name: 'The host closed this game.' })).toBeInTheDocument()
    expect(screen.getByRole('alert')).toHaveTextContent('Nobody can join it any more')
    expect(screen.getByRole('link', { name: 'Back to Play' })).toHaveAttribute('href', '/')
    expect(screen.getByRole('link', { name: 'Create a game' })).toHaveAttribute('href', '/new')
    expect(screen.queryByRole('heading', { name: 'Waiting for players' })).toBeNull()
    expect(document.title).toMatch(/Game closed/)
  })

  it('offers no way to connect again, since the game is gone', async () => {
    const h = await renderGame()
    h.join()
    h.close(4002)
    expect(screen.queryByRole('button', { name: /connect again|try again|use this window/i })).toBeNull()
    expect(screen.queryByText(/reconnecting/i)).toBeNull()
  })

  it('opens no second socket however long it waits', async () => {
    const h = await renderGame()
    h.join()
    h.close(4002)
    await h.advance(120_000)
    expect(h.net.sockets).toHaveLength(1)
  })

  it('stops a socket that was not yet welcomed in the same way', async () => {
    const h = await renderGame()
    h.close(4002)
    expect(screen.getByRole('heading', { name: 'The host closed this game.' })).toBeInTheDocument()
    await h.advance(120_000)
    expect(h.net.sockets).toHaveLength(1)
  })

  it('does not call the close a lost connection', async () => {
    const h = await renderGame()
    h.join()
    h.close(4002)
    expect(screen.queryByText(/connection lost/i)).toBeNull()
    expect(screen.queryByText(/cannot reach the game/i)).toBeNull()
  })
})

describe('the other ways a seat loses its game', () => {
  it('still tells a removed seat that it was removed, with a way to connect again', async () => {
    const h = await renderGame()
    h.join()
    h.close(4001)
    expect(screen.getByRole('heading', { name: 'The connection ended' })).toBeInTheDocument()
    expect(screen.getByRole('alert')).toHaveTextContent('You were removed from this game.')
    expect(screen.getByRole('button', { name: 'Connect again' })).toBeInTheDocument()
    expect(screen.queryByText('The host closed this game.')).toBeNull()
  })

  it('still reconnects after a plain drop of the game (1001), which is not the host’s close', async () => {
    const h = await renderGame()
    h.join()
    h.close(1001)
    expect(screen.getByText(/connection lost/i)).toBeInTheDocument()
    await h.advance(250)
    expect(h.net.sockets).toHaveLength(2)
    expect(screen.queryByText('The host closed this game.')).toBeNull()
  })
})
