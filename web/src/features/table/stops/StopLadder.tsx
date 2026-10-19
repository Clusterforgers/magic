import { Pin } from 'lucide-react'
import { memo, useCallback, useState } from 'react'
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
import { StopSideSwitch } from './StopSideSwitch'
import {
  ladderSteps,
  pressRequest,
  rowName,
  stopDescription,
  stopName,
  stopState,
  stopsOn,
  turnSide,
} from './stopModel'
import type { StopsLayout, StopState } from './stopModel'

const SIDES: readonly StopSide[] = ['Own', 'Opponent']

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
        'relative inline-flex size-8 items-center justify-center rounded-md border border-border bg-background text-muted-foreground transition-colors outline-none pointer-coarse:size-11',
        'hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50 aria-disabled:opacity-50',
        'aria-pressed:border-primary aria-pressed:bg-primary aria-pressed:text-primary-foreground',
        'data-current:ring-2 data-current:ring-(--turn-you-edge)',
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
  /** Whether this is the row on show; the other stays in the page, out of sight and out of the tab order. */
  shown: boolean
  layout: StopsLayout
  lasting: boolean
  online: boolean
  onPress: StopButtonProps['onPress']
}

/** The stops of one side as a row of buttons with one tab stop; the arrow keys, Home and End move along it. */
function StopRow({ side, steps, stops, step, turn, shown, layout, lasting, online, onPress }: StopRowProps) {
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
      hidden={!shown}
      className={cn(
        'items-center gap-px rounded-md px-0.5 py-0.5',
        layout === 'row' ? 'flex' : 'grid grid-cols-5 justify-items-center gap-1',
        turn && (side === 'Own' ? 'bg-(--turn-you-edge)/15' : 'bg-muted'),
      )}
      onFocus={onFocus}
      onKeyDown={onKeyDown}
    >
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
 * One row of step icons for the steps that give priority, in turn order, and a switch for whose stops it shows: Mine
 * (your turns) or Theirs (opponents'). The row follows the turn, and a side picked by hand stays until the turn
 * changes hands. A press sets a one-turn stop at that step (and clears one that is held); shift and press, or the Keep
 * toggle, sets a lasting one, which carries a pin. The step the game is in and whose turn it is are marked. The icons
 * show the stops the server holds and change when it says so. Both rows stay in the page, one of them hidden.
 */
export function StopLadder({ layout }: { layout: StopsLayout }) {
  const { session } = useGame()
  const { shown, online, stops, step, turn } = usePlay(
    useShallow((state) => ({
      shown: canSetStops(state),
      online: state.status === 'open',
      stops: state.stops,
      step: state.view?.stage === 'Turns' ? state.view.turn.step : null,
      turn:
        state.view?.stage === 'Turns' && state.seat !== null
          ? turnSide(state.view.turn.active, state.seat)
          : null,
    })),
  )
  const [keep, setKeep] = useState(false)
  // A side picked by hand holds only for the turn it was picked in.
  const [pick, setPick] = useState<{ side: StopSide; turn: StopSide | null } | null>(null)

  const press = useCallback(
    (on: StopSide, at: StopStep, shift: boolean): void => {
      const state = stopState(session.store.getState().stops, on, at)
      const request = pressRequest(state, keep !== shift)
      session.setStop(on, at, request.on, request.keep)
    },
    [session, keep],
  )
  if (!shown) return null

  const showing: StopSide = pick !== null && pick.turn === turn ? pick.side : (turn ?? 'Own')
  const steps = ladderSteps(step, stops)
  return (
    <div
      role="group"
      aria-label="Stops"
      data-slot="stop-ladder"
      data-layout={layout}
      className={cn('flex items-center gap-2', layout === 'row' ? 'flex-nowrap' : 'flex-wrap')}
    >
      <StopSideSwitch
        shown={showing}
        turn={turn}
        counts={{ Own: stopsOn(stops, 'Own'), Opponent: stopsOn(stops, 'Opponent') }}
        onPick={(side) => setPick({ side, turn })}
      />
      <Button
        data-slot="keep-stops"
        size="sm"
        variant="outline"
        className={cn(
          'px-2 pointer-coarse:px-3 aria-pressed:bg-primary aria-pressed:text-primary-foreground',
          layout === 'panel' && 'px-3',
        )}
        aria-pressed={keep}
        aria-label="Keep stops"
        aria-description="While on, pressing a step sets a stop that stays every turn."
        title="Keep stops: while on, pressing a step sets a stop that stays every turn"
        onClick={() => setKeep(!keep)}
      >
        <Pin aria-hidden="true" />
        <span className={layout === 'panel' ? undefined : 'hidden pointer-coarse:inline'}>Keep</span>
      </Button>
      {SIDES.map((of) => (
        <StopRow
          key={of}
          side={of}
          steps={steps}
          stops={stops}
          step={step}
          turn={turn === of}
          shown={showing === of}
          layout={layout}
          lasting={keep}
          online={online}
          onPress={press}
        />
      ))}
    </div>
  )
}
