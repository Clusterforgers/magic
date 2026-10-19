/** Words for an age: how long ago a game or a deck was made or saved. */

const MINUTE = 60_000
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

// Spelled out rather than asked of `Intl`, whose short month names differ between versions ("Sep" and "Sept").
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']

const plural = (count: number, unit: string): string => `${count} ${unit}${count === 1 ? '' : 's'} ago`

/** "just now", "5 minutes ago", "3 hours ago", "2 days ago", and a date once it is a month old. */
export function timeSince(createdAt: string, now: number): string {
  const created = Date.parse(createdAt)
  if (Number.isNaN(created)) return 'some time ago'
  const age = now - created
  if (age < MINUTE) return 'just now'
  if (age < HOUR) return plural(Math.floor(age / MINUTE), 'minute')
  if (age < DAY) return plural(Math.floor(age / HOUR), 'hour')
  if (age < 30 * DAY) return plural(Math.floor(age / DAY), 'day')
  const date = new Date(created)
  return `on ${date.getUTCDate()} ${MONTHS[date.getUTCMonth()] ?? ''} ${date.getUTCFullYear()}`
}
