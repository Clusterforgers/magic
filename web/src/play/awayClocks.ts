/** The clocks of the seats that are away: how long before the game acts for each, ticked at every whole second. */
import type { Timers } from '@/net/timers'

/** Per seat that is away, milliseconds left at the last tick before the game acts for it; null when it has no clock. */
export type AwayTimes = Readonly<Record<number, number | null>>

/** The away clocks of one session. */
export interface AwayClocks {
  /** A seat's presence changed or its away clock was told (`ServerMessage` `Presence`). */
  presence(seat: number, connected: boolean, deadlineMs: number | undefined): void
  /** Forgets every clock: a snapshot says afresh who is away, and the times follow it. */
  clear(): void
}

/** Clocks that report every change of the whole seconds left to `onChange`; a clock that reaches zero stays at 0. */
export function createAwayClocks(timers: Timers, onChange: (away: AwayTimes) => void): AwayClocks {
  const endsAt = new Map<number, number | null>()
  let timer: number | null = null

  function tick(): void {
    timer = null
    const now = timers.now()
    const away: Record<number, number | null> = {}
    let next = Infinity
    for (const [seat, end] of endsAt) {
      const left = end === null ? null : Math.max(0, end - now)
      away[seat] = left
      // Wake when the whole seconds left of some clock change, as the response clock does.
      if (left !== null && left > 0) next = Math.min(next, left - 1000 * (Math.ceil(left / 1000) - 1))
    }
    onChange(away)
    if (next !== Infinity) timer = timers.setTimeout(tick, next)
  }

  function restart(): void {
    if (timer !== null) timers.clearTimeout(timer)
    tick()
  }

  return {
    presence(seat, connected, deadlineMs) {
      if (connected) endsAt.delete(seat)
      else endsAt.set(seat, deadlineMs === undefined ? null : timers.now() + deadlineMs)
      restart()
    },
    clear() {
      if (endsAt.size === 0) return
      endsAt.clear()
      restart()
    },
  }
}
