import { ScrollTextIcon } from 'lucide-react'
import { describeRules, gameFormat } from '@/lib/houseRules'
import type { HouseRules } from '@/protocol/generated'

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
      <ScrollTextIcon aria-hidden="true" className="mt-1 size-4 shrink-0" />
      <span className="[overflow-wrap:anywhere]">
        <span className="font-medium text-foreground">House rules:</span> {words.join(' · ')}
      </span>
    </p>
  )
}
