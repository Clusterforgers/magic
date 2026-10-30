/** The manual actions sent and not yet answered, in send order: which one a server reply belongs to. */
import type { EventView, PlayerId } from '@/protocol/generated'

/** What a manual action came to: taken, refused with the reason in the server's words, or `lost` with the connection. */
export type ManualResult = { ok: true } | { ok: false; reason: string; lost?: true }

/** Told what became of one manual action, once. */
export type ManualReply = (result: ManualResult) => void

interface Entry {
  /** Where the action sits among everything the session sent that a reply can belong to. */
  order: number
  reply: ManualReply | undefined
}

/**
 * The server answers a manual action with the batch that starts with it or, when it refuses, with one `Error`, and
 * answers in the order it was asked. So the oldest unanswered action owns the next refusal, and the batch that carries
 * a seat's own manual event settles the oldest as taken.
 */
export interface ManualLedger {
  /** Records an action sent at `order`, with whoever wants to hear of its result. */
  add(order: number, reply?: ManualReply): void
  /** The send order of the oldest unanswered action, or null while every action has been answered. */
  oldest(): number | null
  /** Settles the oldest action with `result`; true when it had a listener to tell. */
  settle(result: ManualResult): boolean
  /** Settles every action sent before `order` as taken: the server answered something later, so it had passed them. */
  settleBefore(order: number): void
  /** Settles every action with `result`, such as when the connection is gone. */
  settleAll(result: ManualResult): void
}

/** An empty ledger. */
export function createManualLedger(): ManualLedger {
  let open: Entry[] = []
  return {
    add: (order, reply) => {
      open.push({ order, reply })
    },
    oldest: () => open[0]?.order ?? null,
    settle(result) {
      const first = open.shift()
      first?.reply?.(result)
      return first?.reply !== undefined
    },
    settleBefore(order) {
      const done = open.filter((entry) => entry.order < order)
      open = open.filter((entry) => entry.order >= order)
      for (const entry of done) entry.reply?.({ ok: true })
    },
    settleAll(result) {
      const done = open
      open = []
      for (const entry of done) entry.reply?.(result)
    },
  }
}

/** Whether `event` is the first event of a manual action of `seat`, in its public form or the hidden forms other seats read. */
export function isManualOf(event: EventView, seat: PlayerId): boolean {
  if ('Public' in event) {
    const inner = event.Public
    return typeof inner !== 'string' && 'ManualAction' in inner && inner.ManualAction.player === seat
  }
  const hidden = event.Hidden
  if (typeof hidden === 'string') return false
  if ('ManualMove' in hidden) return hidden.ManualMove.player === seat
  return 'ManualArrange' in hidden && hidden.ManualArrange.player === seat
}
