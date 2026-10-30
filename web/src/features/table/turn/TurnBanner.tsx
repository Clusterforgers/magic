import { useEffect, useRef, useState } from 'react'
import { useReducedMotion } from '@/hooks/useReducedMotion'
import { cn } from '@/lib/utils'
import { useContentBox } from '../useZoneFit'
import { bannerPlace } from './bannerPlace'
import type { BannerCue } from './feed'
import { bannerTimes } from './timing'
import { useCueOptions, useTurnFeed } from './useTurnFeed'

type Phase = 'idle' | 'in' | 'hold' | 'out'

interface Shown {
  cue: BannerCue | null
  phase: Phase
}

const NEXT: Record<Exclude<Phase, 'idle'>, Phase> = { in: 'hold', hold: 'out', out: 'idle' }

/**
/**
 * The band that says whose turn has begun: "Your turn" or "Bo's turn" and the turn number. It stands in the room the
 * top bar leaves free after the step ladder, so it covers neither the ladder nor the turn badge; where that room is too
 * narrow (a phone) it hangs from the bar's lower edge instead (`bannerPlace`). It is the bar's spacer, so the bar does
 * not move when it shows. It fades in, is held and fades out (250, 600 and 150 ms; a fade only and held 1.5 s under
 * reduced motion) and takes no pointer, so what is under it keeps answering. The band is for the eye (`aria-hidden`);
 * a polite live region says the same in words. It draws only for a batch that changes the turn or the active seat,
 * never at once for a snapshot. Place it in the top bar's row, after the step ladder.
 */
export function TurnBanner() {
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
  const room = useRef<HTMLDivElement>(null)
  const { place, turn } = bannerPlace(useContentBox(room)?.width ?? null)
  const { cue } = shown
  const band = (
    <div
      aria-hidden="true"
      data-slot="turn-banner"
      data-phase={phase}
      data-place={place}
      data-your-turn={cue?.yours === true ? true : undefined}
      className={cn(
        'absolute flex max-w-full items-center justify-center gap-3 rounded-xl px-3 shadow-e2',
        place === 'bar'
          ? 'inset-y-0.5 right-0'
          : 'inset-x-0 top-full z-20 mx-auto mt-1 h-8 w-fit max-w-[calc(100%-1rem)]',
        cue?.yours === false ? 'bg-turn-them text-on-turn-them' : 'bg-turn-you text-on-turn-you',
      )}
    >
      <span className="truncate text-lg leading-6 font-semibold">{cue?.who}</span>
      {turn ? (
        <span className="shrink-0 text-sm font-medium tabular-nums">
          {cue === null ? '' : `Turn ${cue.turn}`}
        </span>
      ) : null}
    </div>
  )
  return (
    <>
      {on ? (
        <p data-slot="turn-announcement" aria-live="polite" aria-atomic="true" className="sr-only">
          {announcement}
        </p>
      ) : null}
      {/* The bar's spacer: the free room after the step ladder, whether or not the banner shows. */}
      <div ref={room} data-slot="turn-banner-room" className="relative h-full min-w-px flex-1">
        {on && place === 'bar' ? band : null}
      </div>
      {on && place === 'below' ? band : null}
    </>
  )
}
