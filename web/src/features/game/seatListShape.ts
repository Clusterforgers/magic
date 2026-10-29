/** How the lobby's list of seats is drawn for a number of seats: as it is, or as a scrolling list of tighter rows. */

/** From this many seats the list is compact: a long list scrolls inside its own box instead of pushing the page down. */
export const COMPACT_LIST_FROM_SEATS = 13

/** Classes that make the seat list compact: a box that scrolls and rows with less air. */
const COMPACT_LIST = 'max-h-[60dvh] overflow-y-auto [&>li]:py-1.5'

/** The extra classes of the seat list for `seatCount` seats; none for a short list. */
export function seatListClasses(seatCount: number): string {
  return seatCount >= COMPACT_LIST_FROM_SEATS ? COMPACT_LIST : ''
}
