/** A run of quick life taps: folded into one `Life` action after a quiet moment, with an Undo for the change. */
import { useCallback, useEffect, useRef, useState } from 'react'
import type { PlayerId } from '@/protocol/generated'
import { LIMITS } from '../offers'
import { useManualSender } from './feedback'
import type { Feedback } from './feedback'

/** The quiet, in ms, after the last tap before the run is sent as one action. */
export const QUIET_MS = 600
/** How long, in ms, after the server took a change Undo is offered. */
export const UNDO_MS = 8000
/** The quick changes of the counter, in order. */
export const LIFE_STEPS = [-5, -1, 1, 5] as const

/** A change sent: its size and the life it started from; `undo` marks the change that took another back. */
export interface SentLife {
  delta: number
  from: number
  undo?: true
}

/** What Undo does now: `drop` forgets a change that was not sent yet, `revert` sends the opposite of one the server took. */
export interface Undo {
  kind: 'drop' | 'revert'
  /** The change it takes back. */
  change: number
}

/** The pending change after a tap of `step`, within what one `Life` action may carry. */
export function tapped(pending: number, step: number): number {
  return Math.min(LIMITS.life, Math.max(-LIMITS.life, pending + step))
}

/** A change as a signed figure: "+5", "-5". */
export function signed(delta: number): string {
  return delta > 0 ? `+${delta}` : `${delta}`
}

/** A change in the words of a button and of the feedback line: "Gain 5 life". */
export function lifeWords(delta: number): string {
  return delta > 0 ? `Gain ${delta} life` : `Lose ${-delta} life`
}

/**
 * The life the counter shows: the server's life, plus the change in flight until the server's life has moved (the
 * answer can come before the new state), plus the taps not sent yet.
 */
export function shownLife(life: number, sent: SentLife | null, sending: boolean, pending: number): number {
  const flying = sending && sent !== null && life === sent.from ? sent.delta : 0
  return life + flying + pending
}

/**
 * What Undo does: while taps are pending it drops them (nothing was sent, so the log stays clean); once the server took
 * the change it sends the opposite change, as long as life is still where that change left it. Undoing an undo is no
 * undo: the opposite change is a tap like any other.
 */
export function undoOf(pending: number, sent: SentLife | null, done: boolean, life: number): Undo | null {
  if (pending !== 0) return { kind: 'drop', change: pending }
  if (sent === null || sent.undo === true || !done || life !== sent.from + sent.delta) return null
  return { kind: 'revert', change: sent.delta }
}

/** The state of the counter and what its buttons do. */
export interface LifeRun {
  /** The running total: the life with the changes in flight and pending. */
  total: number
  /** The taps not sent yet, summed. */
  pending: number
  /** The line of the newest sent change. */
  feedback: Feedback | null
  undo: Undo | null
  /** Adds `step` to the run; the run is sent after `QUIET_MS` without a tap. */
  tap(step: number): void
  /** Does what `undo` says. */
  takeBack(): void
}

interface Seat {
  seat: PlayerId
  life: number
  /** Why nothing can be done by hand; left out while the seat plays. */
  off: string | undefined
}

/**
 * The counter of one seat: taps add up to a pending change that is sent as one `Life` action after `QUIET_MS` of quiet
 * (or when the counter goes away). One change is in flight at a time; taps made meanwhile wait for its answer.
 */
export function useLifeRun({ seat, life, off }: Seat): LifeRun {
  const { feedback, send, clear } = useManualSender()
  const [pending, setPending] = useState(0)
  const [sent, setSent] = useState<SentLife | null>(null)
  const lastTap = useRef(0)
  const sending = feedback?.state === 'sending'
  const done = feedback?.state === 'done'

  const push = useCallback(
    (delta: number, undo?: true): void => {
      setSent(undo === true ? { delta, from: life, undo } : { delta, from: life })
      send({ Life: { player: seat, delta } }, lifeWords(delta))
    },
    [life, seat, send],
  )

  useEffect(() => {
    if (pending === 0 || sending || off !== undefined) return
    const wait = Math.max(0, lastTap.current + QUIET_MS - Date.now())
    const timer = window.setTimeout(() => {
      setPending(0)
      push(pending)
    }, wait)
    return () => window.clearTimeout(timer)
  }, [pending, sending, off, push])

  useEffect(() => {
    if (!done) return
    const timer = window.setTimeout(() => setSent(null), UNDO_MS)
    return () => window.clearTimeout(timer)
  }, [feedback, done])

  // A run still pending when the counter goes away (another tab opens) is sent, not lost.
  const latest = useRef({ pending, push })
  useEffect(() => {
    latest.current = { pending, push }
  })
  useEffect(
    () => () => {
      if (latest.current.pending !== 0) latest.current.push(latest.current.pending)
    },
    [],
  )

  const undo = undoOf(pending, sent, done, life)
  return {
    total: shownLife(life, sent, sending, pending),
    pending,
    feedback,
    undo,
    tap: (step) => {
      if (off !== undefined) return
      // The first tap of a run clears the last run's line, and with it the Undo of that run.
      if (pending === 0 && !sending) clear()
      lastTap.current = Date.now()
      setPending((was) => tapped(was, step))
    },
    takeBack: () => {
      if (undo === null) return
      if (undo.kind === 'drop') setPending(0)
      else push(-undo.change, true)
    },
  }
}
