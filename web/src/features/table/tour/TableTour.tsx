import { useContext, useEffect, useId, useMemo, useRef, useState } from 'react'
import type { CSSProperties } from 'react'
import { createPortal } from 'react-dom'
import { useStore } from 'zustand'
import { Button } from '@/components/ui/button'
import { useTourKeys } from '@/components/tour/TourCard'
import { usePlay } from '@/features/game/gameContext'
import { coachPlace } from '@/features/learn/coachPlace'
import { Light } from '@/features/learn/coachLight'
import { CARD_W, RING_PAD, grown, look, useCardSize, useLooking } from '@/features/learn/coachLook'
import type { CardLookup } from '@/features/learn/resolve'
import { anchorLabel, plainText, textParts } from '@/features/learn/step'
import { cn } from '@/lib/utils'
import { useSide } from '../sideContext'
import { TableOptionsContext, TOUR_BY_DEFAULT } from '../tableOptions'
import { endTour, moveTour, startTour, tourSeen, tourStore } from './tourStore'
import { TOUR, tourTarget } from './tourSteps'
import type { TourStep } from './tourSteps'

/** Brings up the side tab a step points at, and closes a rail or sheet panel the tour opened when it ends. */
function useStepTab(step: TourStep | undefined): void {
  const side = useSide()
  const opened = useRef(false)
  const { kind, tab, open, activate, setOpen } = side
  useEffect(() => {
    if (step?.tab === undefined) return
    if (tab === step.tab && open) return
    if (!open && kind !== 'column') opened.current = true
    if (tab === step.tab) setOpen(true)
    else activate(step.tab)
  }, [step, kind, tab, open, activate, setOpen])
  useEffect(() => {
    if (step !== undefined || !opened.current) return
    opened.current = false
    if (kind !== 'column') setOpen(false)
  }, [step, kind, setOpen])
}

const forward = (): void => moveTour(1)
const back = (): void => moveTour(-1)
const NO_CARDS: ReadonlySet<number> = new Set()

/**
 * The table tour for a new player, on the Learn coach's light (`features/learn/coachLight`): one anchor of the table
 * lit at a time with a card that says what it is for, Back, Next and Skip tour. It starts by itself the first time a
 * seat sits at a table in this browser (`tourSeen`) and again from the help menu (`TableOptions.tour`). While it is up
 * the table under it takes no presses.
 */
export function TableTour() {
  const index = useStore(tourStore, (state) => state.step)
  const seated = usePlay((state) => state.seat !== null)
  const step = index === null ? undefined : TOUR[index]
  useStepTab(step)

  const auto = useContext(TableOptionsContext).tour ?? TOUR_BY_DEFAULT
  useEffect(() => {
    // A browser under automation (end-to-end tests, screenshots) is no new player.
    if (auto && seated && !navigator.webdriver && !tourSeen() && tourStore.getState().step === null)
      startTour()
  }, [auto, seated])

  if (step === undefined || index === null) return null
  return <TourCoach step={step} index={index} />
}

/** The coach's light and card for one step of the tour. */
function TourCoach({ step, index }: { step: TourStep; index: number }) {
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
