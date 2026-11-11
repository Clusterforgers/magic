/** What `/dev/read` shows. */
export const DEV_READ_VIEWS = [
  'deck',
  'precon',
  'all',
  'mine',
  'done',
  'loading',
  'error',
  'stale',
  'dialog',
  'long',
] as const

/** One of the views: a deck to read, a precon, every card, the cards of the account's decks, nothing left, a wait, a failed list, a verdict that meets a changed draft, the table's dialog, or a long deck with a very long name. */
export type DevReadView = (typeof DEV_READ_VIEWS)[number]

/** The address parameters of `/dev/read`, read leniently: whatever makes no sense is ignored. */
export interface DevReadSearch {
  /** What the page shows; `deck` when missing or wrong. */
  view: DevReadView
}

/** Reads `?view=`; the router has already parsed the query string. */
export function parseDevReadSearch(search: Record<string, unknown>): DevReadSearch {
  return { view: DEV_READ_VIEWS.find((name) => name === search.view) ?? 'deck' }
}
