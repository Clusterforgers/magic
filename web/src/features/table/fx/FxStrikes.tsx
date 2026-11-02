import { useLayoutEffect, useRef } from 'react'
import { useStore } from 'zustand'
import { CardPicture } from './CardPicture'
import { useFx } from './hooks'
import { sourceSelectors } from './strikeLayer'
import type { Striker, Strikes } from './strikeLayer'
import { strikeKeyframes } from './strikes'

/** The striker's own card (or ghost) on the table, hidden while its picture strikes so it is never seen twice. */
function hideSource(striker: Striker): Animation | null {
  for (const selector of sourceSelectors(striker.source)) {
    const element = document.querySelector(selector)
    if (element instanceof HTMLElement && typeof element.animate === 'function')
      return element.animate([{ opacity: 0 }, { opacity: 0 }], {
        duration: striker.delay + striker.duration,
      })
  }
  return null
}

/** One striking picture; it starts before the browser paints and takes itself off the layer when it is back. */
function StrikerPicture({ striker, strikes }: { striker: Striker; strikes: Strikes }) {
  const root = useRef<HTMLDivElement>(null)
  useLayoutEffect(() => {
    const element = root.current
    if (element === null || typeof element.animate !== 'function') {
      strikes.done(striker.key)
      return undefined
    }
    const timing = { duration: striker.duration, delay: striker.delay, fill: 'both' } as const
    const lunge = element.animate(strikeKeyframes(striker.from, striker.to, striker.mutual), timing)
    lunge.onfinish = () => strikes.done(striker.key)
    const hidden = hideSource(striker)
    return () => {
      lunge.cancel()
      hidden?.cancel()
    }
  }, [striker, strikes])
  const { width, height } = striker.from
  return (
    <div ref={root} data-fx-striker={striker.source} className="fx-flier">
      <CardPicture look={striker.look} size={{ width, height }} back={false} />
    </div>
  )
}

/**
 * Combat damage as it happens: each creature that deals it lunges at what it damages and returns, on a layer over the
 * table that is `aria-hidden`, takes no pointer and sits outside the layout. The impact (the flash, the shake and the
 * number) is the target's `damaged` effect, which lands as the strike does. A newer batch ends these strikes at once.
 */
export function FxStrikes() {
  const strikes = useFx().strikes
  const strikers = useStore(strikes.store, (state) => state.strikers)
  if (strikers.length === 0) return null
  return (
    <div aria-hidden="true" data-slot="fx-strikes" className="fx-travel">
      {strikers.map((striker) => (
        <StrikerPicture key={striker.key} striker={striker} strikes={strikes} />
      ))}
    </div>
  )
}
