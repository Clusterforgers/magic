// Goal: the verdict of a deck at a table separates a deck that runs whole, one that runs on reads, one with cards played by hand (with the cards still waiting for a read) and one the table cannot take, and word each in the badge's state and label.
import { describe, expect, it } from 'vitest'
import { legalityIssue, reportCard, reportOf } from '@/features/decks/fakeDecks'
import type { ReportCard } from '@/protocol/generated'
import { verdictLabel, verdictOf, verdictState } from './verdict'
import type { Verdict } from './verdict'

const card = (name: string, over: Partial<ReportCard> = {}): ReportCard => reportCard(name, over)

describe('verdictOf', () => {
  it.each<[string, ReportCard[], Verdict]>([
    ['a deck that runs whole', [card('Forest', { count: 60 })], { kind: 'runs' }],
    [
      'a deck with approved reads only',
      [card('Forest', { count: 56 }), card('Odd Card', { count: 4, support: 'read', draft: 'approved' })],
      { kind: 'reads', cards: 4 },
    ],
    [
      'a card nobody has read beside approved ones',
      [
        card('Forest', { count: 54 }),
        card('Read Card', { count: 4, support: 'read', draft: 'approved' }),
        card('Waiting', { count: 2, support: 'manual', draft: 'pending' }),
      ],
      { kind: 'hand', cards: 2, names: ['Waiting'], toRead: 1 },
    ],
    [
      'a wrong draft and a card with no draft are played by hand but not to read',
      [card('Wrong', { support: 'manual', draft: 'wrong' }), card('Plain', { support: 'partial' })],
      { kind: 'hand', cards: 2, names: ['Wrong', 'Plain'], toRead: 0 },
    ],
  ])('says %s', (_what, cards, verdict) => {
    expect(verdictOf(reportOf(cards, [], { format: 'commander' }))).toEqual(verdict)
  })

  it('blocks on an issue or an unplayable card, whatever the reads say', () => {
    const issue = reportOf(
      [card('Read Card', { support: 'read', draft: 'approved' })],
      [legalityIssue('Too small')],
    )
    expect(verdictOf(issue)).toMatchObject({ kind: 'blocked' })
    const unplayable = reportOf([card('Odd', { support: 'unplayable' })])
    expect(verdictOf(unplayable)).toMatchObject({ kind: 'blocked' })
  })
})

describe('the words of a verdict', () => {
  it.each<[Verdict, string, string]>([
    [{ kind: 'runs' }, 'verified', 'Whole deck runs'],
    [{ kind: 'reads', cards: 4 }, 'read', 'Runs on reads'],
    [{ kind: 'hand', cards: 3, names: [], toRead: 1 }, 'manual_play', '3 cards played by hand'],
    [{ kind: 'blocked', reasons: [] }, 'blocked', 'Cannot be taken'],
  ])('%j is the state %s and says "%s"', (verdict, state, label) => {
    expect(verdictState(verdict)).toBe(state)
    expect(verdictLabel(verdict)).toBe(label)
  })
})
