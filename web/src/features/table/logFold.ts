/** The game log as the panel draws it: a line for every event worth one, and each round of priority passes folded into one line. */
import type { LogEntry } from '@/play/store'
import type { Event, PlayerId, PlayerView } from '@/protocol/generated'
import { commanderDamageLine } from './commander'
import { commanderHits } from './commanderLog'
import { describeEvent, joinWords } from './eventText'
import type { EventNames } from './eventText'
import { byHandLine, manualRun, manualText } from './manual/manualLog'

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
  /** Set for a line that tells something a seat did by hand. */
  manual?: true
}

/** The events that mean something on the stack was passed over: it is about to resolve. */
const RESOLVES = ['ResolutionBegan', 'SpellResolved', 'ResolutionEnded'] as const

const resolves = (event: Event): boolean =>
  typeof event !== 'string' && RESOLVES.some((name) => name in event)

/** What `foldLog` needs to know about the table. */
export interface FoldOptions {
  /** The seats still in the game: a round in which all of them pass is "All pass". */
  seats: readonly PlayerId[]
  /** The players as the view shows them now, whose commander damage names the hits of commanders; none by default. */
  players?: readonly PlayerView[]
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
  const hits = commanderHits(
    log.map((entry) => entry.event),
    options.players ?? [],
  )
  // The events after a manual action that its sentence already tells.
  let told = 0
  log.forEach(({ seq, event, hostBefore }, index) => {
    if (told > 0) {
      told -= 1
      return
    }
    const view = 'Public' in event ? event.Public : null
    if (view !== null && typeof view !== 'string' && 'PriorityGranted' in view) return
    if (view !== null && typeof view !== 'string' && 'PriorityPassed' in view) {
      if (passed.length === 0) first = seq
      if (!passed.includes(view.PriorityPassed.player)) passed.push(view.PriorityPassed.player)
      return
    }
    flush(view ?? undefined)
    const manual = manualRun(log, index)
    if (manual !== null) {
      told = manual.consumed
      lines.push({
        seq,
        text: manualText(event, manual.from, names),
        minor: false,
        passed: [],
        spoken: '',
        manual: true,
      })
      const entered = byHandLine(event, names)
      if (entered !== null) {
        lines.push({ seq: seq + 0.5, text: entered, minor: false, passed: [], spoken: '', manual: true })
      }
      return
    }
    const hit = hits.get(index)
    const line =
      hit === undefined
        ? describeEvent(event, names, hostBefore)
        : {
            text: commanderDamageLine(
              names.card(hit.source),
              hit.amount,
              names.seat(hit.player),
              hit.total,
              hit.lethal,
            ),
            minor: false,
          }
    lines.push({ seq, text: line.text, minor: line.minor, passed: [], spoken: '' })
    const byHand = byHandLine(event, names)
    // Half a step after its event: a line of its own that keeps the keys of the lines unique.
    if (byHand !== null) {
      lines.push({ seq: seq + 0.5, text: byHand, minor: false, passed: [], spoken: '', manual: true })
    }
  })
  flush(undefined)
  return lines
}
