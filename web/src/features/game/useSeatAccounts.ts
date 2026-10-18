import { useEffect, useMemo } from 'react'
import { useAuth, useAuthStore } from '@/features/auth/context'

/** The accounts in a game's seats, as far as the signed-in user's own list of games knows. */
export interface SeatAccounts {
  /** The username signed in on this page; null for a guest and on a server without accounts. */
  me: string | null
  /** True when the list confirms that this page's seat is saved to the signed-in account. */
  saved: boolean
  /** The username of the account sitting at each seat that has one, by seat number. */
  bySeat: ReadonlyMap<number, string>
}

/**
 * Which accounts sit where in `game`, for the lobby. The server tells only a signed-in user, and only about games
 * they hold a seat in (`GET /api/me/games`), so a guest gets nothing here. The list is asked for again whenever
 * `refreshKey` changes: a seat link opened while signed in saves its seat, and the lobby learns of it when someone
 * new connects.
 */
export function useSeatAccounts(game: string, seat: number | null, refreshKey: string): SeatAccounts {
  const me = useAuth((state) => state.user?.username ?? null)
  const entry = useAuth((state) => state.games.games.find((listed) => listed.game === game) ?? null)
  const store = useAuthStore()

  useEffect(() => {
    if (me !== null) void store.getState().loadGames()
  }, [me, store, refreshKey])

  return useMemo(() => {
    const bySeat = new Map<number, string>()
    const saved = entry !== null && me !== null && seat !== null && entry.seat === seat
    if (entry !== null) {
      for (const other of entry.others) {
        if (other.user !== null) bySeat.set(other.seat, other.user)
      }
      if (saved && me !== null) bySeat.set(entry.seat, me)
    }
    return { me, saved, bySeat }
  }, [entry, me, seat])
}
