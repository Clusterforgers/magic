/** A harness for the builder: the page over a deck server and a card server in memory, signed in as Ada. */
import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { AuthProvider } from '@/features/auth/AuthProvider'
import { createAuthStore } from '@/features/auth/store'
import { deckView, lineOf, startDecks } from '@/features/decks/fakeDecks'
import type { DecksScript } from '@/features/decks/fakeDecks'
import type { CardHit, DeckView } from '@/protocol/generated'
import { startAccounts } from '@/test-utils/accountsServer'
import { renderInRouter } from '@/test-utils/router'
import { BuilderPage } from './BuilderPage'
import { cardHit, startCards } from './fakeCards'
import type { CardsScript } from './fakeCards'

export const BOLT = cardHit('Lightning Bolt', { oracle_id: 'oracle-bolt' })
export const CHAIN = cardHit('Chain Lightning', { type_line: 'Sorcery', oracle_id: 'oracle-chain' })
export const GROWTH = cardHit('Giant Growth', {
  mana_cost: '{G}',
  colors: ['G'],
  color_identity: ['G'],
  oracle_id: 'oracle-growth',
})
export const ELVES = cardHit('Llanowar Elves', {
  mana_cost: '{G}',
  type_line: 'Creature — Elf Druid',
  colors: ['G'],
  color_identity: ['G'],
  oracle_id: 'oracle-elves',
})
export const MOUNTAIN = cardHit('Mountain', {
  mana_cost: '',
  mana_value: 0,
  type_line: 'Basic Land — Mountain',
  colors: [],
  color_identity: ['R'],
  oracle_id: 'oracle-mountain',
})
export const DURESS = cardHit('Duress', {
  mana_cost: '{B}',
  type_line: 'Sorcery',
  colors: ['B'],
  color_identity: ['B'],
  oracle_id: 'oracle-duress',
})
export const SOL_RING = cardHit('Sol Ring', {
  mana_cost: '{1}',
  type_line: 'Artifact',
  colors: [],
  color_identity: [],
  oracle_id: 'oracle-sol-ring',
})
/** The cards the fake card server holds. */
export const POOL: CardHit[] = [BOLT, CHAIN, GROWTH, ELVES, MOUNTAIN, DURESS, SOL_RING]

/** What `renderBuilder` can be told. */
export interface BuilderScript {
  /** Changes to the stored deck: a modern deck called Burn with Mountain and Lightning Bolt in the main deck and Duress beside it. */
  view?: Partial<DeckView>
  cards?: CardsScript
  decks?: DecksScript
  /** False to return before the deck pane is drawn, for a deck that does not load. */
  wait?: boolean
}

/** The builder signed in as Ada over `script`'s servers; resolves once the deck pane is drawn. */
export async function renderBuilder(script: BuilderScript = {}) {
  const view = deckView({
    id: 'deck-1',
    name: 'Burn',
    format: 'modern',
    state: 'manual_play',
    verified: false,
    cards: [
      lineOf('Mountain', 20, { card: MOUNTAIN }),
      lineOf('Lightning Bolt', 4, { card: BOLT }),
      lineOf('Duress', 2, { zone: 'sideboard', card: DURESS }),
    ],
    ...script.view,
  })
  const decks = startDecks({ views: [view], pool: POOL, ...script.decks })
  const cards = startCards({ cards: POOL, ...script.cards })
  const accounts = startAccounts({ users: { Ada_L: 'correct horse battery' }, signedIn: 'Ada_L' })
  const store = createAuthStore({ api: accounts.api })
  const rendered = await renderInRouter(
    <AuthProvider store={store}>
      <BuilderPage deckId={view.id} api={decks.api} cards={cards.api} />
    </AuthProvider>,
  )
  if (script.wait !== false) await screen.findByRole('region', { name: 'Deck' })
  return { ...rendered, decks, cards, view, user: userEvent.setup() }
}

export const searchBox = (): HTMLElement => screen.getByRole('searchbox', { name: 'Search cards' })
export const searchPane = (): HTMLElement => screen.getByRole('region', { name: 'Card search' })
export const deckPane = (): HTMLElement => screen.getByRole('region', { name: 'Deck' })

/** The line of the card called `name` in the deck pane, in `zone` when given. */
export function deckLine(name: string, zone?: string): HTMLElement {
  const rows = [...deckPane().querySelectorAll<HTMLElement>('li[data-line]')].filter(
    (row) =>
      row.textContent?.includes(name) === true &&
      (zone === undefined || row.dataset.line?.startsWith(`${zone}:`)),
  )
  const row = rows[0]
  if (row === undefined) throw new Error(`no line for ${name}`)
  return row
}

/** The copies shown on a line of the deck. */
export const copies = (name: string, zone?: string): number =>
  Number(within(deckLine(name, zone)).getByText(/^\d+$/).textContent)

/** True when the deck pane has no line for the card. */
export const absent = (name: string): boolean =>
  ![...deckPane().querySelectorAll('li[data-line]')].some((row) => row.textContent?.includes(name) === true)

/** The query strings the card server saw, as plain objects, oldest first. */
export const asked = (searches: readonly URLSearchParams[]): Array<Record<string, string>> =>
  searches.map((params) => Object.fromEntries(params.entries()))
