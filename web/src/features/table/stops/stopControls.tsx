import { createContext, useCallback, useContext, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { useGame, usePlay } from '@/features/game/gameContext'
import type { StopStep } from '@/play/stops'
import type { Step, Stop, StopSide } from '@/protocol/generated'
import { canSetStops } from '../selectors'
import { pressRequest, stopState, turnSide } from './stopModel'

/** What the step ladder and the stops control in the dock share. */
export interface StopControls {
  /** Whether this seat can set stops now. */
  shown: boolean
  online: boolean
  stops: readonly Stop[]
  /** The step the game is in, or null outside the turns. */
  step: Step | null
  /** The side whose turn it is, or null outside the turns. */
  turn: StopSide | null
  /** The side whose stops the ladder shows and sets. */
  showing: StopSide
  /** Whether a plain press stands for a lasting stop. */
  keep: boolean
  /** Whether the step ladder is drawn in the top bar (else in the stops popover of the dock). */
  inBar: boolean
  setKeep: (keep: boolean) => void
  pick: (side: StopSide) => void
  press: (side: StopSide, step: StopStep, shift: boolean) => void
}

const StopControlsContext = createContext<StopControls | null>(null)

/** The shared stops controls, or null where the table has no provider (the ladder then only shows the turn). */
export function useStopControls(): StopControls | null {
  return useContext(StopControlsContext)
}

/**
 * Holds the stop ladder's own state: whether a plain press keeps the stop, and the side picked by hand, which holds
 * until the turn changes hands. The ladder (top bar or popover) and the dock's side switch and Keep toggle read it.
 */
export function StopControlsProvider({ inBar, children }: { inBar: boolean; children: ReactNode }) {
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
  const [picked, setPicked] = useState<{ side: StopSide; turn: StopSide | null } | null>(null)
  // A side picked by hand holds until the turn changes hands, then the ladder follows the turn again.
  if (picked !== null && picked.turn !== turn) setPicked(null)
  const pick = useCallback((side: StopSide) => setPicked({ side, turn }), [turn])
  const press = useCallback(
    (on: StopSide, at: StopStep, shift: boolean): void => {
      const state = stopState(session.store.getState().stops, on, at)
      const request = pressRequest(state, keep !== shift)
      session.setStop(on, at, request.on, request.keep)
    },
    [session, keep],
  )
  const value = useMemo<StopControls>(
    () => ({
      shown,
      online,
      stops,
      step,
      turn,
      showing: picked?.side ?? turn ?? 'Own',
      keep,
      inBar,
      setKeep,
      pick,
      press,
    }),
    [shown, online, stops, step, turn, picked, keep, inBar, pick, press],
  )
  return <StopControlsContext value={value}>{children}</StopControlsContext>
}
