import { useEffect, useState } from 'react'
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

/**
 * The band over the shared strip that says whose turn has begun: "Your turn" or "Bo's turn" and the turn number. It
 * fades in, is held and fades out (250, 600 and 150 ms; a fade only and held 1.5 s under reduced motion) and takes no
 * pointer, so the strip under it keeps answering. The band is for the eye (`aria-hidden`); a polite live region says
 * the same in words. It draws only for a batch that changes the turn or the active seat, never at once for a snapshot.
 * Place it inside the strip, which is `relative`; `inset` is how far the strip's sheet starts from the left, which the
 * band keeps to so that it lies exactly over the sheet.
 */
export function TurnBanner({ inset = 0 }: { inset?: number }) {
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
        style={inset === 0 ? undefined : { left: inset }}
        className={cn(
          'absolute inset-0 flex items-center justify-center gap-3 rounded-xl px-4 shadow-e2',
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
