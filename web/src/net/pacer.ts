/**
 * Paces outgoing messages under the server's per-socket limit (docs/server.md: a burst of 40, then 20 a second;
 * a socket that goes over is closed with 1008). The budget here is smaller, so a client that answers as fast as it
 * is asked, such as an auto-passing screen or a test bot, is slowed down instead of disconnected.
 */
import type { ClientMessage } from '@/protocol/generated'
import type { Timers } from './timers'

/** Messages that may go out back to back; the server allows 40. */
export const SEND_BURST = 30
/** Messages per second once the burst is spent; the server allows 20. */
export const SEND_PER_SECOND = 15
/** Messages that may wait for their turn; more are refused. */
export const MAX_WAITING = 200

/** A token bucket in front of a socket, with a queue for what does not fit. */
export class Pacer {
  private readonly timers: Timers
  private readonly write: (message: ClientMessage) => boolean
  private tokens = SEND_BURST
  private refilledAt: number
  private queue: ClientMessage[] = []
  private timer: number | null = null

  constructor(timers: Timers, write: (message: ClientMessage) => boolean) {
    this.timers = timers
    this.write = write
    this.refilledAt = timers.now()
  }

  /** Writes `message` now if the budget allows and nobody waits, else queues it; false when it was refused. */
  push(message: ClientMessage): boolean {
    if (this.queue.length === 0 && this.take()) return this.write(message)
    if (this.queue.length >= MAX_WAITING) return false
    this.queue.push(message)
    this.schedule()
    return true
  }

  /** Forgets everything that waits and refills the budget, for a new connection. */
  reset(): void {
    if (this.timer !== null) this.timers.clearTimeout(this.timer)
    this.timer = null
    this.queue = []
    this.tokens = SEND_BURST
    this.refilledAt = this.timers.now()
  }

  private take(): boolean {
    const now = this.timers.now()
    this.tokens = Math.min(SEND_BURST, this.tokens + ((now - this.refilledAt) / 1000) * SEND_PER_SECOND)
    this.refilledAt = now
    if (this.tokens < 1) return false
    this.tokens -= 1
    return true
  }

  private schedule(): void {
    if (this.timer !== null) return
    const wait = Math.max(1, Math.ceil(((1 - this.tokens) / SEND_PER_SECOND) * 1000))
    this.timer = this.timers.setTimeout(() => {
      this.timer = null
      this.drain()
    }, wait)
  }

  private drain(): void {
    while (this.queue.length > 0 && this.take()) {
      const next = this.queue.shift()
      if (next !== undefined) this.write(next)
    }
    if (this.queue.length > 0) this.schedule()
  }
}
