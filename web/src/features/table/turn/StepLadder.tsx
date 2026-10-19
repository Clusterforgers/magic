import { useEffect, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { Step } from '@/protocol/generated'
import { isYourTurn } from '../selectors'
import { ladderSteps } from '../stops/stopModel'
import { STEP_LABEL } from '../turnText'
import { phaseRuns, pipStates } from './ladder'
import type { PipState } from './ladder'
import { phaseOf } from './phase'
import { SKIPPED_MARK_MS } from './timing'
import { useCueOptions, useTurnFeed } from './useTurnFeed'

// Whole class names, so Tailwind can see them. Shape tells the marks apart as well as colour: filled, dashed, hollow, lit.
const PIP: Record<Exclude<PipState, 'current'>, string> = {
  upcoming: 'border border-border-strong',
  passed: 'border border-step-passed bg-step-passed',
  skipped: 'border-2 border-dashed border-step-skipped',
}
// The current step takes the phase's colour: solid on your turn, a tint with a heavier edge on another seat's.
const CURRENT = {
  you: 'border border-phase bg-phase',
  them: 'border-2 border-phase bg-phase-soft',
}

/**
 * The turn's steps as a row of small marks in the top bar, in runs of one phase, each run led by its phase's icon: the
 * current step lit in the phase's colour, passed steps filled, steps to come hollow, and a step the game passed
 * without a window dashed for a second before it settles as passed. The current run also names its phase, so the
 * colour has an icon and a name beside it. It is a picture of what the turn badge already says in words, so a screen
 * reader skips it; the dashes come only while the table draws its timed cues. Hidden below 1024 px, where the badge
 * has the room.
 */
export function StepLadder() {
  const { on, slow } = useCueOptions()
  const { stage, step } = usePlay(
    useShallow((state) => ({
      stage: state.view?.stage ?? 'PreGame',
      step: state.view?.turn.step ?? 'Untap',
    })),
  )
  const stops = usePlay((state) => state.stops)
  const yours = usePlay(isYourTurn)
  const [dashed, setDashed] = useState<readonly Step[]>([])
  useTurnFeed((feed) => {
    // A new turn starts its ladder over; inside a turn the steps skipped so far stay dashed together.
    if (feed.banner !== null) setDashed(feed.skipped)
    else if (feed.skipped.length > 0) setDashed((was) => [...new Set([...was, ...feed.skipped])])
  })
  useEffect(() => {
    if (dashed.length === 0) return undefined
    const timer = setTimeout(() => setDashed([]), SKIPPED_MARK_MS * slow)
    return () => clearTimeout(timer)
  }, [dashed, slow])
  if (stage !== 'Turns') return null
  const steps = ladderSteps(step, stops)
  const states = pipStates(steps, step, on ? dashed : [])
  const runs = phaseRuns(steps)
  const phase = phaseOf(step)
  // The run holding the current step; a step the ladder does not draw (untap, cleanup) lights the run of its phase.
  const holding = runs.findIndex((run) => run.steps.some((name) => name === step))
  const here = holding >= 0 ? holding : runs.findIndex((run) => run.phase.id === phase.id)
  // Where each run starts among the marks, which `pipStates` gave in the order of the steps.
  const starts = runs.map((_run, index) =>
    runs.slice(0, index).reduce((sum, run) => sum + run.steps.length, 0),
  )
  return (
    <div
      aria-hidden="true"
      data-slot="step-ladder"
      data-side={yours ? 'you' : 'them'}
      className="hidden shrink-0 items-center gap-2 lg:flex"
    >
      {runs.map((run, index) => {
        const Icon = run.phase.icon
        const lit = index === here
        return (
          <div
            key={`${run.phase.id}-${run.steps[0]}`}
            data-slot="phase-run"
            data-phase-id={run.phase.id}
            data-current={lit || undefined}
            className="flex items-center gap-1"
          >
            <Icon
              data-themed=""
              className={cn('size-4 shrink-0', lit ? 'text-phase' : 'text-muted-foreground')}
            />
            {lit ? <span className="text-xs font-medium text-foreground">{run.phase.name}</span> : null}
            <ol className="flex items-center gap-1">
              {run.steps.map((name, inRun) => {
                const state = states[(starts[index] ?? 0) + inRun] ?? 'upcoming'
                return (
                  <li
                    key={name}
                    title={STEP_LABEL[name]}
                    data-step={name}
                    data-state={state}
                    data-themed=""
                    className={cn(
                      'h-3 w-5 rounded-[3px]',
                      state === 'current' ? CURRENT[yours ? 'you' : 'them'] : PIP[state],
                    )}
                  />
                )
              })}
            </ol>
          </div>
        )
      })}
    </div>
  )
}
