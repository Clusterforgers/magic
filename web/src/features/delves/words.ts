/** The Delves pages' words and helpers (pure, apart from the deck lists): errors, statuses and relic roles. */
import { decksApi } from '@/net/decks'
import { DelveError } from '@/net/delves'
import { preconsApi } from '@/net/precons'

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

/** A deck a hero may bring: its id for the server and its name. */
export interface DeckOption {
  value: string
  label: string
}

/** The precons and the player's own Commander decks, for the deck choice. */
export async function deckOptions(): Promise<DeckOption[]> {
  const [precons, decks] = await Promise.all([
    preconsApi.list().catch(() => ({ precons: [] })),
    decksApi.list().catch(() => []),
  ])
  return [
    ...decks
      .filter((d) => d.format === 'commander')
      .map((d) => ({ value: d.id, label: `${d.name} (yours)` })),
    ...precons.precons.map((p) => ({ value: `precon:${p.code}`, label: p.name })),
  ]
}
