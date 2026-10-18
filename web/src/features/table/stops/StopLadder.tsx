import { ChevronDown, Pin } from 'lucide-react'
import { memo, useCallback, useId, useState } from 'react'
import type { FocusEvent, KeyboardEvent } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { Button } from '@/components/ui/button'
import { useGame, usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { StopStep } from '@/play/stops'
import type { Step, Stop, StopSide } from '@/protocol/generated'
import { canSetStops } from '../selectors'
import { STEP_LABEL } from '../turnText'
import { rovingTarget } from '../useRoving'
import { StopIcon } from './StopIcon'
import {
  ladderSteps,
  pressRequest,
  rowName,
  stopCount,
  stopDescription,
  stopName,
  stopState,
  turnSide,
} from './stopModel'
import type { StopState } from './stopModel'

interface StopButtonProps {
  side: StopSide
  step: StopStep
  state: StopState
  /** Whether this is the step the game is in, on this side's turn. */
  current: boolean
  /** Whether a plain press stands for a lasting stop (the Keep toggle is on). */
  lasting: boolean
  online: boolean
  /** Whether this button is the row's one tab stop. */
  tabbable: boolean
  onPress: (side: StopSide, step: StopStep, shift: boolean) => void
}

/** One step of one side: lit when a stop is held, pinned when it lasts, ringed when it is the current step. */
const StopButton = memo(function StopButton({
  side,
  step,
  state,
  current,
  lasting,
  online,
  tabbable,
  onPress,
}: StopButtonProps) {
  const description = stopDescription(side, state, lasting)
  return (
    <button
      type="button"
      data-slot="stop"
      data-side={side}
      data-step={step}
      data-keep={state === 'off' ? undefined : state}
      data-current={current || undefined}
      aria-pressed={state !== 'off'}
      aria-current={current ? 'step' : undefined}
      aria-label={stopName(side, step, state, lasting)}
      aria-description={description}
      aria-disabled={online ? undefined : true}
      title={description === undefined ? STEP_LABEL[step] : `${STEP_LABEL[step]}. ${description}`}
      tabIndex={tabbable ? 0 : -1}
      className={cn(
        'relative inline-flex size-7 items-center justify-center rounded-md border border-border bg-background text-muted-foreground transition-colors outline-none',
        'hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50 aria-disabled:opacity-50',
        'aria-pressed:border-primary aria-pressed:bg-primary aria-pressed:text-primary-foreground',
        'data-current:ring-2 data-current:ring-(--your-turn)',
      )}
      onClick={(event) => {
        if (online) onPress(side, step, event.shiftKey)
      }}
    >
      <StopIcon step={step} />
      {state === 'lasting' ? (
        <Pin
          aria-hidden="true"
          className="absolute -top-1.5 -right-1.5 size-3.5 rounded-full bg-background p-0.5 text-foreground ring-1 ring-border"
        />
      ) : null}
    </button>
  )
})

interface StopRowProps {
  side: StopSide
  steps: readonly StopStep[]
  stops: readonly Stop[]
  /** The step the game is in, or null outside the turns. */
  step: Step | null
  /** Whether it is this side's turn now. */
  turn: boolean
  lasting: boolean
  online: boolean
  onPress: StopButtonProps['onPress']
}

/** The stops of one side as a row of buttons with one tab stop; the arrow keys, Home and End move along it. */
function StopRow({ side, steps, stops, step, turn, lasting, online, onPress }: StopRowProps) {
  const [last, setLast] = useState<StopStep | null>(null)
  const fallback = turn ? steps.find((each) => each === step) : undefined
  const tab = steps.find((each) => each === last) ?? fallback ?? steps[0]

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
      role="group"
      aria-label={rowName(side, turn)}
      data-slot="stop-row"
      data-side={side}
      data-turn={turn || undefined}
      className={cn(
        'flex flex-wrap items-center gap-x-1 gap-y-0.5 rounded-md px-1 py-0.5',
        turn && (side === 'Own' ? 'bg-(--your-turn)/15' : 'bg-muted'),
      )}
      onFocus={onFocus}
      onKeyDown={onKeyDown}
    >
      <span
        aria-hidden="true"
        className="flex w-20 shrink-0 items-center gap-1 text-[0.7rem] font-semibold tracking-wide text-muted-foreground uppercase max-sm:basis-full"
      >
        {side === 'Own' ? 'Yours' : 'Theirs'}
        {turn ? (
          <span className="rounded bg-foreground px-1 text-[0.6rem] leading-4 text-background">turn</span>
        ) : null}
      </span>
      {steps.map((each) => (
        <StopButton
          key={each}
          side={side}
          step={each}
          state={stopState(stops, side, each)}
          current={turn && each === step}
          lasting={lasting}
          online={online}
          tabbable={each === tab}
          onPress={onPress}
        />
      ))}
    </div>
  )
}

/**
 * Two rows of step icons, yours and theirs, for the steps that give priority in turn order. A press sets a one-turn
 * stop at that step (and clears one that is held); shift and press, or the Keep toggle, sets a lasting one, which
 * carries a pin. The step the game is in and whose turn it is are marked. The icons show the stops the server holds
 * and change when it says so. Where the screen is narrow the rows fold into one line that counts the stops.
 */
export function StopLadder() {
  const { session } = useGame()
  const { shown, online, stops, step, side } = usePlay(
    useShallow((state) => ({
      shown: canSetStops(state),
      online: state.status === 'open',
      stops: state.stops,
      step: state.view?.stage === 'Turns' ? state.view.turn.step : null,
      side:
        state.view?.stage === 'Turns' && state.seat !== null
          ? turnSide(state.view.turn.active, state.seat)
          : null,
    })),
  )
  const [keep, setKeep] = useState(false)
  const [open, setOpen] = useState(false)
  const panel = useId()

  const press = useCallback(
    (on: StopSide, at: StopStep, shift: boolean): void => {
      const state = stopState(session.store.getState().stops, on, at)
      const request = pressRequest(state, keep !== shift)
      session.setStop(on, at, request.on, request.keep)
    },
    [session, keep],
  )
  if (!shown) return null

  const steps = ladderSteps(step, stops)
  const row = (of: StopSide) => (
    <StopRow
      side={of}
      steps={steps}
      stops={stops}
      step={step}
      turn={side === of}
      lasting={keep}
      online={online}
      onPress={press}
    />
  )
  return (
    <div
      role="group"
      aria-label="Stops"
      data-slot="stop-ladder"
      data-open={open || undefined}
      className="flex flex-wrap items-center gap-2"
    >
      <Button
        data-slot="stop-summary"
        size="sm"
        variant="outline"
        className="sm:hidden"
        aria-expanded={open}
        aria-controls={panel}
        onClick={() => setOpen(!open)}
      >
        Stops · {stopCount(stops.length)}
        <ChevronDown aria-hidden="true" className={cn('transition-transform', open && 'rotate-180')} />
      </Button>
      <div id={panel} className={cn('flex flex-wrap items-center gap-2', !open && 'max-sm:hidden')}>
        <div className="grid gap-0.5">
          {row('Own')}
          {row('Opponent')}
        </div>
        <Button
          data-slot="keep-stops"
          size="sm"
          variant="outline"
          aria-pressed={keep}
          aria-label="Keep stops"
          aria-description="While on, pressing a step sets a stop that stays every turn."
          title="Keep stops: while on, pressing a step sets a stop that stays every turn"
          className="aria-pressed:bg-primary aria-pressed:text-primary-foreground"
          onClick={() => setKeep(!keep)}
        >
          <Pin aria-hidden="true" />
          Keep
        </Button>
      </div>
    </div>
  )
}
