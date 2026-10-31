/** The game log as the panel draws it: a line for every event worth one, and each round of priority passes folded into one line. */
import type { LogEntry } from '@/play/store'
import type { Event, PlayerId, PlayerView } from '@/protocol/generated'
import { commanderDamageLine } from './commander'
import { commanderHits } from './commanderLog'
import { abilitySources, eventContexts } from './eventContext'
import { describeEvent, joinWords } from './eventText'
import type { EventNames } from './eventText'
import { byHandLine, manualRun, manualText, remember } from './manual/manualLog'
import { STEP_LABEL } from './turnText'

/** What a line is: the head of a turn, the name of a step something happened in, or a line of play. */
export type LineKind = 'turn' | 'step' | 'play'

/** One line of the folded log. */
export interface FoldedLine {
  kind: LineKind
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
export function foldLog(log: readonly LogEntry[], given: EventNames, options: FoldOptions): FoldedLine[] {
  const sources = abilitySources(log.map((entry) => entry.event))
  const names: EventNames = { ...given, ability: (id) => given.ability?.(id) ?? sources.get(id) }
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
      kind: 'play',
      passed: run,
      spoken: all ? `${joinWords(who)} pass` : '',
    })
  }
  const hits = commanderHits(
    log.map((entry) => entry.event),
    options.players ?? [],
  )
  const contexts = eventContexts(log.map((entry) => entry.event))
  // The events after a manual action that its sentence already tells.
  let told = 0
  log.forEach(({ seq, event, hostBefore }, index) => {
    if (told > 0) {
      told -= 1
      return
    }
    const view = 'Public' in event ? event.Public : null
    if (view !== null && typeof view !== 'string' && 'PriorityGranted' in view) return
    // A step of a resolution is bookkeeping: the lines of what the step did tell it.
    if (view !== null && typeof view !== 'string' && 'StepDone' in view) return
    if (view !== null && typeof view !== 'string' && 'PriorityPassed' in view) {
      if (passed.length === 0) first = seq
      if (!passed.includes(view.PriorityPassed.player)) passed.push(view.PriorityPassed.player)
      return
    }
    flush(view ?? undefined)
    if (view !== null && typeof view !== 'string' && 'StepBegan' in view) {
      const text = STEP_LABEL[view.StepBegan.step]
      lines.push({ seq, text, minor: true, kind: 'step', passed: [], spoken: '' })
      return
    }
    const manual = manualRun(log, index)
    if (manual !== null) {
      told = manual.consumed
      lines.push({
        seq,
        text: manualText(event, manual.from, remember(names, manual.names), manual.shown, manual.rolled),
        minor: false,
        kind: 'play',
        passed: [],
        spoken: '',
        manual: true,
      })
      const entered = byHandLine(event, names)
      if (entered !== null) {
        lines.push({
          seq: seq + 0.5,
          text: entered,
          minor: false,
          kind: 'play',
          passed: [],
          spoken: '',
          manual: true,
        })
      }
      return
    }
    const hit = hits.get(index)
    const line =
      hit === undefined
        ? describeEvent(event, names, contexts[index], hostBefore)
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
    const turn = view !== null && typeof view !== 'string' && 'TurnBegan' in view
    lines.push({
      seq,
      text: line.text,
      minor: line.minor,
      kind: turn ? 'turn' : 'play',
      passed: [],
      spoken: '',
    })
    const byHand = byHandLine(event, names)
    // Half a step after its event: a line of its own that keeps the keys of the lines unique.
    if (byHand !== null) {
      lines.push({
        seq: seq + 0.5,
        text: byHand,
        minor: false,
        kind: 'play',
        passed: [],
        spoken: '',
        manual: true,
      })
    }
  })
  flush(undefined)
  return withoutEmptySteps(lines)
}

/**
 * `lines` without the steps nothing was read in: a step line stays, as the heading of what follows it, only while a
 * line of play that is not minor comes before the next step or turn. The turn heads say where the game is, so the
 * Upkeep and the End step of a turn in which nothing happened are not lines at all.
 */
export function withoutEmptySteps(lines: readonly FoldedLine[]): FoldedLine[] {
  return lines.filter((line, index) => {
    if (line.kind !== 'step') return true
    for (let at = index + 1; at < lines.length; at += 1) {
      const next = lines[at]
      if (next === undefined || next.kind !== 'play') return false
      if (!next.minor) return true
    }
    return false
  })
}
