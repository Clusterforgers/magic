/**
 * The cache of the account's lists that every deck chooser opens with (its decks, deck groups, starred precons and the
 * precons): stale while revalidate, so a list seen once shows at once on the next open and is read again behind it.
 * Each list is keyed by the client that reads it, so a test's fake never meets another's answers.
 */
import { QueryClient } from '@tanstack/react-query'
import type { DeckGroupsApi } from './deckGroups'
import type { DecksApi } from './decks'
import type { FavouritesApi } from './favourites'
import type { PreconsApi } from './precons'

/**
 * The query client of the lists. A list read more than ten seconds ago is read again when a chooser opens (shown
 * meanwhile), and kept ten minutes after its last reader; the hooks name it themselves, so no provider is needed, and
 * tests clear it between cases.
 */
export const listCache = new QueryClient({
  defaultOptions: {
    queries: { retry: false, staleTime: 10_000, gcTime: 10 * 60_000, refetchOnWindowFocus: false },
  },
})

const ids = new WeakMap<object, number>()
let next = 0

/** A number that stands for `client` in the query keys. */
function idOf(client: object): number {
  let id = ids.get(client)
  if (id === undefined) {
    next += 1
    id = next
    ids.set(client, id)
  }
  return id
}

/** The key of the account's decks read through `api`. */
export const deckListKey = (api: Pick<DecksApi, 'list'>) => ['deck-list', idOf(api)] as const
/** The key of the precons read through `api`. */
export const preconListKey = (api: Pick<PreconsApi, 'list'>) => ['precon-list', idOf(api)] as const
/** The key of the deck groups read through `api`. */
export const groupListKey = (api: Pick<DeckGroupsApi, 'list'>) => ['group-list', idOf(api)] as const
/** The key of the starred precons read through `api`. */
export const favouriteListKey = (api: Pick<FavouritesApi, 'list'>) => ['favourite-list', idOf(api)] as const

/** The account's decks: the cached list at once while a newer one is read, else the newly read one. */
export function cachedDecks(api: Pick<DecksApi, 'list'>) {
  return listCache.ensureQueryData({
    queryKey: deckListKey(api),
    queryFn: () => api.list(),
    revalidateIfStale: true,
  })
}

/** The precons, as [`cachedDecks`]. */
export function cachedPrecons(api: Pick<PreconsApi, 'list'>) {
  return listCache.ensureQueryData({
    queryKey: preconListKey(api),
    queryFn: () => api.list(),
    revalidateIfStale: true,
  })
}

/** The deck groups, as [`cachedDecks`]. */
export function cachedGroups(api: Pick<DeckGroupsApi, 'list'>) {
  return listCache.ensureQueryData({
    queryKey: groupListKey(api),
    queryFn: () => api.list(),
    revalidateIfStale: true,
  })
}

/** The account's lists a chooser opens with. */
export interface ChooserLists {
  decks: Pick<DecksApi, 'list'>
  groups: Pick<DeckGroupsApi, 'list'>
  favourites: Pick<FavouritesApi, 'list'>
  precons: Pick<PreconsApi, 'list'>
}

/**
 * Reads the lists a deck chooser opens with ahead of it (once the account is signed in and the page has settled), so it
 * opens on them; a list read within the last half minute is not read again, and a failure is left for the chooser.
 */
export function prefetchChooserLists(lists: ChooserLists): void {
  const staleTime = 30_000
  void listCache.prefetchQuery({
    queryKey: deckListKey(lists.decks),
    queryFn: () => lists.decks.list(),
    staleTime,
  })
  void listCache.prefetchQuery({
    queryKey: groupListKey(lists.groups),
    queryFn: () => lists.groups.list(),
    staleTime,
  })
  void listCache.prefetchQuery({
    queryKey: favouriteListKey(lists.favourites),
    queryFn: () => lists.favourites.list(),
    staleTime,
  })
  void listCache.prefetchQuery({
    queryKey: preconListKey(lists.precons),
    queryFn: () => lists.precons.list(),
    staleTime,
  })
}

/** Drops the cached deck lists after a deck was saved, renamed or deleted, so no chooser offers what is gone. */
export function forgetDeckLists(): void {
  listCache.removeQueries({ queryKey: ['deck-list'], type: 'inactive' })
  void listCache.invalidateQueries({ queryKey: ['deck-list'] })
}
