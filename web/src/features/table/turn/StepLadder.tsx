import { Pin } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { FocusEvent, KeyboardEvent } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { StopStep } from '@/play/stops'
import type { Step } from '@/protocol/generated'
import { isYourTurn, turnLeft } from '../selectors'
import { useStopControls } from '../stops/stopControls'
import type { StopControls } from '../stops/stopControls'
import { ladderSteps, rowName, stopDescription, stopName, stopState } from '../stops/stopModel'
import type { StopState } from '../stops/stopModel'
import { STEP_LABEL } from '../turnText'
import { rovingTarget } from '../useRoving'
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

interface MarkProps {
  step: StopStep
  state: PipState
  /** The mark's own shape and colour for the turn's progress. */
  className: string
  yours: boolean
}

/** One step's mark: the progress of the turn, alone when the ladder only shows the turn. */
function PlainMark({ step, state, className }: MarkProps) {
  return (
    <li
      title={STEP_LABEL[step]}
      data-step={step}
      data-state={state}
      data-themed=""
      className={cn('h-3 w-5 rounded-[3px]', className)}
    />
  )
}

interface StopMarkProps extends MarkProps {
  controls: StopControls
  hold: StopState
  /** Whether this mark is the ladder's one tab stop. */
  tabbable: boolean
}

/**
 * A step's mark that is also its stop: a press toggles the stop on the side the ladder shows. A held stop is an
 * outline round the mark and a dot under it (a pin for a lasting one); the current step is the mark's fill.
 */
function StopMark({ step, state, className, controls, hold, tabbable }: StopMarkProps) {
  const { showing, keep, online } = controls
  const description = stopDescription(showing, hold, keep)
  return (
    <li
      title={STEP_LABEL[step]}
      data-step={step}
      data-state={state}
      data-hold={hold === 'off' ? undefined : hold}
      data-themed=""
      className={cn(
        'relative h-3 w-5 rounded-[3px] data-hold:outline-2 data-hold:outline-offset-1 data-hold:outline-primary',
        className,
      )}
    >
      <button
        type="button"
        data-slot="stop"
        data-side={showing}
        data-step={step}
        data-keep={hold === 'off' ? undefined : hold}
        data-current={state === 'current' || undefined}
        aria-pressed={hold !== 'off'}
        aria-current={state === 'current' ? 'step' : undefined}
        aria-label={stopName(showing, step, hold, keep)}
        aria-description={description}
        aria-disabled={online ? undefined : true}
        title={description === undefined ? STEP_LABEL[step] : `${STEP_LABEL[step]}. ${description}`}
        tabIndex={tabbable ? 0 : -1}
        className="absolute top-1/2 left-1/2 h-9 w-6 -translate-x-1/2 -translate-y-1/2 rounded-md outline-none hover:bg-foreground/10 focus-visible:ring-3 focus-visible:ring-ring/50 aria-disabled:opacity-50 pointer-coarse:h-11 pointer-coarse:w-7"
        onClick={(event) => {
          if (online) controls.press(showing, step, event.shiftKey)
        }}
      />
      <span
        aria-hidden="true"
        className="pointer-events-none absolute top-full left-1/2 mt-1 flex -translate-x-1/2 items-center"
      >
        {hold === 'lasting' ? (
          <Pin className="size-2.5 text-primary" />
        ) : hold === 'turn' ? (
          <span className="size-1.5 rounded-full bg-primary" />
        ) : null}
      </span>
    </li>
  )
}

/**
 * The turn's steps as a row of small marks, in runs of one phase, each run led by its phase's icon: the current step
 * lit in the phase's colour, passed steps filled, steps to come hollow, and a step the game passed without a window
 * dashed for a second before it settles as passed. The current run also names its phase, so the colour has an icon
 * and a name beside it. For a seat that can set stops the marks are the stop buttons too (`StopMark`): a press
 * toggles the stop at that step on the side the stops control picks, with the arrow keys, Home and End moving along
 * the ladder, which is one tab stop. Otherwise it is a picture of what the turn badge already says in words, so a
 * screen reader skips it. It sits in the top bar from 1024 px (`place` bar) and in the stops popover below that
 * (`panel`), where it wraps.
 */
export function StepLadder({ place = 'bar' }: { place?: 'bar' | 'panel' }) {
  const { on, slow } = useCueOptions()
  const { stage, step } = usePlay(
    useShallow((state) => ({
      stage: state.view?.stage ?? 'PreGame',
      step: state.view?.turn.step ?? 'Untap',
    })),
  )
  const stops = usePlay((state) => state.stops)
  const yours = usePlay(isYourTurn)
  const left = usePlay(turnLeft)
  const controls = useStopControls()
  const [last, setLast] = useState<StopStep | null>(null)
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
  // With a provider the ladder is drawn in one place only; without one it is the top bar's picture.
  if (controls !== null && controls.shown && controls.inBar !== (place === 'bar')) return null
  const interactive = controls !== null && controls.shown
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
  const showing = controls?.showing ?? 'Own'
  const tab = steps.find((each) => each === last) ?? steps.find((each) => each === step) ?? steps[0]

  const onFocus = (event: FocusEvent<HTMLElement>): void => {
    const focused = event.target.closest<HTMLElement>('[data-slot="stop"]')?.dataset['step']
    const found = steps.find((each) => each === focused)
    if (found !== undefined) setLast(found)
  }
  const onKeyDown = (event: KeyboardEvent<HTMLElement>): void => {
    if (event.altKey || event.ctrlKey || event.metaKey) return
    const buttons = [...event.currentTarget.querySelectorAll<HTMLElement>('button[data-slot="stop"]')]
    const index = buttons.findIndex((button) => button === document.activeElement)
    const to = rovingTarget(event.key, Math.max(index, 0), buttons.length)
    if (index < 0 || to === null) return
    event.preventDefault()
    buttons[to]?.focus()
  }

  return (
    <div
      {...(interactive
        ? { role: 'group', 'aria-label': rowName(showing, controls.turn === showing) }
        : { 'aria-hidden': true })}
      data-slot="step-ladder"
      data-place={place}
      data-side={yours ? 'you' : 'them'}
      data-stops-side={interactive ? showing : undefined}
      data-left={left || undefined}
      className={cn(
        'shrink-0 items-center gap-2',
        place === 'panel'
          ? 'flex flex-wrap justify-center gap-y-4 pb-3'
          : interactive
            ? 'flex'
            : 'hidden lg:flex',
      )}
      onFocus={interactive ? onFocus : undefined}
      onKeyDown={interactive ? onKeyDown : undefined}
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
            <ol aria-label={run.phase.name} className="flex items-center gap-1">
              {run.steps.map((name, inRun) => {
                const state = states[(starts[index] ?? 0) + inRun] ?? 'upcoming'
                const className = state === 'current' ? CURRENT[yours ? 'you' : 'them'] : PIP[state]
                const mark = { step: name, state, className, yours }
                return controls !== null && interactive ? (
                  <StopMark
                    key={name}
                    {...mark}
                    controls={controls}
                    hold={stopState(stops, showing, name)}
                    tabbable={name === tab}
                  />
                ) : (
                  <PlainMark key={name} {...mark} />
                )
              })}
            </ol>
          </div>
        )
      })}
    </div>
  )
}
