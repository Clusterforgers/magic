import { useEffect } from 'react'
import { deckGroupsApi } from '@/net/deckGroups'
import { decksApi } from '@/net/decks'
import { favouritesApi } from '@/net/favourites'
import { prefetchChooserLists } from '@/net/listCache'
import type { ChooserLists } from '@/net/listCache'
import { preconsApi } from '@/net/precons'

/** The shared clients of the lists a deck chooser opens with. */
const SHARED: ChooserLists = {
  decks: decksApi,
  groups: deckGroupsApi,
  favourites: favouritesApi,
  precons: preconsApi,
}

/** How long after sign-in or page load the lists are read, so they do not compete with the page's own requests. */
export const PREFETCH_DELAY_MS = 1500

/**
 * Reads the lists of a deck chooser (the account's decks, groups, stars and the precons) once the account is signed in
 * and the page has settled, so the chooser of a game, a delve or the Decks page opens on them.
 */
export function usePrefetchChooserLists(signedIn: boolean, lists: ChooserLists = SHARED): void {
  useEffect(() => {
    if (!signedIn) return
    const timer = setTimeout(() => prefetchChooserLists(lists), PREFETCH_DELAY_MS)
    return () => clearTimeout(timer)
  }, [signedIn, lists])
}
