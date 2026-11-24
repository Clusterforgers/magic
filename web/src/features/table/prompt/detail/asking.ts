/**
 * What asks the pending decision and where its detail is drawn: the card of the band's chip, the rules text with the
 * asked clause, the run of triggers it is one of, and whether the decision brings the Stack column forward.
 */
import type { PendingDecision } from '@/play/store'
import type { AskingCause, CardIdentity, DecisionKind, ObjectId, RunView } from '@/protocol/generated'

/** The decisions answered on the board, in the hand or in the band alone: the Stack column stays as the viewer left it. */
const NO_DETAIL: ReadonlySet<string> = new Set([
  'Priority',
  'Mulligan',
  'BottomCards',
  'DeclareAttackers',
  'DeclareBlockers',
  'Discard',
])

/** The name of a decision's kind: the key it came with. */
export function kindKey(kind: DecisionKind): string {
  return typeof kind === 'object' && kind !== null ? (Object.keys(kind)[0] ?? '') : ''
}

/** Whether a decision of the kind `name` has detail for the Stack column or the decision sheet. */
export function detailKind(name: string): boolean {
  return name !== '' && !NO_DETAIL.has(name)
}

/** The sheet's one line: who asks, the place in a run, or the decision's name. */
export function sheetTitle(asker: (Asker & { shown: string }) | null, label: string, paying = false): string {
  if (asker === null || asker.shown === '') return label
  if (paying) return `Pay for ${asker.shown}`
  if (asker.run !== undefined) {
    return `${asker.cause?.name ?? asker.shown} · ${asker.run.index} of ${asker.run.entries.length}`
  }
  return `${asker.shown} asks you`
}

/** Whether `pending` has detail for the Stack column or the decision sheet: every decision but those of `NO_DETAIL`. */
export function hasDetail(pending: PendingDecision | null): boolean {
  return pending !== null && !NO_DETAIL.has(kindKey(pending.decision.kind))
}

/** The object a decision names as what asks: its `source`, or the spell of a payment. */
export function namedSource(kind: DecisionKind): ObjectId | undefined {
  const options: unknown = typeof kind === 'object' && kind !== null ? Object.values(kind)[0] : undefined
  if (typeof options !== 'object' || options === null) return undefined
  const { source, spell } = options as { source?: unknown; spell?: unknown }
  if (typeof source === 'number') return source
  return typeof spell === 'number' ? spell : undefined
}

/** What asks a decision, for the band's chip and the column's entry. */
export interface Asker {
  /** The card to draw and to light on the board, while the view may hold it. */
  source: ObjectId | undefined
  /** The spell or ability on the stack that asks. */
  stack: ObjectId | undefined
  /** It is resolving now; false while it is being cast or put on the stack. */
  resolving: boolean
  /** The source's name as the server gave it; empty when it gave none. */
  name: string
  /** The printed card the source last was, for its picture once it has left its zone. */
  identity?: CardIdentity | undefined
  /** The asking ability's rules text. */
  text: string | undefined
  /** The asked clause of `text`, in UTF-16 code units. */
  clause: readonly [number, number] | undefined
  /** What triggered the asking ability. */
  cause: AskingCause | undefined
  /** The run of triggers it is one of. */
  run: RunView | undefined
}

/**
 * What asks `pending`: the server's `asking`, else (a fixture, an older server) the object the decision names. Null for
 * a decision nothing asks.
 */
export function askerOf(pending: PendingDecision | null): Asker | null {
  if (pending === null || !hasDetail(pending)) return null
  const { asking } = pending
  const named = namedSource(pending.decision.kind)
  if (asking === undefined) {
    if (named === undefined) return null
    return {
      source: named,
      stack: undefined,
      resolving: false,
      name: '',
      identity: undefined,
      text: undefined,
      clause: undefined,
      cause: undefined,
      run: undefined,
    }
  }
  return {
    source: asking.source,
    stack: asking.stack,
    resolving: asking.resolving === true,
    name: asking.name ?? '',
    identity: asking.identity,
    text: asking.text,
    clause: asking.clause,
    cause: asking.cause,
    run: asking.run,
  }
}

/** `text` in three pieces around `clause`; the whole text first when there is no clause or it does not fit the text. */
export function clauseParts(
  text: string,
  clause: readonly [number, number] | undefined,
): { before: string; marked: string; after: string } {
  if (clause === undefined) return { before: text, marked: '', after: '' }
  const [start, end] = clause
  if (!(start >= 0 && end > start && end <= text.length)) return { before: text, marked: '', after: '' }
  return { before: text.slice(0, start), marked: text.slice(start, end), after: text.slice(end) }
}

const FOLLOW_UPS = [
  'If you do,',
  "If you don't,",
  "If you can't,",
  'When you do,',
  'If they do,',
  "If they don't,",
]

const sentenceEnd = (text: string, from: number): number => {
  const at = text.indexOf('. ', from)
  return at < 0 ? text.length : at + 1
}

/**
 * The "you may" clause of a card's own text that `question` asks about ("Pay {1}?"), with the sentences that depend on
 * it: for a spell, whose text the server does not send. The same rule as the server's for an ability.
 */
export function mayClause(text: string, question: string): readonly [number, number] | undefined {
  const lower = text.toLowerCase()
  const stem = question.trim().replace(/\?+$/, '').toLowerCase()
  const may = 'you may '
  let start = stem === '' ? -1 : lower.indexOf(stem)
  if (start >= 0 && lower.slice(0, start).endsWith(may)) start -= may.length
  if (start < 0) start = lower.indexOf(may)
  if (start < 0) return undefined
  let end = sentenceEnd(text, start)
  for (;;) {
    const rest = text.slice(end).trimStart()
    if (!FOLLOW_UPS.some((lead) => rest.startsWith(lead))) break
    end = sentenceEnd(text, text.length - rest.length)
  }
  return [start, end]
}

/** "trigger 2 of 3" for a run. */
export function runPlace(run: RunView): string {
  return `trigger ${run.index} of ${run.entries.length}`
}

/** The kicker over the column's entry: "Resolving now · asks you", or the cause and the place in its run. */
export function askKicker(asker: Asker): { lead: string; rest: string } {
  if (asker.run !== undefined) {
    return { lead: asker.cause === undefined ? 'Your triggers' : asker.cause.name, rest: runPlace(asker.run) }
  }
  if (asker.resolving) return { lead: 'Resolving now', rest: 'asks you' }
  return asker.stack === undefined
    ? { lead: 'Asks you', rest: '' }
    : { lead: 'Going on the stack', rest: 'asks you' }
}

/** What a run's row says under its name: a done one its outcome, a waiting one its effect. */
export function runRowWords(
  entry: RunView['entries'][number],
  place: number,
  index: number,
  outcome: string | undefined,
): string {
  if (entry.done === true || place < index - 1) return outcome ?? 'Done'
  const lead = place === index ? 'Next' : 'Then'
  return entry.text === undefined ? lead : `${lead} · ${entry.text}`
}
