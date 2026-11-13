// Goal: every deck picker shows the account's decks in its collapsible groups (Ungrouped last): the group of the chosen deck, else the last used group, else the first opens by default, a search opens every group with a match, the arrow keys move only through open groups, and choosing a deck remembers its group.
import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { AuthContext } from '@/features/auth/context'
import { createAuthStore, offlineAccounts } from '@/features/auth/store'
import { groupView, startGroups } from '@/features/decks/groups/fakeGroups'
import { renderInRouter } from '@/test-utils/router'
import { lobbyDecks, lobbyPrecons } from './dev/deckApis'
import { DeckChooser } from './DeckChooser'
import type { DeckChoice } from './useDeckChoice'

const KEY = 'mtg.deckPicker.lastGroup'

afterEach(() => {
  vi.unstubAllGlobals()
  localStorage.clear()
})

const GROUPS = [groupView('g1', 'Fun', ['stored-pauper']), groupView('g2', 'Hardcore', ['stored-rakdos'])]

async function open(chosenId: string | null = null) {
  vi.stubGlobal('matchMedia', (query: string) => ({
    matches: true,
    media: query,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }))
  const choose = vi.fn()
  const choice: DeckChoice = { pending: null, chosenId, note: null, problem: null, choose }
  const auth = createAuthStore({
    api: { ...offlineAccounts, myGames: async () => [] },
    initial: { status: 'signed-in', user: { id: 'u', username: 'Ada_L', is_admin: false } },
  })
  const groups = startGroups(GROUPS)
  await renderInRouter(
    <AuthContext value={auth}>
      <DeckChooser
        api={{ listDecks: async () => [] }}
        decks={lobbyDecks({})}
        precons={lobbyPrecons({})}
        favouritesApi={{ list: async () => [], star: async () => undefined, unstar: async () => undefined }}
        deckGroupsApi={groups.api}
        game="g1"
        format="normal"
        signedIn
        current={null}
        ready={false}
        busy={false}
        choice={choice}
      />
    </AuthContext>,
  )
  await screen.findByRole('button', { name: /^Hardcore/ })
  return { choose, user: userEvent.setup() }
}

const header = (name: RegExp) => screen.getByRole('button', { name })
const shownDecks = (): string[] =>
  screen
    .queryAllByRole('option')
    .filter((row) => row.hasAttribute('data-deck-option'))
    .map((row) => row.getAttribute('aria-label') ?? row.textContent ?? '')
    .map((text) => text.split(/[,·]/)[0]?.trim() ?? '')

describe('the deck picker in groups', () => {
  it('lists the groups with Ungrouped last and opens the first by default', async () => {
    await open()
    expect(header(/^Fun/)).toHaveAttribute('aria-expanded', 'true')
    expect(header(/^Hardcore/)).toHaveAttribute('aria-expanded', 'false')
    expect(header(/^Ungrouped/)).toHaveAttribute('aria-expanded', 'false')
    expect(screen.getByRole('listbox', { name: 'Your decks: Fun' })).toBeInTheDocument()
    expect(screen.getByRole('option', { name: /^Pauper Affinity/ })).toBeInTheDocument()
    expect(screen.queryByRole('option', { name: /^Rakdos/ })).toBeNull()
  })

  it.each([
    ['the last used group', null, 'g2', /^Hardcore/],
    ['the group of the chosen deck, over the last used one', 'stored-pauper', 'g2', /^Fun/],
    ['the first group when the last used one is gone', null, 'gone', /^Fun/],
  ])('opens %s', async (_, chosen, last, opened) => {
    localStorage.setItem(KEY, last)
    await open(chosen)
    expect(header(opened)).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getAllByRole('button', { expanded: true })).toHaveLength(1)
  })

  it('opens every group with a match while searching', async () => {
    const { user } = await open()
    await user.type(screen.getByRole('searchbox', { name: 'Search your decks' }), 'r')
    await waitFor(() => expect(header(/^Hardcore/)).toHaveAttribute('aria-expanded', 'true'))
    expect(screen.getByRole('option', { name: /^Rakdos/ })).toBeInTheDocument()
    expect(screen.getByRole('option', { name: /^Burn/ })).toBeInTheDocument()
  })

  it('moves the arrow keys through open groups only and remembers the group of the deck chosen', async () => {
    const { user, choose } = await open()
    await user.click(header(/^Ungrouped/))
    const pauper = screen.getByRole('option', { name: /^Pauper Affinity/ })
    await waitFor(() => expect(pauper).toHaveAttribute('aria-selected', 'true'))
    pauper.focus()
    await user.keyboard('{ArrowDown}')
    expect(screen.getByRole('option', { name: /^Burn/ })).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(choose).toHaveBeenLastCalledWith({ id: 'stored-burn', name: 'Burn' })
    expect(localStorage.getItem(KEY)).toBeNull()
    await user.dblClick(screen.getByRole('option', { name: /^Pauper Affinity/ }))
    expect(localStorage.getItem(KEY)).toBe('g1')
  })

  it('folds a group with its heading button', async () => {
    const { user } = await open()
    await user.click(header(/^Fun/))
    expect(header(/^Fun/)).toHaveAttribute('aria-expanded', 'false')
    expect(shownDecks()).toEqual([])
  })
})
