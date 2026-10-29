/** What `/dev/precons` shows of the list. */
export const DEV_VIEWS = ['list', 'empty', 'loading', 'error', 'unavailable'] as const

/** One of the views: the fixture list, an empty server, a list that never arrives, a failed load, a server with no cards. */
export type DevView = (typeof DEV_VIEWS)[number]

/** The address parameters of `/dev/precons`, read leniently: whatever makes no sense is ignored. */
export interface DevPreconsSearch {
  /** What the list does; `list` when missing or wrong. */
  view: DevView
  /** `1` makes this page a signed-in player, who plays in one click; a guest is asked for a name. */
  account?: true
  /** `refused` makes every Play end in the server's 422; a game is made when missing. */
  play?: 'refused'
}

/** Reads `?view=`, `?account=1` and `?play=refused`; the router has already parsed the query string. */
export function parseDevPreconsSearch(search: Record<string, unknown>): DevPreconsSearch {
  const view = DEV_VIEWS.find((name) => name === search.view) ?? 'list'
  return {
    view,
    ...(search.account === 1 || search.account === '1' || search.account === true
      ? { account: true as const }
      : {}),
    ...(search.play === 'refused' ? { play: 'refused' as const } : {}),
  }
}
