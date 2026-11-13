// Goal: when an admin ends a game the page says "Ended by an admin" with the reason, as the whole page with a way back to Play, the socket stops for good (no second socket however long it waits), and a bare 4003 without the message still reads as ended by an admin, never as a lost connection.
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { renderGame } from '@/test-utils/gameHarness'

describe('a game an admin ended', () => {
  it('says so with the reason, as the whole page, with a way back to Play', async () => {
    const h = await renderGame()
    h.join()
    h.deliver({ type: 'EndedByAdmin', reason: 'the server restarts' })
    h.close(4003)
    expect(
      screen.getByRole('heading', { level: 1, name: 'Ended by an admin: the server restarts' }),
    ).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Back to Play' })).toHaveAttribute('href', '/')
    expect(screen.queryByText(/connection lost/i)).toBeNull()
    expect(document.title).toMatch(/Game closed/)
    await h.advance(120_000)
    expect(h.net.sockets).toHaveLength(1)
  })

  it('says so without a reason when the admin gave none', async () => {
    const h = await renderGame()
    h.join()
    h.deliver({ type: 'EndedByAdmin' })
    expect(screen.getByRole('heading', { level: 1, name: 'Ended by an admin.' })).toBeInTheDocument()
    await h.advance(120_000)
    expect(h.net.sockets).toHaveLength(1)
  })

  it('reads a bare 4003 the same way and does not reconnect', async () => {
    const h = await renderGame()
    h.join()
    h.close(4003)
    expect(screen.getByRole('heading', { level: 1, name: 'Ended by an admin.' })).toBeInTheDocument()
    expect(screen.queryByText('The host closed this game.')).toBeNull()
    await h.advance(120_000)
    expect(h.net.sockets).toHaveLength(1)
  })
})
