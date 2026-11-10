// Goal: a legality warning words every kind of issue ("Not legal in Commander: 2 banned cards, color identity"), counts the cards of a per-card issue and names a whole-deck rule once, and the table's count reads the same for every seat.
import { describe, expect, it } from 'vitest'
import type { IssueCode } from '@/protocol/generated'
import { issueSummary, notLegalText, tableWarning } from './legality'

const issues = (...codes: IssueCode[]) => codes.map((code) => ({ code }))

describe('notLegalText', () => {
  it.each<[string, string, ReturnType<typeof issues> | number, string]>([
    ['a banned card', 'commander', issues('banned'), 'Not legal in Commander: 1 banned card'],
    [
      'banned cards and an off-identity card',
      'commander',
      issues('banned', 'color_identity', 'banned'),
      'Not legal in Commander: 2 banned cards, color identity',
    ],
    ['a wrong size', 'modern', issues('deck_size'), 'Not legal in Modern: deck size'],
    ['an unknown card', 'freeform', issues('unknown_card'), 'Not legal in Freeform: 1 unknown card'],
    ['a count only', 'commander', 3, 'Not legal in Commander: 3 issues'],
    ['no detail', 'commander', 0, 'Not legal in Commander'],
  ])('words %s', (_, format, found, words) => {
    expect(notLegalText(format, found)).toBe(words)
  })

  it('names a rule of the whole deck once however often it breaks', () => {
    expect(issueSummary(issues('commander_count', 'commander_pair'))).toBe('commander')
  })

  it('tells the table how many rules a seat breaks', () => {
    expect(tableWarning(2)).toBe('Not legal at this table (2)')
  })
})
