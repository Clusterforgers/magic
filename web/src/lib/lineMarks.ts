/** The line marks of a partly automatic card: which printed lines run automatically and which the player does by hand. */
import { segmentsOf } from '@/features/builder/clauses'
import type { Reason, Support } from '@/protocol/generated'

/** The mark of a line the game runs. */
export const RUNS_AUTOMATICALLY = 'Runs automatically'

/** The mark of a line the player resolves. */
export const YOU_DO_BY_HAND = 'You do this by hand'

/** One line, or one piece of a line cut at a clause; `mark` is null while the clauses cannot be placed in the text. */
export interface MarkedLine {
  text: string
  mark: 'auto' | 'hand' | null
}

/** The marked lines of one face and the clauses that could not be found in its text. */
export interface FaceMarks {
  lines: MarkedLine[]
  /** Reasons of the clauses not found in the text, shown under "You do this by hand". */
  unplaced: string[]
}

/**
 * Marks each line of face `face`: a stretch covered by a clause is "by hand", the rest "automatic", and a printed line
 * with a clause inside is cut at it. When a clause of the face is not found in the text, no line is marked.
 */
export function lineMarks(text: string, reasons: readonly Reason[], face: number): FaceMarks {
  const segments = segmentsOf(text, reasons, face)
  const found = new Set(segments.flatMap((segment) => segment.covers))
  const unplaced = reasons.flatMap((reason, at) =>
    reason.face === face && !found.has(at) ? [reason.text.trim()] : [],
  )
  const lines: MarkedLine[] = []
  for (const segment of segments) {
    const mark = unplaced.length > 0 ? null : segment.covers.length > 0 ? 'hand' : 'auto'
    for (const piece of segment.text.split('\n')) {
      if (piece.trim() !== '') lines.push({ text: piece.trim(), mark })
    }
  }
  return { lines, unplaced }
}

/** The card-level words: what runs and what the player does, from the card's support. */
export function cardWords(support: Support): string {
  switch (support) {
    case 'verified':
    case 'read':
    case 'forge':
      return RUNS_AUTOMATICALLY
    case 'partial':
      return 'Partly automatic'
    case 'manual':
      return YOU_DO_BY_HAND
    case 'stale':
      return `${YOU_DO_BY_HAND} until it is checked again`
    case 'unplayable':
      return 'The game cannot play it'
  }
}
