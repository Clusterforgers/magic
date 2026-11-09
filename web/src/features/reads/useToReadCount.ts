/** How many distinct cards of the account's decks wait for a first read, for the nav badge and the home notice. */
import { QueryClient, useQuery } from '@tanstack/react-query'
import { readsApi } from '@/net/reads'
import type { ReadsApi } from '@/net/reads'
import type { ReadQueue } from '@/protocol/generated'

/**
 * The query client of the count. The hook names it itself, so the shell needs no provider above it; tests clear it
 * between cases.
 */
export const toReadClient = new QueryClient({
  defaultOptions: { queries: { retry: false, staleTime: 30_000, refetchOnWindowFocus: false } },
})

const KEY = 'to-read-count'

/** The cards of `queue` (every card waiting for a first read) that one of the account's decks holds, each once. */
export function myToRead(queue: ReadQueue): number {
  const mine = new Set<string>()
  for (const card of queue.cards) {
    if (card.in_decks && card.state === 'pending') mine.add(card.oracle_id)
  }
  return mine.size
}

/** Asks for the count again, after a verdict changed it. */
export function forgetToReadCount(): void {
  void toReadClient.invalidateQueries({ queryKey: [KEY] })
}

/**
 * The distinct cards of `account`'s decks whose draft waits for a read, from `GET /api/reads` (`in_decks`); 0 for no
 * account, while it loads and when the server cannot say.
 */
export function useToReadCount(account: string | null, api: Pick<ReadsApi, 'queue'> = readsApi): number {
  const query = useQuery(
    {
      queryKey: [KEY, account],
      queryFn: async () => myToRead(await api.queue(null)),
      enabled: account !== null,
    },
    toReadClient,
  )
  return account === null ? 0 : (query.data ?? 0)
}
