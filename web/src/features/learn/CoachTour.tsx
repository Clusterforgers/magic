import { useEffect, useId, useRef, useState } from 'react'
import type { CSSProperties } from 'react'
import { createPortal } from 'react-dom'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { firstShown, useInSight, useTourKeys } from './tourLook'
import type { Target } from './anchors'
import { coachPlace } from './coachPlace'
import { Light } from './coachLight'
import { CARD_W, PHONE_W, RING_PAD, grown, look, useCardSize, useLooking } from './coachLook'
import type { CardLookup } from './resolve'
import { anchorLabel, plainText, textParts } from './step'

const NO_LOOKUP: CardLookup = { objects: {}, usable: new Set() }

/** What `CoachTour` draws: one step of a tour and the buttons around it. */
export interface CoachTourProps {
  /** The root's `data-slot`. */
  slot: string
  /** The tour's name, above the title. */
  name: string
  step: { id: string; title: string; text: string; at: readonly Target[] }
  /** The step's place, from 0, and how many there are. */
  index: number
  count: number
  /** Next; without it the step says what it waits for in `waiting`. */
  onNext?: () => void
  nextLabel?: string
  waiting?: string
  /** Back, shown from the second step. */
  onBack?: () => void
  onSkip: () => void
  skipLabel: string
  /**
   * Whether the page under the tour stays usable: the layer takes no pointer but on its card, and the card is no modal
   * dialog. Off, the layer takes every press.
   */
  passThrough?: boolean
  /** The arrow keys and Escape; off where the page needs them. */
  keys?: boolean
  /** The names of the cards a card ref reads. */
  lookup?: CardLookup
  /** A page's own top bar, which the phone bar docks under; the table's is measured by `look`. */
  pageBar?: boolean
}

/**
 * A tour step on the Learn coach's light: the first drawn of the step's anchors lit with the coach's ring, and the
 * coach card placed clear of it (`coachPlace`) with Skip, Back and Next. Draws the table tour and the page tours.
 */
export function CoachTour({
  slot,
  name,
  step,
  index,
  count,
  onNext,
  nextLabel = 'Next',
  waiting,
  onBack,
  onSkip,
  skipLabel,
  passThrough = false,
  keys = true,
  lookup = NO_LOOKUP,
  pageBar = false,
}: CoachTourProps) {
  const card = useRef<HTMLElement>(null)
  const next = useRef<HTMLButtonElement>(null)
  const cardSize = useCardSize(card)
  const [folded, setFolded] = useState(false)
  const titleId = useId()
  const textId = useId()
  const back = index > 0 ? onBack : undefined
  useLooking(true)
  useTourKeys(keys, onNext, back, onSkip)
  useInSight(step.at)
  useEffect(() => {
    if (!passThrough) next.current?.focus({ preventScroll: true })
  }, [index, passThrough])

  const page = look(firstShown(step.at, lookup), lookup)
  const rings = page.lit.map((each) => grown(each.box, RING_PAD))
  const top = pageBar
    ? Math.max(0, document.querySelector('header.plaque')?.getBoundingClientRect().bottom ?? 0)
    : 0
  const phone = pageBar
    ? page.window.width < PHONE_W
      ? { top, bottom: page.window.height }
      : undefined
    : page.phone
  const width = phone === undefined ? Math.min(CARD_W, page.window.width - 16) : page.window.width - 16
  const place = coachPlace({
    window: page.window,
    card: { width, height: cardSize.height },
    lit: rings,
    clear: [],
    ...(phone === undefined ? {} : { phone }),
  })
  const bar = phone !== undefined
  const shut = bar && (folded || place.folded)
  const text = plainText(textParts(step.text, anchorLabel, page.coarse))
  const style: CSSProperties = { left: place.x, top: place.y, width }

  return createPortal(
    <div
      data-slot={slot}
      data-step={step.id}
      data-tour-index={index}
      data-tour-count={count}
      data-coach-lit={page.lit.map((each) => each.via ?? each.target).join('|')}
      className={cn('fixed inset-0 z-[60]', passThrough && 'pointer-events-none')}
    >
      <Light rings={rings} targets={page.lit.map((each) => each.target)} size={page.window} />
      <section
        ref={card}
        role="dialog"
        aria-modal={passThrough ? undefined : true}
        aria-labelledby={titleId}
        aria-describedby={textId}
        data-slot="tour-card"
        data-folded={shut || undefined}
        style={style}
        className="coach-card pointer-events-auto fixed grid gap-2 rounded-xl bg-surface p-3 text-sm text-foreground shadow-e3"
      >
        <header className={cn('flex items-center gap-2', shut && 'hidden')}>
          <span className="min-w-0 flex-1 truncate text-xs text-muted-foreground">
            {name}
            {bar ? ` · ${index + 1} of ${count}` : null}
          </span>
          {bar ? null : (
            <span aria-label={`Step ${index + 1} of ${count}`} role="img" className="flex items-center gap-1">
              {Array.from({ length: count }, (_, at) => (
                <span
                  key={at}
                  data-done={at < index || undefined}
                  data-current={at === index || undefined}
                  className="coach-dot h-1.5 w-1.5 rounded-full"
                />
              ))}
            </span>
          )}
        </header>
        <h2 id={titleId} className="font-heading text-base font-semibold">
          {bar ? (
            <button
              type="button"
              className="text-left"
              aria-expanded={!shut}
              onClick={() => setFolded(!folded)}
            >
              {step.title}
            </button>
          ) : (
            step.title
          )}
        </h2>
        <p id={textId} data-slot="tour-text" className={cn(shut && 'hidden')}>
          {text}
        </p>
        <div className="flex items-center gap-2">
          <Button size="sm" variant="ghost" onClick={onSkip}>
            {skipLabel}
          </Button>
          <span className="flex-1" />
          {back === undefined ? null : (
            <Button size="sm" variant="ghost" onClick={back}>
              Back
            </Button>
          )}
          {onNext === undefined ? (
            <p data-slot="tour-waiting" role="status" className="flex items-center gap-2 text-xs font-medium">
              <span aria-hidden="true" className="coach-dot size-2 rounded-full" data-current />
              {waiting ?? 'Waiting for you'}
            </p>
          ) : (
            <Button
              ref={next}
              size="sm"
              data-slot="tour-next"
              className="bg-coach text-on-coach hover:bg-coach/90"
              onClick={onNext}
            >
              {nextLabel}
            </Button>
          )}
        </div>
      </section>
    </div>,
    document.body,
  )
}
