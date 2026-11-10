import { LegalityWarning } from '@/features/decks/LegalityWarning'
import type { Issue } from '@/protocol/generated'
import { legalityLines } from './report'

interface LegalitySectionProps {
  /** `report.legality`: why the deck is not legal in its format. */
  issues: readonly Issue[]
  /** The format the report checked the deck in. */
  format: string
  /** The draft differs from the saved deck, so the issues are those of the saved one. */
  stale: boolean
}

/**
 * The ways the deck breaks the rules of its format, as a warning with plain sentences: the deck still goes to a game.
 * They are legality, not support, so they sit apart from the cards the game cannot play by itself. Nothing is drawn
 * for a legal deck.
 */
export function LegalitySection({ issues, format, stale }: LegalitySectionProps) {
  if (issues.length === 0) return null
  return (
    <LegalityWarning
      slot="legality"
      format={format}
      issues={issues}
      lines={legalityLines(issues)}
      aside={stale ? ' (as saved)' : null}
    />
  )
}
