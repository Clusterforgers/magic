import { createStore } from 'zustand/vanilla'
import type { StoreApi } from 'zustand/vanilla'
import type { Session } from '@/play/session'
import type { PlayState, PlayStore } from '@/play/store'
import type { Answer, StopSide, Step, YieldUntil } from '@/protocol/generated'
import { allowed, defaultHint, narrow } from './gate'
import type { Allow } from './step'

/** What the current step lets through, and what the coach says of anything else. */
export interface Gate {
  /** The step's id, so a refusal belongs to the step it was made at. */
  step: string
  allow: Allow | undefined
  hint: string | undefined
}

/** What the coach hears from the gate. */
export interface CoachState {
  /** The gate of the step on show; null once the learner plays on without the coach. */
  gate: Gate | null
  /** The last input held back: its reason and when, so the coach shakes once per refusal. */
  refused: { reason: string; at: number } | null
}

/** A session with a lesson's gate in front of it. */
export interface LessonSession extends Session {
  /** The gate and the refusals, for the coach. */
  readonly coach: StoreApi<CoachState>
  /** Sets the gate of the step on show; null drops the gate (play on without the coach). */
  setGate(gate: Gate | null): void
}

/** `state` as the table draws it under `gate`: the pending decision narrowed to what the step asks for. */
export function narrowedState(state: PlayState, gate: Gate | null): PlayState {
  if (gate === null || state.pending === null) return state
  const decision = narrow(state.pending.decision, gate.allow, state.objectsById)
  if (decision === state.pending.decision) return state
  return { ...state, pending: { ...state.pending, decision } }
}

/**
 * Wraps `inner` for a lesson: an answer passes only when the current step's `allow` accepts it, everything else that
 * changes the game outside a decision is held unless the step allows it (manual actions, Concede, Full Control, stops,
 * yields, automatic choices, take-back), and the store the table reads has the pending decision narrowed. Preview,
 * chat and resync pass. The server still checks every answer; the gate is a teaching aid of the client.
 */
export function lessonSession(inner: Session, now: () => number = Date.now): LessonSession {
  const coach = createStore<CoachState>(() => ({ gate: null, refused: null }))
  const base = inner.store
  let current = narrowedState(base.getState(), null)
  const listeners = new Set<(state: PlayState, previous: PlayState) => void>()
  const refresh = (): void => {
    const previous = current
    current = narrowedState(base.getState(), coach.getState().gate)
    if (current !== previous) for (const listener of listeners) listener(current, previous)
  }
  base.subscribe(refresh)
  coach.subscribe((state, previous) => {
    if (state.gate !== previous.gate) refresh()
  })
  const store: PlayStore = {
    getState: () => current,
    getInitialState: () => base.getInitialState(),
    setState: base.setState,
    subscribe: (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
  }

  const gate = (): Gate | null => coach.getState().gate
  const refuse = (): false => {
    const at = gate()
    coach.setState({ refused: { reason: at?.hint ?? defaultHint(at?.allow), at: now() } })
    return false
  }
  /** Runs `send` when the coach is gone or `pass` says the step allows it; refuses otherwise. */
  const held =
    <A extends unknown[]>(
      send: (...args: A) => boolean,
      pass: (allow: Allow | undefined, ...args: A) => boolean = () => false,
    ) =>
    (...args: A): boolean => {
      const at = gate()
      if (at === null || pass(at.allow, ...args)) return send(...args)
      return refuse()
    }

  return {
    ...inner,
    store,
    coach,
    setGate(next) {
      coach.setState({ gate: next, refused: null })
    },
    answer: held(
      (answer: Answer) => inner.answer(answer),
      (allow, answer) => allowed(allow, answer, base.getState().objectsById),
    ),
    concede: held(() => inner.concede()),
    setStop: held(
      (side: StopSide, step: Step, on: boolean, keep: boolean) => inner.setStop(side, step, on, keep),
      (allow, side, step, on) => allow?.kind === 'stop' && on && allow.side === side && allow.step === step,
    ),
    setFullControl: held((...args: Parameters<Session['setFullControl']>) => inner.setFullControl(...args)),
    setEndTurnByHand: held((on: boolean) => inner.setEndTurnByHand(on)),
    setAutoChoices: held((...args: Parameters<Session['setAutoChoices']>) => inner.setAutoChoices(...args)),
    autoYield: held((...args: Parameters<Session['autoYield']>) => inner.autoYield(...args)),
    clearAutoYield: held((...args: Parameters<Session['clearAutoYield']>) => inner.clearAutoYield(...args)),
    manual: held(
      (...args: Parameters<Session['manual']>) => inner.manual(...args),
      (allow) => allow?.kind === 'manual',
    ),
    autoSkip: held((...args: Parameters<Session['autoSkip']>) => inner.autoSkip(...args)),
    cancelAutoSkip: held((...args: Parameters<Session['cancelAutoSkip']>) => inner.cancelAutoSkip(...args)),
    castWhenAsked: held((...args: Parameters<Session['castWhenAsked']>) => inner.castWhenAsked(...args)),
    yieldUntil: held(
      (until: YieldUntil) => inner.yieldUntil(until),
      (allow, until) => allow?.kind === 'endTurn' && until === 'EndOfTurn',
    ),
    takeBack: held(() => inner.takeBack()),
    voteTakeBack: held((approve: boolean) => inner.voteTakeBack(approve)),
    goBack: held(() => inner.goBack()),
    voteGoBack: held((approve: boolean) => inner.voteGoBack(approve)),
  }
}
