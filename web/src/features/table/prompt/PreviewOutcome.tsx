import { useDeferredValue, useMemo, useState } from 'react'
import type { CombatForecast } from '@/protocol/generated'
import { forecastSentences, forecastSummary } from '../combat/forecastText'
import { useForecastNames } from '../combat/useForecastNames'
import type { PreviewStatus } from '../combat/usePreviewRequest'

/** What the section says while the engine is being asked. */
export const CHECKING_WORDS = 'Checking outcome…'
/** What it says when no forecast will come for the choice. */
export const FALLBACK_WORDS = 'Outcome shown after you confirm.'

const NO_SENTENCES: readonly string[] = []

/** Props of `PreviewOutcome`. */
export interface PreviewOutcomeProps {
  /** The `data-slot` of the section. */
  slot: string
  /** Over the sentences when there is a forecast: "If you block like this". */
  heading: string
  /** The forecast the seam gives for the choice, or null. */
  forecast: CombatForecast | null
  /** Where the engine's answer for the choice stands. */
  status: PreviewStatus
}

/**
 * What a combat prompt says about its choice: the forecast as sentences under a heading, "Checking outcome…" while the
 * engine is asked, or "Outcome shown after you confirm." when no forecast will come. While it is asked, the last list
 * of sentences stays in the page, unseen, so the section does not change height on every click. One polite live
 * region says the result in a single sentence each time it changes; it is the node that shows the fallback words.
 */
export function PreviewOutcome({ slot, heading, forecast, status }: PreviewOutcomeProps) {
  const names = useForecastNames(forecast)
  const sentences = useMemo(
    () => (forecast === null ? NO_SENTENCES : forecastSentences(forecast, names)),
    [forecast, names],
  )
  const summary = useMemo(
    () => (forecast === null ? '' : `${heading}: ${forecastSummary(forecast, names)}`),
    [forecast, names, heading],
  )
  // The last list that was shown, kept as state (set while rendering, the way React documents it) for the unseen copy.
  const [kept, setKept] = useState(sentences)
  if (sentences.length > 0 && sentences !== kept) setKept(sentences)

  const checking = forecast === null && status === 'checking'
  const spoken = useDeferredValue(forecast !== null ? summary : checking ? '' : FALLBACK_WORDS, '')
  return (
    <div
      data-slot={slot}
      data-state={forecast !== null ? 'ready' : checking ? 'checking' : 'unavailable'}
      className="grid basis-full gap-0.5 text-sm"
    >
      {forecast !== null ? (
        <>
          <p className="font-medium">{heading}</p>
          <ul className="max-h-32 list-disc overflow-y-auto pl-5 text-muted-foreground">
            {sentences.map((sentence, index) => (
              <li key={`${index}:${sentence}`}>{sentence}</li>
            ))}
          </ul>
        </>
      ) : null}
      {checking ? (
        <>
          <p data-slot="outcome-checking" className="text-muted-foreground">
            {CHECKING_WORDS}
          </p>
          <ul aria-hidden="true" className="invisible max-h-32 list-disc overflow-y-auto pl-5">
            {kept.map((sentence, index) => (
              <li key={`${index}:${sentence}`}>{sentence}</li>
            ))}
          </ul>
        </>
      ) : null}
      <p
        aria-live="polite"
        aria-atomic="true"
        data-slot="outcome-live"
        className={forecast === null && !checking ? 'text-muted-foreground' : 'sr-only'}
      >
        {spoken}
      </p>
    </div>
  )
}
