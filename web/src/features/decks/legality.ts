/**
 * The words of a legality warning: a deck that breaks its format's rules is still played, so every page says so as a
 * warning ("Not legal in Commander: 2 banned cards, color identity"), never as a refusal. Pure, so tables test it.
 */
import type { Issue, IssueCode } from '@/protocol/generated'
import { formatLabel } from './format'

/** What a warning says beside its title, wherever a deck that is not legal is shown. */
export const STILL_PLAYS = 'It can still be taken to a game.'

const plural = (count: number, noun: string): string => `${count} ${noun}${count === 1 ? '' : 's'}`

// Codes that name one card each are counted ("2 banned cards"); the rest name one rule of the deck as a whole.
const COUNTED: Partial<Record<IssueCode, readonly [string, string]>> = {
  banned: ['banned card', 'banned cards'],
  not_legal: ['card not in the format', 'cards not in the format'],
  unknown_card: ['unknown card', 'unknown cards'],
  copies: ['card over the copy limit', 'cards over the copy limit'],
}

const RULE: Record<IssueCode, string> = {
  empty: 'no cards',
  unknown_format: 'unknown format',
  unknown_card: 'unknown cards',
  deck_size: 'deck size',
  sideboard_size: 'sideboard size',
  companion_count: 'companion',
  companion_condition: 'companion',
  no_commander_zone: 'commander',
  commander_count: 'commander',
  commander_eligibility: 'commander',
  commander_pair: 'commander',
  copies: 'copies',
  color_identity: 'color identity',
  banned: 'banned cards',
  not_legal: 'cards not in the format',
}

/** The issues grouped by kind in the order they first come: "2 banned cards, color identity, deck size". */
export function issueSummary(issues: readonly Pick<Issue, 'code'>[]): string {
  const counts = new Map<string, { count: number; nouns?: readonly [string, string] }>()
  for (const { code } of issues) {
    const nouns = COUNTED[code]
    const key = nouns?.[1] ?? RULE[code]
    const found = counts.get(key)
    counts.set(key, { count: (found?.count ?? 0) + 1, nouns })
  }
  return [...counts]
    .map(([key, { count, nouns }]) =>
      nouns === undefined ? key : `${count} ${count === 1 ? nouns[0] : nouns[1]}`,
    )
    .join(', ')
}

/** "Not legal in Commander". */
export const notLegalIn = (format: string): string => `Not legal in ${formatLabel(format)}`

/**
 * The one-line warning: "Not legal in Commander: 2 banned cards, color identity" from the issues, or "Not legal in
 * Commander: 2 issues" where only their count is known.
 */
export function notLegalText(format: string, issues: readonly Pick<Issue, 'code'>[] | number): string {
  const detail =
    typeof issues === 'number' ? (issues > 0 ? plural(issues, 'issue') : '') : issueSummary(issues)
  return detail === '' ? notLegalIn(format) : `${notLegalIn(format)}: ${detail}`
}

/** What the table sees of a seat's deck that breaks the table's rules: "Not legal at this table (2)". */
export const tableWarning = (warnings: number): string => `Not legal at this table (${warnings})`
