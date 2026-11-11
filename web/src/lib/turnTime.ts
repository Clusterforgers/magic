/** The chess-clock tally of a seat's own turns as a figure and in words. */

const plural = (count: number, unit: string): string => `${count} ${unit}${count === 1 ? '' : 's'}`

function parts(ms: number): [number, number, number] {
  const total = Math.max(0, Math.floor(ms / 1000))
  return [Math.floor(total / 3600), Math.floor((total % 3600) / 60), total % 60]
}

/** `m:ss`, or `h:mm:ss` from an hour on; whole seconds, rounded down. */
export function formatTurnTime(ms: number): string {
  const [hours, minutes, seconds] = parts(ms)
  const ss = String(seconds).padStart(2, '0')
  return hours > 0 ? `${hours}:${String(minutes).padStart(2, '0')}:${ss}` : `${minutes}:${ss}`
}

/** The accessible name of a pod's turn time: "Time used on own turns: 12 minutes 5 seconds". */
export function turnTimeText(ms: number): string {
  const [hours, minutes, seconds] = parts(ms)
  const words = [
    hours > 0 ? plural(hours, 'hour') : null,
    hours > 0 || minutes > 0 ? plural(minutes, 'minute') : null,
    plural(seconds, 'second'),
  ].filter((word) => word !== null)
  return `Time used on own turns: ${words.join(' ')}`
}
