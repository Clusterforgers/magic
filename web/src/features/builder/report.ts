/**
 * The deck check as the builder draws it (docs/deck-check.md, "Highlighting"): what the report says of each line, the
 * order issues come in, and the words of a reason or a legality issue. Pure, so tables test it.
 */
import type { DeckReport, Issue, Reason, Support } from '@/protocol/generated'
import type { Line } from './deckEdit'

/** The support states from the worst to the best: what stops a deck, what is done by hand, what is nearly there. */
export const SUPPORT_ORDER: readonly Support[] = ['unplayable', 'manual', 'stale', 'partial', 'verified']

const RANK: Readonly<Record<Support, number>> = {
  unplayable: 0,
  manual: 1,
  stale: 2,
  partial: 3,
  verified: 4,
}

/** True for a state the game cannot play without manual steps. */
export const isIssue = (support: Support): boolean => support !== 'verified'

/** What the check says of one card: how far the game plays it and the clauses it leaves to the table. */
export interface LineCheck {
  support: Support
  reasons: readonly Reason[]
}

/** The report's verdict on each card by oracle id; a card is judged once, in whichever zone it sits. */
export function checksOf(report: DeckReport): ReadonlyMap<string, LineCheck> {
  const checks = new Map<string, LineCheck>()
  for (const card of report.cards) {
    if (!checks.has(card.oracle_id))
      checks.set(card.oracle_id, { support: card.support, reasons: card.reasons })
  }
  return checks
}

/**
 * The check of a line: the report's entry for its card. A card added after the check has none, so it keeps the state it
 * came with from the card search and no reasons until the deck is saved.
 */
export function checkOf(checks: ReadonlyMap<string, LineCheck>, line: Line): LineCheck {
  return checks.get(line.card.oracle_id) ?? { support: line.card.support, reasons: [] }
}

/** How the lines of a zone are ordered: by name, or the cards with issues first (worst first, then by name). */
export type SortKey = 'name' | 'issues'

/** What to show of a zone's lines. */
export interface Arrangement {
  /** Leave out the lines whose card is verified. */
  issuesOnly: boolean
  sort: SortKey
}

/**
 * The lines to show, from `lines` already in name order: the issue lines alone when asked, and with `sort` `issues` the
 * worst states first. Equal states keep their name order.
 */
export function arrange(
  lines: readonly Line[],
  checks: ReadonlyMap<string, LineCheck>,
  { issuesOnly, sort }: Arrangement,
): Line[] {
  const rank = (line: Line): number => RANK[checkOf(checks, line).support]
  const shown = issuesOnly ? lines.filter((line) => rank(line) < RANK.verified) : [...lines]
  return sort === 'issues' ? shown.sort((a, b) => rank(a) - rank(b)) : shown
}

/** A phrase with a full stop at the end unless it ends in one already. */
const stopped = (text: string): string => (/[.!?…]$/.test(text) ? text : `${text}.`)

/** A phrase as a sentence: the first letter capital and a full stop at the end unless it ends in one already. */
export function sentence(text: string): string {
  const trimmed = text.trim()
  if (trimmed === '') return ''
  return stopped(trimmed.charAt(0).toUpperCase() + trimmed.slice(1))
}

const faceName = (face: number): string =>
  face === 0 ? 'Front face' : face === 1 ? 'Back face' : `Face ${face + 1}`

/** One reason as a plain line. */
export interface ReasonLine {
  key: string
  text: string
}

/**
 * The reasons of a card as plain lines, in the order the report gives them. A card with a clause on a face other than the
 * first names the face on every line ("Back face: ..."); a one-face card does not.
 */
export function reasonLines(reasons: readonly Reason[]): ReasonLine[] {
  const named = reasons.some((reason) => reason.face > 0)
  return reasons.map((reason, index) => ({
    key: `${reason.face}:${reason.span.start}-${reason.span.end}:${index}`,
    text: named ? `${faceName(reason.face)}: ${stopped(reason.text.trim())}` : sentence(reason.text),
  }))
}

/** One way the deck breaks its format's rules as a plain line. */
export interface LegalityLine {
  key: string
  text: string
}

/** The legality issues of a report as plain sentences, in the report's order. */
export function legalityLines(issues: readonly Issue[]): LegalityLine[] {
  return issues.map((issue, index) => ({
    key: `${issue.code}:${issue.zone ?? ''}:${issue.oracle_id ?? ''}:${index}`,
    text: sentence(issue.message),
  }))
}
