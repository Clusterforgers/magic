/** What `/dev/unvalidated` shows. */
export const DEV_VIEWS = [
  'all',
  'deck',
  'clean',
  'none',
  'loading',
  'error',
  'unavailable',
  'failed',
  'texts',
] as const

/** One of the views: every deck, one deck, decks that run whole, no decks, a wait, a failed list, a server without decks, one failed check, or card texts that cannot be read. */
export type DevView = (typeof DEV_VIEWS)[number]

/** The address parameters of `/dev/unvalidated`, read leniently: whatever makes no sense is ignored. */
export interface DevUnvalidatedSearch {
  /** What the page shows; `all` when missing or wrong. */
  view: DevView
}

/** Reads `?view=`; the router has already parsed the query string. */
export function parseDevUnvalidatedSearch(search: Record<string, unknown>): DevUnvalidatedSearch {
  return { view: DEV_VIEWS.find((name) => name === search.view) ?? 'all' }
}
