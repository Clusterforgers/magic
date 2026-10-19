/**
 * A face's oracle text split at the spans of the clauses the game cannot run, so a preview can underline them
 * (docs/deck-check.md, "Highlighting"). Spans count Unicode scalar values, which the text is indexed by here too, so a
 * character outside the Basic Multilingual Plane is one position. Pure, so tables test it.
 */
import type { Reason } from '@/protocol/generated'

/** A stretch of oracle text and the clauses that cover it. */
export interface Segment {
  text: string
  /** The positions, in the reasons given, of the clauses that cover this stretch; none for plain text. */
  covers: readonly number[]
  /** The positions of the clauses whose span ends with this stretch, where their note markers go. */
  ends: readonly number[]
}

interface Clause {
  index: number
  start: number
  end: number
}

/**
 * The text of face `face` cut into stretches: each is plain or covered by one or more clauses of that face. A clause is
 * named by its position in `reasons`, so the numbers stay those of the whole card. A span is cut back to the text, and
 * one that is empty, reversed or wholly past the end covers nothing. The stretches join into `text`; an empty text
 * has none.
 */
export function segmentsOf(text: string, reasons: readonly Reason[], face: number): Segment[] {
  const chars = Array.from(text)
  const length = chars.length
  const within = (position: number): number =>
    Math.min(Math.max(Number.isFinite(position) ? Math.trunc(position) : 0, 0), length)
  const clauses: Clause[] = []
  reasons.forEach((reason, index) => {
    if (reason.face !== face) return
    const start = within(reason.span.start)
    const end = within(reason.span.end)
    if (end > start) clauses.push({ index, start, end })
  })
  const cuts = [...new Set([0, length, ...clauses.flatMap((clause) => [clause.start, clause.end])])].sort(
    (a, b) => a - b,
  )
  const segments: Segment[] = []
  let from = 0
  for (const to of cuts) {
    if (to > from) {
      segments.push({
        text: chars.slice(from, to).join(''),
        covers: clauses
          .filter((clause) => clause.start <= from && to <= clause.end)
          .map((clause) => clause.index),
        ends: clauses.filter((clause) => clause.end === to).map((clause) => clause.index),
      })
    }
    from = to
  }
  return segments
}
