/** A harness for the decks page: the page over a deck server in memory, signed in as Ada, on a fixed clock. */
import { screen, waitFor, within } from '@testing-library/react'
import { expect } from 'vitest'
import userEvent from '@testing-library/user-event'
import { AuthProvider } from '@/features/auth/AuthProvider'
import { createAuthStore } from '@/features/auth/store'
import { startAccounts } from '@/test-utils/accountsServer'
import { renderInRouter } from '@/test-utils/router'
import { DecksPage } from './DecksPage'
import { startDecks } from './fakeDecks'
import type { DecksScript } from './fakeDecks'

/** The moment the page counts ages to: 18 October 2026, noon UTC. */
export const NOW = Date.parse('2026-10-18T12:00:00Z')

/** The page signed in as Ada over `script`'s server; resolves once the page has drawn its heading. */
export async function renderDecks(script: DecksScript = {}) {
  const server = startDecks(script)
  const accounts = startAccounts({ users: { Ada_L: 'correct horse battery' }, signedIn: 'Ada_L' })
  const store = createAuthStore({ api: accounts.api })
  const view = await renderInRouter(
    <AuthProvider store={store}>
      <DecksPage api={server.api} clock={() => NOW} />
    </AuthProvider>,
  )
  await screen.findByRole('heading', { level: 1, name: 'Decks' })
  return { ...view, server, store, user: userEvent.setup() }
}

/** The list of decks, once it is drawn; found also while a modal dialog hides the page behind it. */
export const deckList = (): HTMLElement => screen.getByRole('list', { name: 'Your decks', hidden: true })

/** The rows of the list, top to bottom. */
export const deckRows = (): HTMLElement[] => within(deckList()).getAllByRole('listitem', { hidden: true })

/** Waits until the first answer of the list has been drawn, whatever it was. */
export const listed = (): Promise<void> =>
  waitFor(() => expect(screen.queryByText('Loading your decks…')).toBeNull())

/** The row of the deck called `name`. */
export function rowOf(name: string): HTMLElement {
  const row = deckRows().find((item) => within(item).queryByText(name) !== null)
  if (row === undefined) throw new Error(`no row for ${name}`)
  return row
}

/** Every request the server saw, as "METHOD path". */
export const requests = (calls: ReadonlyArray<{ url: string; init?: RequestInit }>): string[] =>
  calls.map((call) => `${call.init?.method ?? 'GET'} ${call.url}`)
