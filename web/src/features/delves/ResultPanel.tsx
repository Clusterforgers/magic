/** What the party's answers in the last camp did: each hero's choice with what it gave or cost, the viewer's first. */
import { ScrollTextIcon } from 'lucide-react'
import type { DelveView } from '@/protocol/generated'
import { EffectChip } from './EffectChips'
import { resultSentence, resultViews } from './eventEffects'

export function ResultPanel({ view }: { view: DelveView }) {
  const rows = resultViews(view.run.results, view.run.heroes, view.me)
  if (rows.length === 0) return null
  return (
    <section
      role="status"
      aria-label="What happened"
      data-slot="results"
      className="grid gap-2 rounded-xl border-2 border-relic bg-delve-stone p-3 motion-safe:animate-in motion-safe:fade-in motion-safe:duration-slow"
    >
      <h2 className="flex items-center gap-2 text-sm font-semibold">
        <ScrollTextIcon aria-hidden="true" className="size-4 text-relic" />
        What happened
      </h2>
      <ul className="grid gap-2">
        {rows.map((row) => (
          <li
            key={row.key}
            aria-label={resultSentence(row)}
            data-mine={row.mine}
            className="grid gap-1 text-sm"
          >
            <span aria-hidden="true" className="flex flex-wrap items-center gap-x-2 gap-y-1">
              <span className={row.mine ? 'font-semibold' : 'text-muted-foreground'}>
                {row.who} {row.what}
              </span>
              {row.rolls.map((roll) => (
                <span key={roll} className="text-xs font-medium text-muted-foreground">
                  {roll}
                </span>
              ))}
              {row.chips.map((chip, i) => (
                <EffectChip key={`${chip.kind}-${i}`} chip={chip} />
              ))}
              {row.chips.length === 0 && row.rolls.length === 0 ? (
                <span className="text-xs text-muted-foreground">No effect</span>
              ) : null}
            </span>
          </li>
        ))}
      </ul>
    </section>
  )
}
