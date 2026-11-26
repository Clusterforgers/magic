/** What `/dev/decks` shows. */
export const DEV_VIEWS = ['groups', 'list', 'empty', 'loading', 'error', 'unavailable'] as const

/** The fixture decks in groups, the same decks with no group, no decks, a list that never arrives, a failed list, a server without decks. */
export type DevView = (typeof DEV_VIEWS)[number]

/** The address parameters of `/dev/decks`, read leniently: whatever makes no sense is ignored. */
export interface DevDecksSearch {
  /** What the page shows; `groups` when missing or wrong. */
  view: DevView
}

/** Reads `?view=`; the router has already parsed the query string. */
export function parseDevDecksSearch(search: Record<string, unknown>): DevDecksSearch {
  return { view: DEV_VIEWS.find((name) => name === search.view) ?? 'groups' }
}
