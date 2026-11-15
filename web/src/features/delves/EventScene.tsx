/** An event: its story, then each choice as a card that says plainly what it gives, costs or risks, and why it is shut. */
import { ScrollTextIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { DelveEventView } from '@/protocol/generated'
import { Effects } from './EffectChips'
import { isEmpty, optionViews } from './eventEffects'
import type { OptionView } from './eventEffects'

function Choice({
  index,
  option,
  summary,
  busy,
  onPick,
}: {
  index: number
  option: OptionView
  /** The server's words for what the option does, read out with it. */
  summary: string
  busy: boolean
  onPick: () => void
}) {
  const id = `event-option-${index}`
  const shut = option.blocked !== null
  return (
    <li>
      <button
        type="button"
        aria-label={option.label}
        aria-describedby={`${id}-says`}
        disabled={busy || shut}
        data-blocked={shut}
        onClick={onPick}
        className={cn(
          'grid min-h-11 w-full gap-2 rounded-xl border bg-surface p-3 text-left transition-colors',
          shut
            ? 'cursor-not-allowed opacity-70'
            : 'hover:border-delve hover:bg-delve-stone focus-visible:border-delve disabled:opacity-60',
        )}
      >
        <span className="flex items-center gap-2">
          <span
            aria-hidden="true"
            className="grid size-6 shrink-0 place-items-center rounded-full bg-delve text-xs font-semibold text-on-delve"
          >
            {index + 1}
          </span>
          <span className="font-semibold">{option.label}</span>
        </span>
        <span aria-hidden="true" className="grid gap-1">
          {isEmpty(option.part) ? (
            <span className="text-sm text-muted-foreground">Nothing happens.</span>
          ) : (
            <Effects part={option.part} />
          )}
          {shut ? <span className="text-sm font-medium text-destructive">{option.blocked}</span> : null}
        </span>
        <span id={`${id}-says`} className="sr-only">
          {isEmpty(option.part) ? 'Nothing happens' : summary}
          {shut ? `. ${option.blocked}` : ''}
        </span>
      </button>
    </li>
  )
}

/** The event card of the run page: the story in a quote, then the choices with their effects. */
export function EventScene({
  event,
  gold,
  life,
  busy,
  onPick,
}: {
  event: DelveEventView | undefined
  gold: number
  /** The hero's life; the gold alone gates an option when it is unknown. */
  life: number | undefined
  busy: boolean
  onPick: (option: number) => void
}) {
  const options = optionViews(event?.options ?? [], gold, life ?? Number.MAX_SAFE_INTEGER)
  return (
    <article className="mx-auto grid w-full max-w-prose gap-4 rounded-xl border-2 border-relic bg-surface p-4 shadow-e2 sm:p-6">
      <p className="flex gap-2 text-sm italic leading-relaxed text-muted-foreground">
        <ScrollTextIcon aria-hidden="true" className="mt-1 size-4 shrink-0" />
        <span className="whitespace-pre-line">{event?.text}</span>
      </p>
      <div className="grid gap-2">
        <h3 className="flex flex-wrap items-baseline gap-x-2 text-sm font-semibold">
          Choose one
          <span className="text-xs font-normal text-muted-foreground">
            Each hero chooses for themselves: the effects are yours alone.
          </span>
        </h3>
        <ol className="grid gap-2">
          {options.map((option, i) => (
            <Choice
              key={option.label}
              index={i}
              option={option}
              summary={event?.options[i]?.summary ?? ''}
              busy={busy}
              onPick={() => onPick(i)}
            />
          ))}
        </ol>
      </div>
    </article>
  )
}
