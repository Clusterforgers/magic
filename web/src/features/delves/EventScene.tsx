/** An event: its story, then each choice as a card that says plainly what it costs and what it gives or risks, and why it is shut. */
import { ScrollTextIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'
import type { DelveEventView, DelveOptionView } from '@/protocol/generated'
import { EffectChip, Effects } from './EffectChips'
import { isEmpty, optionViews } from './eventEffects'
import type { HeroFacts, OptionView } from './eventEffects'

/** One side of a choice: its heading over its chips. */
function Side({ side, children }: { side: 'cost' | 'reward'; children: ReactNode }) {
  return (
    <span data-side={side} className="grid content-start gap-1">
      <span className="text-xs font-semibold tracking-wide text-muted-foreground uppercase">
        {side === 'cost' ? 'Cost' : 'Reward'}
      </span>
      {children}
    </span>
  )
}

function Choice({
  index,
  option,
  words,
  busy,
  onPick,
}: {
  index: number
  option: OptionView
  /** The server's words for what the option costs and gives, read out with it. */
  words: Pick<DelveOptionView, 'cost' | 'reward'> | undefined
  busy: boolean
  onPick: () => void
}) {
  const id = `event-option-${index}`
  const shut = option.blocked !== null
  const nothing = isEmpty(option.part)
  const said =
    nothing || words === undefined ? 'Nothing happens' : `Cost: ${words.cost}. Reward: ${words.reward}`
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
        <span aria-hidden="true" className="grid gap-2">
          {nothing ? (
            <span className="text-sm text-muted-foreground">Nothing happens.</span>
          ) : (
            <span className="grid gap-x-4 gap-y-2 sm:grid-cols-2">
              <Side side="cost">
                {option.cost.length === 0 ? (
                  <span data-free="" className="text-sm font-semibold text-success">
                    Free
                  </span>
                ) : (
                  <span className="flex flex-wrap items-center gap-2">
                    {option.cost.map((chip, i) => (
                      <EffectChip key={`${chip.kind}-${i}`} chip={chip} />
                    ))}
                  </span>
                )}
              </Side>
              <Side side="reward">
                {isEmpty(option.reward) ? (
                  <span className="text-sm text-muted-foreground">Nothing</span>
                ) : (
                  <Effects part={option.reward} />
                )}
              </Side>
            </span>
          )}
          {shut ? <span className="text-sm font-medium text-destructive">{option.blocked}</span> : null}
        </span>
        <span id={`${id}-says`} className="sr-only">
          {said}
          {shut ? `. ${option.blocked}` : ''}
        </span>
      </button>
    </li>
  )
}

/** The event card of the run page: the story in a quote, then the choices with their costs and rewards. */
export function EventScene({
  event,
  hero,
  busy,
  onPick,
}: {
  event: DelveEventView | undefined
  /** What gates the hero's choices; a fact left out never shuts one. */
  hero: HeroFacts
  busy: boolean
  onPick: (option: number) => void
}) {
  const options = optionViews(event?.options ?? [], hero)
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
              words={event?.options[i]}
              busy={busy}
              onPick={() => onPick(i)}
            />
          ))}
        </ol>
      </div>
    </article>
  )
}
