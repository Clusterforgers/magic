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
 * The band that says whose turn has begun: "Your turn" or "Bo's turn" and the turn number. It lies over the middle of
 * the top bar, which holds the same words in the turn badge, so it never covers a card of a battlefield, the prompt,
 * the stack or the controls. It fades in, is held and fades out (250, 600 and 150 ms; a fade only and held 1.5 s under
 * reduced motion) and takes no pointer, so what is under it keeps answering. The band is for the eye (`aria-hidden`); a
 * polite live region says the same in words. It draws only for a batch that changes the turn or the active seat, never
 * at once for a snapshot. Place it inside the top bar, which is `relative`.
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
        className={cn(
          'absolute inset-x-0 inset-y-0.5 mx-auto flex w-fit max-w-[calc(100%-1rem)] items-center justify-center gap-3 rounded-xl px-4 shadow-e2',
          cue?.yours === false ? 'bg-turn-them text-on-turn-them' : 'bg-turn-you text-on-turn-you',
        )}
      >
        <span className="truncate text-lg leading-6 font-semibold">{cue?.who}</span>
        <span className="shrink-0 text-sm font-medium tabular-nums">
          {cue === null ? '' : `Turn ${cue.turn}`}
        </span>
      </div>
    </>
  )
}
