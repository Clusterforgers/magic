/** The Delves pages' words and helpers (pure): errors, statuses and relic roles. */
import { DelveError } from '@/net/delves'

/** How often the run page reads the run again, in milliseconds. */
export const POLL_MS = 4000

export function errorText(error: unknown): string {
  return error instanceof DelveError ? error.message : 'Something went wrong. Try again.'
}

/** What the run is doing, in a word or two. */
export function statusText(status: string): string {
  const words: Record<string, string> = {
    lobby: 'Gathering',
    map: 'Choosing a path',
    fight: 'Fighting',
    room: 'Taking loot',
    victory: 'Victory',
    defeat: 'Defeated',
    abandoned: 'Abandoned',
  }
  return words[status] ?? status
}

/** The words a relic's role is shown with; a risky relic says that it helps the boss too. */
export function roleLabel(role: string): string {
  return role === 'Risky' ? 'Risky: helps the boss too' : role
}
