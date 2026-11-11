import { createStore } from 'zustand/vanilla'
import type { GlossaryKind } from '@/lib/glossary'

/** The lexicon's state: open or shut, the search and the kind shown. */
export interface LexiconState {
  open: boolean
  query: string
  kind: GlossaryKind | 'all'
}

/** The one lexicon of the page; the help menu and every keyword word open it. */
export const lexiconStore = createStore<LexiconState>(() => ({ open: false, query: '', kind: 'all' }))

/** Opens the lexicon, at the term named `query` when there is one. */
export function showLexicon(query = ''): void {
  lexiconStore.setState({ open: true, query, kind: 'all' })
}

/** Opens or shuts the lexicon, keeping its search. */
export function setLexiconOpen(open: boolean): void {
  lexiconStore.setState({ open })
}
