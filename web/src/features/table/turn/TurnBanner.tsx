import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import type { RefObject } from 'react'
import { useReducedMotion } from '@/hooks/useReducedMotion'
import { cn } from '@/lib/utils'
import type { BannerCue } from './feed'
import { bannerTimes } from './timing'
import { useCueOptions, useTurnFeed } from './useTurnFeed'

type Phase = 'idle' | 'in' | 'hold' | 'out'

interface Shown {
  cue: BannerCue | null
  phase: Phase
}

const NEXT: Record<Exclude<Phase, 'idle'>, Phase> = { in: 'hold', hold: 'out', out: 'idle' }

/** The gap kept between the band and the sheet it hangs on, in px. */
const BAND_GAP_PX = 4

/** The height of an element with its border, followed with a `ResizeObserver`; null where nothing is laid out (a test). */
function useOuterHeight(ref: RefObject<HTMLElement | null>): number | null {
  const [height, setHeight] = useState<number | null>(null)
  useLayoutEffect(() => {
    const element = ref.current
    if (element === null) return undefined
    const measure = (): void => setHeight(element.offsetHeight > 0 ? element.offsetHeight : null)
    measure()
    if (typeof ResizeObserver !== 'function') return undefined
    const observer = new ResizeObserver(measure)
    observer.observe(element)
    return () => observer.disconnect()
  }, [ref])
  return height
}

/**
 * The band that says whose turn has begun: "Your turn" or "Bo's turn" and the turn number. It hangs on the top edge of
 * the shared strip's sheet (`anchor`), so it never lies over the prompt, the stack or the controls, however tall the
 * sheet grows; without a sheet to measure it sits on the strip's top edge. It fades in, is held and fades out (250, 600
 * and 150 ms; a fade only and held 1.5 s under reduced motion) and takes no pointer, so what is under it keeps
 * answering. The band is for the eye (`aria-hidden`); a polite live region says the same in words. It draws only for a
 * batch that changes the turn or the active seat, never at once for a snapshot. Place it inside the strip, which is
 * `relative`; `inset` is how far the sheet starts from the left, which the band keeps to.
 */
export function TurnBanner({
  inset = 0,
  anchor,
}: {
  inset?: number
  anchor?: RefObject<HTMLElement | null>
}) {
  const spare = useRef<HTMLElement>(null)
  const sheet = useOuterHeight(anchor ?? spare)
  const { on, slow } = useCueOptions()
  const reduced = useReducedMotion()
  const [shown, setShown] = useState<Shown>({ cue: null, phase: 'idle' })
  const [announcement, setAnnouncement] = useState('')
  useTurnFeed((feed) => {
    if (feed.banner === null) return
    setShown({ cue: feed.banner, phase: 'in' })
    setAnnouncement(feed.announcement)
  })
  const { phase } = shown
  useEffect(() => {
    if (phase === 'idle') return undefined
    const times = bannerTimes(reduced, slow)
    const wait = { in: times.inMs, hold: times.holdMs, out: times.outMs }[phase]
    // Each part hands on to the next; a new turn sets a new object, which starts the run again from the fade in.
    const timer = setTimeout(() => setShown((was) => ({ ...was, phase: NEXT[phase] })), wait)
    return () => clearTimeout(timer)
  }, [shown, phase, reduced, slow])
  if (!on) return null
  const { cue } = shown
  return (
    <>
      <p data-slot="turn-announcement" aria-live="polite" aria-atomic="true" className="sr-only">
        {announcement}
      </p>
      <div
        aria-hidden="true"
        data-slot="turn-banner"
        data-phase={phase}
        data-your-turn={cue?.yours === true ? true : undefined}
        style={{
          ...(inset === 0 ? null : { left: inset }),
          ...(sheet === null ? null : { bottom: sheet + BAND_GAP_PX }),
        }}
        className={cn(
          'absolute inset-x-0 flex h-9 items-center justify-center gap-3 rounded-xl px-4 shadow-e2',
          sheet === null && 'bottom-full mb-1',
          cue?.yours === false ? 'bg-turn-them text-on-turn-them' : 'bg-turn-you text-on-turn-you',
        )}
      >
        <span className="truncate text-lg font-semibold">{cue?.who}</span>
        <span className="shrink-0 text-sm font-medium tabular-nums">
          {cue === null ? '' : `Turn ${cue.turn}`}
        </span>
      </div>
    </>
  )
}
