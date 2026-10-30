/** Where the turn banner stands: in the free room of the top bar, or under the bar where the bar has none. Pure, so the sizes are a table. */

/** The least free room in px of the bar for the banner to stand in it: the words of "Your turn" and its padding. */
export const BANNER_BAR_MIN_PX = 112

/** The free room in px from which the banner also says the turn number. */
export const BANNER_TURN_MIN_PX = 200

/** Where the banner stands and what it says. */
export interface BannerPlace {
  /** `bar` is the free room between the step ladder and the menu; `below` hangs from the bar's lower edge. */
  place: 'bar' | 'below'
  /** Whether the turn number is written: the badge says it too, so a narrow room goes without. */
  turn: boolean
}

/** The place of the banner for `room` px free in the top bar; null (no layout yet) is a room that holds all of it. */
export function bannerPlace(room: number | null): BannerPlace {
  if (room === null) return { place: 'bar', turn: true }
  if (room < BANNER_BAR_MIN_PX) return { place: 'below', turn: true }
  return { place: 'bar', turn: room >= BANNER_TURN_MIN_PX }
}
