/** An in-memory art server for tests: a card's printings, the lines' choices and the preferred art it was told. */
import type { PrintingsApi } from '@/net/printings'
import { DeckError } from '@/net/decks'
import type { DeckZone, PrintingView } from '@/protocol/generated'

/** A printing of a set, newest first in a list when its date is later. */
export const printingView = (id: string, set: string, over: Partial<PrintingView> = {}): PrintingView => ({
  printing_id: id,
  set,
  collector_number: '1',
  rarity: 'common',
  released_at: '2020-01-01',
  lang: 'en',
  digital: false,
  images: { card: null, faces: [] },
  set_name: null,
  frame: '2015',
  finishes: ['nonfoil'],
  full_art: false,
  ...over,
})

/** What the fake server saw and holds. */
export interface FakePrintings {
  api: PrintingsApi
  /** Every `setLine` call, oldest first. */
  lines: Array<{ deckId: string; zone: DeckZone; oracleId: string; printingId: string | null }>
  /** The preferred art, by oracle id. */
  preferred: Record<string, string>
}

/** Starts a server that knows `printings` by oracle id; `failLine` makes every line choice fail with that message. */
export function startPrintings(
  printings: Record<string, PrintingView[]>,
  options: { failLine?: string; preferred?: Record<string, string> } = {},
): FakePrintings {
  const state: FakePrintings = {
    lines: [],
    preferred: { ...options.preferred },
    api: {
      printingsOf: (oracleId) => Promise.resolve(printings[oracleId] ?? []),
      setLine: (deckId, zone, oracleId, printingId) => {
        if (options.failLine !== undefined)
          return Promise.reject(new DeckError('invalid_deck', 400, options.failLine))
        state.lines.push({ deckId, zone, oracleId, printingId })
        return Promise.resolve()
      },
      preferred: () => Promise.resolve({ ...state.preferred }),
      prefer: (oracleId, printingId) => {
        if (printingId === null) delete state.preferred[oracleId]
        else state.preferred[oracleId] = printingId
        return Promise.resolve()
      },
    },
  }
  return state
}
