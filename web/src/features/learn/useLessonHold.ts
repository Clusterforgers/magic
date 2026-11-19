import { useStore } from 'zustand'
import { createStore } from 'zustand/vanilla'
import { useGame } from '@/features/game/gameContext'
import type { CoachState } from './lessonSession'

/** The coach state of a game that is no lesson: no gate, nothing held. */
const NO_COACH = createStore<CoachState>(() => ({ gate: null, refused: null }))

/** Whether a lesson holds passing back now: its gate is up and the step asks for another input. */
export function usePassHeld(): boolean {
  const { session } = useGame()
  const coach = 'coach' in session ? (session.coach as typeof NO_COACH) : NO_COACH
  return useStore(coach, (state) => state.gate !== null && state.gate.allow?.kind !== 'pass')
}
