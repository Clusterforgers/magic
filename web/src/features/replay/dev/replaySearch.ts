/** What `/dev/replay` loads. */
export const DEV_VIEWS = ['game', 'loading', 'missing', 'signed-out', 'offline', 'broken'] as const

/** A replay, one that never arrives, no replay (404), a visitor not signed in (401), no connection, a body the page cannot read. */
export type DevView = (typeof DEV_VIEWS)[number]

/** The replays the `game` view can load. */
export const DEV_GAMES = ['duel', 'commander'] as const

/** The recorded duel or the four-seat Commander board. */
export type DevGame = (typeof DEV_GAMES)[number]

/** The address parameters of `/dev/replay`, read leniently: whatever makes no sense is ignored. */
export interface DevReplaySearch {
  /** What the load does; `game` when missing or wrong. */
  view: DevView
  /** Which replay; `duel` when missing or wrong. */
  game: DevGame
  /** The step the table opens at: a number from 0, or `last`; the first when missing or wrong. */
  step: number | 'last'
}

const stepOf = (value: unknown): number | 'last' => {
  if (value === 'last') return 'last'
  const number = typeof value === 'number' ? value : typeof value === 'string' ? Number(value) : NaN
  return Number.isInteger(number) && number >= 0 ? number : 0
}

/** Reads `?view=`, `?game=` and `?step=`; the router has already parsed the query string. */
export function parseDevReplaySearch(search: Record<string, unknown>): DevReplaySearch {
  return {
    view: DEV_VIEWS.find((name) => name === search.view) ?? 'game',
    game: DEV_GAMES.find((name) => name === search.game) ?? 'duel',
    step: stepOf(search.step),
  }
}
