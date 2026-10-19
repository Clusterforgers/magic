import { useEffect, useState } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { Step } from '@/protocol/generated'
import { isYourTurn } from '../selectors'
import { ladderSteps } from '../stops/stopModel'
import { STEP_LABEL } from '../turnText'
import { pipStates } from './ladder'
import type { PipState } from './ladder'
import { SKIPPED_MARK_MS } from './timing'
import { useCueOptions, useTurnFeed } from './useTurnFeed'

// Whole class names, so Tailwind can see them. Shape tells the marks apart as well as colour: filled, dashed, hollow, lit.
const PIP: Record<Exclude<PipState, 'current'>, string> = {
  upcoming: 'border border-border-strong',
  passed: 'border border-step-passed bg-step-passed',
  skipped: 'border-2 border-dashed border-step-skipped',
}
const CURRENT = {
  you: 'border border-turn-you-edge bg-turn-you',
  them: 'border border-turn-them bg-turn-them',
}

/**
 * The turn's steps as a row of small marks in the top bar: the current step lit in the colour of whose turn it is,
 * passed steps filled, steps to come hollow, and a step the game passed without a window dashed for a second before it
 * settles as passed. It is a picture of what the turn badge already says in words, so a screen reader skips it; the
 * dashes come only while the table draws its timed cues. Hidden below 768 px, where the badge has the room.
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
  return (
    <ol
      aria-hidden="true"
      data-slot="step-ladder"
      data-side={yours ? 'you' : 'them'}
      className="hidden shrink-0 items-center gap-1 md:flex"
    >
      {steps.map((name, index) => {
        const state = states[index] ?? 'upcoming'
        return (
          <li
            key={name}
            title={STEP_LABEL[name]}
            data-step={name}
            data-state={state}
            className={cn(
              'h-3 w-5 rounded-[3px]',
              state === 'current' ? CURRENT[yours ? 'you' : 'them'] : PIP[state],
            )}
          />
        )
      })}
    </ol>
  )
}
