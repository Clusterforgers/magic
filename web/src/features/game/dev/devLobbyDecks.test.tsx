// Goal: /dev/lobby gives every seat a deck unless `nodeck` lists it, reads `nodeck` and `account` leniently, lists the stored decks of a signed-in page beside the built-in ones, and answers a choice as a server would: the roster shows the deck, a deck of Commander is refused with its issues, and Ready opens once the seat has a deck.
import { createMemoryHistory, createRouter, RouterProvider } from '@tanstack/react-router'
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { scaled } from '@/test-utils/loadFactor'
import { clearFlash } from '@/lib/flash'
import { fixtureRoster } from './lobbyGame'
import { parseDevLobbySearch } from './lobbySearch'

async function visit(url: string) {
  vi.resetModules()
  const { routeTree } = await import('@/routeTree.gen')
  const router = createRouter({ routeTree, history: createMemoryHistory({ initialEntries: [url] }) })
  render(<RouterProvider router={router} />)
  await act(async () => {
    await router.load()
  })
}

afterEach(() => {
  cleanup()
  clearFlash()
})

const search = (over: Record<string, unknown>) => parseDevLobbySearch({ seats: 4, ...over })

describe('the address', () => {
  it.each<[Record<string, unknown>, number[] | undefined]>([
    [{}, undefined],
    [{ nodeck: '1,3' }, [1, 3]],
    [{ nodeck: [2, 0] }, [0, 2]],
    [{ nodeck: 9 }, undefined],
    [{ nodeck: 'x' }, undefined],
  ])('reads nodeck %j as %j', (given, expected) => {
    expect(search(given).nodeck).toEqual(expected)
  })

  it.each<[unknown, true | undefined]>([
    [1, true],
    ['1', true],
    [true, true],
    [0, undefined],
    ['yes', undefined],
    [undefined, undefined],
  ])('reads account %j as %j', (given, expected) => {
    expect(search({ account: given }).account).toBe(expected)
  })

  it('gives every seat a deck but those in nodeck, and never a card name', () => {
    const roster = fixtureRoster(search({ nodeck: [1] }))
    expect(roster.seats.map((seat) => seat.deck === undefined)).toEqual([false, true, false, false])
    for (const seat of roster.seats)
      expect(Object.keys(seat.deck ?? {})).toEqual(
        seat.deck === undefined ? [] : ['name', 'format', 'manual_cards'],
      )
  })
})

describe('/dev/lobby with decks', () => {
  it('lists the stored decks of a signed-in page with the built-in ones, and keeps a guest to the built-in ones', async () => {
    await visit('/dev/lobby?seats=3&me=1&nodeck=1&account=1')
    expect(
      await screen.findByRole('list', { name: 'Your decks' }, { timeout: scaled(5000) }),
    ).toBeInTheDocument()
    expect(screen.getByRole('list', { name: 'Demo decks' })).toBeInTheDocument()
    cleanup()
    await visit('/dev/lobby?seats=3&me=1&nodeck=1')
    expect(await screen.findByRole('list', { name: 'Demo decks' })).toBeInTheDocument()
    expect(screen.queryByRole('list', { name: 'Your decks' })).toBeNull()
  }, 20_000)

  it('shows the choice in the roster and opens Ready, and refuses a Commander deck with its issues', async () => {
    const user = userEvent.setup()
    await visit('/dev/lobby?seats=3&me=1&nodeck=1&account=1')
    const ready = await screen.findByRole('button', { name: 'Ready' })
    expect(ready).toBeDisabled()
    await user.click(await screen.findByRole('button', { name: /Atraxa Superfriends/ }))
    const alert = await screen.findByRole('alert', {}, { timeout: scaled(5000) })
    expect(within(alert).getAllByRole('listitem').length).toBeGreaterThan(0)
    await user.click(screen.getByRole('button', { name: /Burn/ }))
    await waitFor(() => expect(screen.getByRole('button', { name: 'Ready' })).toBeEnabled(), {
      timeout: scaled(5000),
    })
    expect(document.querySelectorAll('[data-slot="seat-deck"]')[1]).toHaveTextContent(
      'Burn, Modern, 3 cards need manual steps',
    )
  }, 20_000)
})
