// Goal: deleting a deck asks first and names the deck, sends nothing on Keep deck or Escape, sends exactly one DELETE on confirm and then lists the rest without it, keeps the question up with the reason when the server refuses, ignores a second press while the first is in flight, and treats a deck that is already gone as deleted.
import { screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { deckError, deckItem } from './fakeDecks'
import { deckRows, renderDecks, requests } from './renderDecks'

const dialog = () => screen.queryByRole('alertdialog')
const TWO = [deckItem({ id: 'deck-a', name: 'Atraxa' }), deckItem({ id: 'deck-b', name: 'Burn' })]

/** The page with two decks, and the question for the first one open. */
async function ask(script: Parameters<typeof renderDecks>[0] = { decks: TWO }) {
  const app = await renderDecks(script)
  await screen.findByRole('list', { name: 'Your decks' })
  await app.user.click(screen.getByRole('button', { name: 'Delete Atraxa' }))
  const question = await screen.findByRole('alertdialog', { name: 'Delete “Atraxa”?' })
  return { ...app, question }
}

describe('asking first', () => {
  it('opens a question that names the deck and sends nothing yet', async () => {
    const { question, server } = await ask()
    expect(within(question).getByText(/cannot be brought back/)).toBeInTheDocument()
    expect(requests(server.calls)).toEqual(['GET /api/decks'])
  })

  it('moves the focus to the safe choice', async () => {
    const { question } = await ask()
    await waitFor(() => expect(within(question).getByRole('button', { name: 'Keep deck' })).toHaveFocus())
  })

  it('sends nothing when the deck is kept, and gives the focus back to its button', async () => {
    const { user, question, server } = await ask()
    await user.click(within(question).getByRole('button', { name: 'Keep deck' }))
    await waitFor(() => expect(dialog()).toBeNull())
    expect(requests(server.calls)).toEqual(['GET /api/decks'])
    expect(deckRows()).toHaveLength(2)
    await waitFor(() => expect(screen.getByRole('button', { name: 'Delete Atraxa' })).toHaveFocus())
  })

  it('sends nothing on Escape', async () => {
    const { user, server } = await ask()
    await user.keyboard('{Escape}')
    await waitFor(() => expect(dialog()).toBeNull())
    expect(requests(server.calls)).toEqual(['GET /api/decks'])
  })

  it('asks again, about the right deck, when another one is chosen after a cancel', async () => {
    const { user, question } = await ask()
    await user.click(within(question).getByRole('button', { name: 'Keep deck' }))
    await waitFor(() => expect(dialog()).toBeNull())
    await user.click(screen.getByRole('button', { name: 'Delete Burn' }))
    expect(await screen.findByRole('alertdialog', { name: 'Delete “Burn”?' })).toBeInTheDocument()
  })
})

describe('confirming', () => {
  it('sends one DELETE for that deck, closes, and lists the others', async () => {
    const { user, question, server } = await ask()
    await user.click(within(question).getByRole('button', { name: 'Delete deck' }))
    await waitFor(() => expect(dialog()).toBeNull())
    await waitFor(() => expect(deckRows()).toHaveLength(1))
    expect(deckRows()[0]).toHaveTextContent('Burn')
    expect(requests(server.calls)).toEqual(['GET /api/decks', 'DELETE /api/decks/deck-a', 'GET /api/decks'])
    expect(server.removed).toEqual(['deck-a'])
    expect(server.calls[1]?.init?.credentials).toBe('include')
  })

  it('says so aloud and moves the focus to the heading, since the row that had it is gone', async () => {
    const { user, question } = await ask()
    await user.click(within(question).getByRole('button', { name: 'Delete deck' }))
    expect(await screen.findByText('Deleted “Atraxa”.')).toBeInTheDocument()
    await waitFor(() => expect(screen.getByRole('heading', { level: 1, name: 'Decks' })).toHaveFocus())
  })

  it('shows the empty state after the last deck is deleted', async () => {
    const { user, question } = await ask({ decks: [deckItem({ id: 'deck-a', name: 'Atraxa' })] })
    await user.click(within(question).getByRole('button', { name: 'Delete deck' }))
    expect(await screen.findByRole('heading', { name: 'You have no decks yet' })).toBeInTheDocument()
  })

  it('sends one request when the button is pressed twice while the first is in flight', async () => {
    const release: { current: (() => void) | null } = { current: null }
    const { user, question, server } = await ask({
      decks: TWO,
      override: {
        remove: () =>
          new Promise<Response>((resolve) => {
            release.current = () => resolve(new Response(null, { status: 204 }))
          }),
      },
    })
    const confirm = within(question).getByRole('button', { name: 'Delete deck' })
    await user.click(confirm)
    await user.click(confirm)
    expect(requests(server.calls).filter((call) => call.startsWith('DELETE'))).toHaveLength(1)
    expect(confirm).toHaveAttribute('aria-busy', 'true')
    release.current?.()
    await waitFor(() => expect(dialog()).toBeNull())
  })
})

describe('a delete the server refuses', () => {
  it.each([
    ['a server error', 500, 'internal', /server ran into a problem/],
    ['an ended session', 401, 'unauthenticated', /session has ended/],
    ['a server that lost its database', 503, 'decks_unavailable', /keeps no decks/],
  ] as const)(
    'keeps the question up with the reason for %s, and the deck in the list',
    async (_what, status, code, words) => {
      const { user, question } = await ask({
        decks: TWO,
        override: { remove: () => deckError(status, code, 'no') },
      })
      await user.click(within(question).getByRole('button', { name: 'Delete deck' }))
      expect(await within(question).findByRole('alert')).toHaveTextContent(words)
      expect(dialog()).toBeInTheDocument()
      expect(deckRows()).toHaveLength(2)
      expect(within(question).getByRole('button', { name: 'Delete deck' })).not.toHaveAttribute('aria-busy')
    },
  )

  it('lets the user try again', async () => {
    let attempts = 0
    const { user, question } = await ask({
      decks: TWO,
      override: {
        remove: () => {
          attempts += 1
          return attempts === 1 ? deckError(500, 'internal', 'boom') : undefined
        },
      },
    })
    await user.click(within(question).getByRole('button', { name: 'Delete deck' }))
    await within(question).findByRole('alert')
    await user.click(within(question).getByRole('button', { name: 'Delete deck' }))
    await waitFor(() => expect(dialog()).toBeNull())
    await waitFor(() => expect(deckRows()).toHaveLength(1))
  })

  it('counts a deck that is already gone as deleted and shows the list as it is now', async () => {
    const { user, question, server } = await ask()
    await server.api.remove('deck-a')
    await user.click(within(question).getByRole('button', { name: 'Delete deck' }))
    await waitFor(() => expect(dialog()).toBeNull())
    await waitFor(() => expect(deckRows()).toHaveLength(1))
    expect(deckRows()[0]).toHaveTextContent('Burn')
  })
})
