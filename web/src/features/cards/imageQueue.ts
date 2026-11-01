/**
 * A queue that lets at most `limit` pictures load at once, so a list of 200 decks asks the image cache for a few
 * pictures at a time and the rows scrolled into view come first. Pure, so the order and the limit are tested without a
 * page.
 */

/** A place in the queue: waiting for a slot, then holding one until `done`. */
export interface Ticket {
  /** Leaves the queue if still waiting; a ticket that was granted keeps its slot. */
  cancel(): void
  /** Frees the slot (or leaves the queue); safe to call twice. */
  done(): void
}

/** The queue. */
export interface ImageQueue {
  /** Asks for a slot; `granted` runs once one is free (at once when one is), in the order asked. */
  request(granted: () => void): Ticket
  /** Slots held now. */
  readonly active: number
  /** Requests still waiting. */
  readonly waiting: number
}

interface Entry {
  run(): void
  held: boolean
  over: boolean
}

/** Builds a queue of `limit` slots (at least one). */
export function createImageQueue(limit: number): ImageQueue {
  const slots = Math.max(1, Math.floor(limit))
  let active = 0
  const waiting: Entry[] = []

  function leave(entry: Entry): void {
    const at = waiting.indexOf(entry)
    if (at >= 0) waiting.splice(at, 1)
  }

  function pump(): void {
    while (active < slots) {
      const next = waiting.shift()
      if (next === undefined) return
      active += 1
      next.held = true
      next.run()
    }
  }

  return {
    request(granted) {
      const entry: Entry = { run: granted, held: false, over: false }
      waiting.push(entry)
      pump()
      return {
        cancel() {
          if (!entry.held) leave(entry)
        },
        done() {
          if (entry.over) return
          entry.over = true
          if (entry.held) {
            active -= 1
            pump()
          } else {
            leave(entry)
          }
        },
      }
    },
    get active() {
      return active
    },
    get waiting() {
      return waiting.length
    },
  }
}
