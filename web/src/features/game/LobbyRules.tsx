import { ScrollTextIcon } from 'lucide-react'
import { useEffect, useState } from 'react'
import { describeRules, gameFormat } from '@/lib/houseRules'
import type { Visibility } from '@/protocol/generated'
import type { HouseRules } from '@/protocol/generated'
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

interface LobbyRulesProps {
  rules: HouseRules | undefined
  /** The format key of the game. */
  format: string
}

/** "House rules: Starting life 30 · 2 free mulligans": the rules that differ from the format's own; nothing when none do. */
export function LobbyRules({ rules, format }: LobbyRulesProps) {
  const words = describeRules(rules, gameFormat(format))
  if (words.length === 0) return null
  return (
    <p data-slot="lobby-rules" className="mt-2 flex items-start gap-2 text-sm text-muted-foreground">
      <ScrollTextIcon aria-hidden="true" className="mt-0.5 size-4 shrink-0" />
      <span className="[overflow-wrap:anywhere]">
        <span className="font-medium text-foreground">House rules:</span> {words.join(' · ')}
      </span>
    </p>
  )
}
