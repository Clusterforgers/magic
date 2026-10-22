import { useEffect, useState } from 'react'
import { ApiError } from '@/net/api'
import type { Api } from '@/net/api'
import type { Timers } from '@/net/timers'
import type { GameInfo } from '@/protocol/generated'

/** How often the lobby asks who is there, in milliseconds. */
export const LOBBY_POLL_MS = 3000

/** What a lookup keeps of the answer: all of `GameInfo`, though a stand-in lookup (a table fixture) may omit `visibility` and `format`. */
export type KnownGame = Omit<GameInfo, 'visibility' | 'format'> &
  Partial<Pick<GameInfo, 'visibility' | 'format'>>

/** What REST knows about the game: the public facts, and whether the last look worked. */
export interface GameLookup {
  /** Seat names, who is connected and the visibility, or null until the first answer. Kept when a later look fails. */
  game: KnownGame | null
  /** The server says there is no such game. */
  missing: boolean
  /** The last look could not be completed (network or server trouble). */
  failed: boolean
}

interface Options {
  /** Keep asking every `LOBBY_POLL_MS` instead of once. */
  poll: boolean
  timers: Timers
}

/** Looks up `GET /api/games/{id}` when the page opens and, while `poll` is set, again and again. */
export function useGameInfo(api: Api, game: string, { poll, timers }: Options): GameLookup {
  const [lookup, setLookup] = useState<GameLookup>({ game: null, missing: false, failed: false })

  useEffect(() => {
    let stale = false
    let timer: number | null = null
    const again = (): void => {
      if (poll) timer = timers.setTimeout(load, LOBBY_POLL_MS)
    }
    function load(): void {
      api.getGame(game).then(
        (info) => {
          if (stale) return
          setLookup({ game: info, missing: false, failed: false })
          again()
        },
        (error: unknown) => {
          if (stale) return
          const missing = error instanceof ApiError && error.code === 'not_found'
          setLookup((before) => ({ game: before.game, missing, failed: !missing }))
          if (!missing) again()
        },
      )
    }
    load()
    return () => {
      stale = true
      if (timer !== null) timers.clearTimeout(timer)
    }
  }, [api, game, poll, timers])

  return lookup
}

/** The name `lookup` has for `seat`, or "Seat N" while the lookup has none. */
export function seatName(lookup: GameLookup, seat: number): string {
  return lookup.game?.seats.find((entry) => entry.seat === seat)?.name ?? `Seat ${seat + 1}`
}
