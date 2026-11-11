import { createStore } from 'zustand/vanilla'
import type { GlossaryKind } from '@/lib/glossary'

/** The lexicon's state: open or shut, the search and the kind shown, and whether a tour holds it open. */
export interface LexiconState {
  open: boolean
  query: string
  kind: GlossaryKind | 'all'
  /** A tour runs over it: it stays open and modeless, so the tour's card beside it takes presses. */
  held: boolean
}

/** The one lexicon of the page; the help menu and every keyword word open it. */
export const lexiconStore = createStore<LexiconState>(() => ({
  open: false,
  query: '',
  kind: 'all',
  held: false,
}))

/** Opens the lexicon, at the term named `query` when there is one. */
export function showLexicon(query = ''): void {
  lexiconStore.setState({ open: true, query, kind: 'all' })
}

/** Opens or shuts the lexicon, keeping its search; a held lexicon stays open. */
export function setLexiconOpen(open: boolean): void {
  if (!open && lexiconStore.getState().held) return
  lexiconStore.setState({ open })
}

/** Holds the lexicon open for a tour, or lets it go; `close` shuts it as it is let go. */
export function setLexiconHeld(held: boolean, close = false): void {
  lexiconStore.setState(close ? { held, open: false } : { held })
}
