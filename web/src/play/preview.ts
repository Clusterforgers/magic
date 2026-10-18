/**
 * Previews: what the engine says combat damage would do for an answer that is still being chosen (docs/protocol.md,
 * Preview). The wire carries no key, so the client names each answer it asks about and keeps the name beside the reply.
 */
import type { Answer, CombatForecast, DecisionId } from '@/protocol/generated'

/** The engine's reply to the latest preview this seat asked for. */
export interface PreviewState {
  /** The decision the preview is for; only the pending decision's preview counts. */
  decision: DecisionId
  /** Names the answer previewed (`previewKey`), so a forecast is used only for the choice it describes. */
  key: string
  /** The forecast, or null when the engine had none to give for that answer (not legal, not combat, over the limit). */
  forecast: CombatForecast | null
}

/** A name for an answer, equal for equal answers: the answer as the wire carries it, order of choice included. */
export function previewKey(answer: Answer): string {
  return JSON.stringify(answer)
}
