/**
 * The band that says a boss's phase has begun ("Skrix · Phase II · The Nest Stirs"), hung from the top bar's lower
 * edge. It keeps the turn banner's times (`bannerTimes`), takes no pointer and is for the eye (`aria-hidden`); a
 * polite live region says the same in words. It draws only when a batch raises a boss seat's `begun`, never for a
 * snapshot, and reads the public view only, so every seat sees the same.
 */
import { CrownIcon } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useGame } from '@/features/game/gameContext'
import { useReducedMotion } from '@/hooks/useReducedMotion'
import { batchBetween } from '../fx/batch'
import { useSeatNamer } from '../names'
import { bannerTimes } from '../turn/timing'
import { useCueOptions } from '../turn/useTurnFeed'
import { phaseCueBetween } from './bossPhase'
import type { PhaseCue } from './bossPhase'

type Phase = 'idle' | 'in' | 'hold' | 'out'

const NEXT: Record<Exclude<Phase, 'idle'>, Phase> = { in: 'hold', hold: 'out', out: 'idle' }

export function PhaseBanner() {
  const { on, slow } = useCueOptions()
  const reduced = useReducedMotion()
  const { session } = useGame()
  const name = useSeatNamer()
  const latest = useRef(name)
  useEffect(() => {
    latest.current = name
  })
  const [shown, setShown] = useState<{ cue: PhaseCue | null; phase: Phase }>({ cue: null, phase: 'idle' })
  const [announcement, setAnnouncement] = useState('')
  useEffect(() => {
    if (!on) return undefined
    return session.store.subscribe((next, prev) => {
      // Only a batch: a snapshot (a reload, a gap) moves the phase without the moment it began.
      const step = batchBetween(prev, next)
      if (step === null) return
      const cue = phaseCueBetween(step.before.view.players, step.after.view.players, latest.current)
      if (cue === null) return
      setShown({ cue, phase: 'in' })
      setAnnouncement(cue.announcement)
    })
  }, [on, session])
  const { phase, cue } = shown
  useEffect(() => {
    if (phase === 'idle') return undefined
    const times = bannerTimes(reduced, slow)
    const wait = { in: times.inMs, hold: times.holdMs, out: times.outMs }[phase]
    const timer = setTimeout(() => setShown((was) => ({ ...was, phase: NEXT[phase] })), wait)
    return () => clearTimeout(timer)
  }, [shown, phase, reduced, slow])
  if (!on) return null
  return (
    <>
      <p data-slot="phase-announcement" aria-live="polite" aria-atomic="true" className="sr-only">
        {announcement}
      </p>
      {cue === null || phase === 'idle' ? null : (
        <div
          aria-hidden="true"
          data-slot="phase-banner"
          data-phase={phase}
          className="absolute inset-x-0 top-full z-20 mx-auto mt-1 flex w-fit max-w-[calc(100%-1rem)] items-center gap-2 rounded-xl border-2 border-boss bg-raised px-3 py-1 text-foreground shadow-e2"
        >
          <CrownIcon className="size-5 shrink-0 text-boss" />
          <span className="truncate text-sm font-medium text-muted-foreground">{cue.who}</span>
          <span className="shrink-0 text-lg leading-6 font-semibold text-boss">{cue.label}</span>
          {cue.title === '' ? null : (
            <span className="truncate text-lg leading-6 font-semibold">{cue.title}</span>
          )}
        </div>
      )}
    </>
  )
}
