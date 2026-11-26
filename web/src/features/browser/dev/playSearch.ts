/** What `/dev/play` lists. */
export const DEV_VIEWS = ['list', 'many', 'empty', 'loading', 'error'] as const

/** One row of every kind, those and forty more, no open game, a list that never arrives, a failed list. */
export type DevView = (typeof DEV_VIEWS)[number]

/** The address parameters of `/dev/play`, read leniently: whatever makes no sense is ignored. */
export interface DevPlaySearch {
  /** What the list shows; `list` when missing or wrong. */
  view: DevView
  /** `1` makes the visitor a signed-in player with a running game and a lobby to continue; a guest otherwise. */
  account?: true
}

/** Reads `?view=` and `?account=1`; the router has already parsed the query string. */
export function parseDevPlaySearch(search: Record<string, unknown>): DevPlaySearch {
  const view = DEV_VIEWS.find((name) => name === search.view) ?? 'list'
  return {
    view,
    ...(search.account === 1 || search.account === '1' || search.account === true
      ? { account: true as const }
      : {}),
  }
}
