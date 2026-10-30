import { Zap } from 'lucide-react'
import { useLayoutEffect, useRef } from 'react'
import type { CSSProperties, Ref } from 'react'
import { CardBack } from '@/features/cards/CardImage'
import { CardFace } from '../CardFace'
import { CardWidthContext } from '../ImageFace'
import { useFx } from './hooks'
import { EASE_OUT, keyframesOf, REVEAL_KEYFRAMES } from './travel'
import type { Flight, Size, Travel } from './travel'
import { useFlights } from './useFlights'

/** A face this wide or wider draws the type line and the rest, not only the name and cost. */
const NORMAL_FACE_PX = 76

const pictureStyle = ({ width, height }: Size): CSSProperties =>
  ({ '--card-w': `${width}px`, '--card-h': `${height}px` }) as CSSProperties

/** The picture a card flies as: the back, the face, or the back that turns to the face as it lands. */
function Picture({ flight, size, face }: { flight: Flight; size: Size; face: Ref<HTMLSpanElement> }) {
  const { look } = flight.move
  const layer =
    'relative col-start-1 row-start-1 grid h-(--card-h) w-(--card-w) place-items-center rounded-md'
  return (
    <span className="grid shadow-lg" style={pictureStyle(size)}>
      {flight.faceDown ? <CardBack className={layer} /> : null}
      {look !== null && (!flight.faceDown || flight.reveal) ? (
        <span ref={face} className={`${layer} bg-background`}>
          <CardWidthContext.Provider value={size.width}>
            <CardFace
              object={look}
              role={null}
              size={size.width < NORMAL_FACE_PX ? 'compact' : 'normal'}
              fit
              upright
            />
          </CardWidthContext.Provider>
        </span>
      ) : null}
    </span>
  )
}

/** An ability on its way to the stack: a chip with its source's name. */
function AbilityChip({ flight }: { flight: Flight }) {
  return (
    <span className="flex size-full items-center gap-1.5 rounded-lg border border-primary bg-background px-2 text-sm font-medium shadow-lg">
      <Zap aria-hidden="true" className="size-4 shrink-0 text-primary" />
      <span className="truncate">{flight.move.look?.name ?? ''}</span>
    </span>
  )
}

/** One flying picture; it starts its animations before the browser paints and takes itself off the layer when it lands. */
function Flier({ flight, travel }: { flight: Flight; travel: Travel }) {
  const root = useRef<HTMLDivElement>(null)
  const face = useRef<HTMLSpanElement>(null)
  useLayoutEffect(() => {
    const element = root.current
    if (element === null || typeof element.animate !== 'function') {
      travel.done(flight.key)
      return undefined
    }
    // An ability's chip has the size its stylesheet gives it; before the animation starts nothing moves it.
    const size = flight.size ?? element.getBoundingClientRect()
    const timing = { duration: flight.duration, delay: flight.delay, easing: EASE_OUT, fill: 'both' } as const
    const bounds = { width: window.innerWidth, height: window.innerHeight }
    const flying = element.animate(keyframesOf(flight, size, bounds), timing)
    flying.onfinish = () => travel.done(flight.key)
    const turning =
      face.current === null || !flight.reveal
        ? null
        : face.current.animate(REVEAL_KEYFRAMES, { ...timing, easing: 'linear' })
    return () => {
      flying.cancel()
      turning?.cancel()
    }
  }, [flight, travel])
  return (
    <div
      ref={root}
      data-fx-flier={flight.move.kind}
      data-chip={flight.size === null ? '' : undefined}
      className="fx-flier"
    >
      {flight.size === null ? (
        <AbilityChip flight={flight} />
      ) : (
        <Picture flight={flight} size={flight.size} face={face} />
      )}
    </div>
  )
}

/**
 * The cards that fly while a batch's moves play: a picture of each card (a card back for one the viewer may not
 * identify, turning face up as it reaches the stack) or a chip for an ability, flying from where it was to where it
 * goes. The layer lies over the table, is `aria-hidden` and takes no pointer, and sits outside the layout: nothing
 * moves because of it. A newer batch ends these flights at once; the table underneath already shows the result.
 */
export function FxTravel() {
  const travel = useFx().travel
  const flights = useFlights()
  if (flights.length === 0) return null
  return (
    <div aria-hidden="true" data-slot="fx-travel" className="fx-travel">
      {flights.map((flight) => (
        <Flier key={flight.key} flight={flight} travel={travel} />
      ))}
    </div>
  )
}
