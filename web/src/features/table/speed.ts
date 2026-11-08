/** The words of a seat's speed (CR 702.179). Pure. */

/** The highest speed: max speed. */
export const MAX_SPEED = 4

/** "Bo's speed: 2" or "Bo's speed: 4, max speed". */
export function speedWords(owner: string, speed: number): string {
  return `${owner}'s speed: ${speed}${speed >= MAX_SPEED ? ', max speed' : ''}`
}
