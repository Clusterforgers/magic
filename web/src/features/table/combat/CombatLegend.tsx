import { useMemo } from 'react'
import { useBlockPreview } from './forecast'
import { forecastSentences } from './forecastText'
import { legendEntries } from './legend'
import { useCombatPlan } from './useCombatPlan'
import { useForecastNames } from './useForecastNames'
import './combat.css'

/**
 * The key to the lines over the table, only for the kinds on screen (an attack, a block, each outcome that occurs,
 * choices not sent yet), and the forecast as sentences for a screen reader. Nothing when there is no combat to explain.
 */
export function CombatLegend() {
  const lines = useCombatPlan()
  const forecast = useBlockPreview()
  const names = useForecastNames(forecast)
  const entries = useMemo(() => legendEntries(lines), [lines])
  const sentences = useMemo(
    () => (forecast === null ? [] : forecastSentences(forecast, names)),
    [forecast, names],
  )
  if (entries.length === 0 && sentences.length === 0) return null
  return (
    <div data-slot="combat-legend" className="basis-full">
      {entries.length === 0 ? null : (
        // The lines are decoration (the words below are the same facts for a screen reader), so the key is too.
        <ul aria-hidden="true" data-slot="combat-key" className="flex flex-wrap gap-x-4 gap-y-1 text-xs">
          {entries.map((entry) => (
            <li
              key={entry.id}
              data-legend={entry.id}
              className="flex items-center gap-1.5 text-muted-foreground"
            >
              <svg
                width="30"
                height="10"
                viewBox="0 0 30 10"
                className="combat-group"
                data-kind={entry.kind}
                data-pending={entry.pending || undefined}
                data-outcome={entry.outcome}
              >
                <path className="combat-line" d="M2 5 H28" />
              </svg>
              {entry.label}
            </li>
          ))}
        </ul>
      )}
      {sentences.length === 0 ? null : (
        <ul aria-label="Combat forecast" data-slot="forecast-sentences" className="sr-only">
          {sentences.map((sentence, index) => (
            <li key={`${index}:${sentence}`}>{sentence}</li>
          ))}
        </ul>
      )}
    </div>
  )
}
