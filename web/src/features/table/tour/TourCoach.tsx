import { useEffect, useId, useMemo, useRef, useState } from 'react'
import type { CSSProperties } from 'react'
import { createPortal } from 'react-dom'
import { Button } from '@/components/ui/button'
import { useTourKeys } from '@/components/tour/TourCard'
import { usePlay } from '@/features/game/gameContext'
import { coachPlace } from '@/features/learn/coachPlace'
import { Light } from '@/features/learn/coachLight'
import { CARD_W, RING_PAD, grown, look, useCardSize, useLooking } from '@/features/learn/coachLook'
import type { CardLookup } from '@/features/learn/resolve'
import { anchorLabel, plainText, textParts } from '@/features/learn/step'
import { cn } from '@/lib/utils'
import { endTour, moveTour } from './tourStore'
import { TOUR, tourTarget } from './tourSteps'
import type { TourStep } from './tourSteps'

const forward = (): void => moveTour(1)
const back = (): void => moveTour(-1)
const NO_CARDS: ReadonlySet<number> = new Set()

/**
 * The coach's light and card for one step of the table tour: the step's anchor lit with the coach's ring, and a card
 * placed clear of it (`coachPlace`) with Skip tour, Back and Next; the arrow keys and Escape do the same. It takes every
 * press, so the table under it takes none.
 */
export function TourCoach({ step, index }: { step: TourStep; index: number }) {
  const objects = usePlay((state) => state.objectsById)
  const lookup = useMemo<CardLookup>(() => ({ objects, usable: NO_CARDS }), [objects])
  const card = useRef<HTMLElement>(null)
  const next = useRef<HTMLButtonElement>(null)
  const cardSize = useCardSize(card)
  const [folded, setFolded] = useState(false)
  const titleId = useId()
  const textId = useId()
  const last = index === TOUR.length - 1
  useLooking(true)
  useTourKeys(true, forward, index > 0 ? back : undefined, endTour)
  useEffect(() => next.current?.focus({ preventScroll: true }), [index])

  const page = look(tourTarget(step, lookup), lookup)
  const rings = page.lit.map((each) => grown(each.box, RING_PAD))
  const phone = page.phone
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
      data-slot="table-tour"
      data-step={step.id}
      data-tour-index={index}
      data-tour-count={TOUR.length}
      data-coach-lit={page.lit.map((each) => each.via ?? each.target).join('|')}
      className="fixed inset-0 z-[60]"
    >
      <Light rings={rings} targets={page.lit.map((each) => each.target)} size={page.window} />
      <section
        ref={card}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={textId}
        data-slot="tour-card"
        data-folded={shut || undefined}
        style={style}
        className="coach-card fixed grid gap-2 rounded-xl bg-surface p-3 text-sm text-foreground shadow-e3"
      >
        <header className={cn('flex items-center gap-2', shut && 'hidden')}>
          <span className="min-w-0 flex-1 truncate text-xs text-muted-foreground">
            Table tour
            {bar ? ` · ${index + 1} of ${TOUR.length}` : null}
          </span>
          {bar ? null : (
            <span
              aria-label={`Step ${index + 1} of ${TOUR.length}`}
              role="img"
              className="flex items-center gap-1"
            >
              {TOUR.map((each, at) => (
                <span
                  key={each.id}
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
          <Button size="sm" variant="ghost" onClick={endTour}>
            Skip tour
          </Button>
          <span className="flex-1" />
          {index > 0 ? (
            <Button size="sm" variant="ghost" onClick={back}>
              Back
            </Button>
          ) : null}
          <Button
            ref={next}
            size="sm"
            data-slot="tour-next"
            className="bg-coach text-on-coach hover:bg-coach/90"
            onClick={forward}
          >
            {last ? 'Done' : 'Next'}
          </Button>
        </div>
      </section>
    </div>,
    document.body,
  )
}
