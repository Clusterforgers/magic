/** The legend of the overlay: which kinds of line are on screen, and what each one means. Pure. */
import { OUTCOME_WORDS, OUTCOMES } from './outcome'
import type { Outcome } from './outcome'
import type { LineKind, LineSpec } from './plan'

/** One kind of line in the legend. */
export interface LegendEntry {
  /** Unique among the entries. */
  id: string
  /** What the line means. */
  label: string
  kind: LineKind
  pending: boolean
  outcome?: Outcome
}

/**
 * The entries to list for `lines`, only for the kinds on screen: attacks, blocks without a forecast, one entry per
 * outcome that occurs, then the unsent choices.
 */
export function legendEntries(lines: readonly LineSpec[]): LegendEntry[] {
  const declared = lines.filter((line) => !line.pending)
  const entries: LegendEntry[] = []
  if (declared.some((line) => line.kind === 'attack')) {
    entries.push({ id: 'attack', label: 'Attack', kind: 'attack', pending: false })
  }
  if (declared.some((line) => line.kind === 'block' && line.outcome === undefined)) {
    entries.push({ id: 'block', label: 'Block', kind: 'block', pending: false })
  }
  for (const outcome of OUTCOMES) {
    if (declared.some((line) => line.outcome === outcome)) {
      entries.push({ id: outcome, label: OUTCOME_WORDS[outcome], kind: 'block', pending: false, outcome })
    }
  }
  if (lines.some((line) => line.pending)) {
    entries.push({ id: 'pending', label: 'Chosen, not confirmed yet', kind: 'block', pending: true })
  }
  return entries
}
