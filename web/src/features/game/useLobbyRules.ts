import { useEffect, useState } from 'react'
import type { HouseRules, Visibility } from '@/protocol/generated'
import type { LobbyApi } from './useLobbyControls'

/**
 * The house rules of a listed game, from its row in `GET /api/lobbies` (the lookup of one game carries none, so an
 * invite-only game names no rules in its lobby until it starts). Asked once, when the lookup says the game is listed.
 */
export function useLobbyRules(
  api: Pick<LobbyApi, 'listLobbies'>,
  game: string,
  visibility: Visibility | undefined,
): HouseRules | undefined {
  const [found, setFound] = useState<{ game: string; rules: HouseRules | undefined } | null>(null)
  useEffect(() => {
    if (visibility !== 'public') return
    let stale = false
    api.listLobbies().then(
      (listing) => {
        if (!stale) setFound({ game, rules: listing.games.find((row) => row.game === game)?.house_rules })
      },
      () => undefined,
    )
    return () => {
      stale = true
    }
  }, [api, game, visibility])
  return found?.game === game ? found.rules : undefined
}
