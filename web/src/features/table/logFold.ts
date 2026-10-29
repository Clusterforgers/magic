/** The game log as the panel draws it: a line for every event worth one, and each round of priority passes folded into one line. */
import type { LogEntry } from '@/play/store'
import type { Event, PlayerId } from '@/protocol/generated'
import { describeEvent, joinWords } from './eventText'
import type { EventNames } from './eventText'

/** One line of the folded log. */
export interface FoldedLine {
  /** The position in the server's log of the event the line stands for (the first of a fold). */
  seq: number
  text: string
  /** Drawn quietly. */
  minor: boolean
  /** The seats that passed, in order, for a folded line of passes; empty for every other line. */
  passed: readonly PlayerId[]
  /** For a folded line of passes: the words that name the seats, which the line itself leaves to their colour dots. */
  spoken: string
}

/** The events that mean something on the stack was passed over: it is about to resolve. */
const RESOLVES = ['ResolutionBegan', 'SpellResolved', 'ResolutionEnded'] as const

const resolves = (event: Event): boolean =>
  typeof event !== 'string' && RESOLVES.some((name) => name in event)

/** What `foldLog` needs to know about the table. */
export interface FoldOptions {
  /** The seats still in the game: a round in which all of them pass is "All pass". */
  seats: readonly PlayerId[]
}

/**
 * The lines of a log: one sentence for every event, except that a seat receiving priority says nothing and the passes
 * of a round of priority are one line naming the seats ("All pass", or "Bo and Cy pass" when an action cut the round
 * short). A round in which everybody passed with nothing to resolve (a step that went by) is left out, since the next
 * step says it; so is a round that has no event after it yet, which waits for that event to say what it was.
 */
export function foldLog(log: readonly LogEntry[], names: EventNames, options: FoldOptions): FoldedLine[] {
  const lines: FoldedLine[] = []
  let passed: PlayerId[] = []
  let first = 0
  const flush = (next: Event | undefined): void => {
    const run = passed
    passed = []
    if (run.length === 0) return
    const all = options.seats.every((seat) => run.includes(seat))
    if (all && (next === undefined || !resolves(next))) return
    const who = run.map((seat) => names.seat(seat))
    lines.push({
      seq: first,
      text: all ? 'All pass' : `${joinWords(who)} ${who.length === 1 ? 'passes' : 'pass'}`,
      minor: true,
      passed: run,
      spoken: all ? `${joinWords(who)} pass` : '',
    })
  }
  for (const { seq, event } of log) {
    const view = 'Public' in event ? event.Public : null
    if (view !== null && typeof view !== 'string' && 'PriorityGranted' in view) continue
    if (view !== null && typeof view !== 'string' && 'PriorityPassed' in view) {
      if (passed.length === 0) first = seq
      if (!passed.includes(view.PriorityPassed.player)) passed.push(view.PriorityPassed.player)
      continue
    }
    flush(view ?? undefined)
    const line = describeEvent(event, names)
    lines.push({ seq, text: line.text, minor: line.minor, passed: [], spoken: '' })
  }
  flush(undefined)
  return lines
}
