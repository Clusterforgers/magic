// Goal: the verdict of a deck at a table separates a deck that runs whole, one that runs whole without being all Verified (read, from Forge or unverified), one with cards played by hand (with the cards still waiting for a read, and the unplayable ones), carries legality as a warning that never changes how the deck plays, and words each in the badge's state and label.
import { describe, expect, it } from 'vitest'
import { legalityIssue, reportCard, reportOf } from '@/features/decks/fakeDecks'
import type { ReportCard } from '@/protocol/generated'
import { verdictLabel, verdictOf, verdictState } from './verdict'
import type { Play, Verdict } from './verdict'

const card = (name: string, over: Partial<ReportCard> = {}): ReportCard => reportCard(name, over)

describe('verdictOf', () => {
  it.each<[string, ReportCard[], Verdict]>([
    ['a deck that runs whole', [card('Forest', { count: 60 })], { kind: 'runs', warning: null }],
    [
      'a deck with approved reads only',
      [card('Forest', { count: 56 }), card('Odd Card', { count: 4, support: 'read', draft: 'approved' })],
      { kind: 'reads', cards: 4, warning: null },
    ],
    [
      'an unverified draft and one from Forge beside an approved read: the whole deck runs',
      [
        card('Forest', { count: 50 }),
        card('Read Card', { count: 4, support: 'read', draft: 'approved' }),
        card('Unread', { count: 4, support: 'unverified', draft: 'pending' }),
        card('Translated', { count: 2, support: 'forge', draft: 'approved' }),
      ],
      { kind: 'reads', cards: 10, warning: null },
    ],
    [
      'a partial draft nobody has read beside approved ones',
      [
        card('Forest', { count: 54 }),
        card('Read Card', { count: 4, support: 'read', draft: 'approved' }),
        card('Waiting', { count: 2, support: 'manual', draft: 'pending' }),
      ],
      { kind: 'hand', cards: 2, names: ['Waiting'], toRead: 1, warning: null },
    ],
    [
      'a wrong draft and a card with no draft are played by hand but not to read',
      [card('Wrong', { support: 'manual', draft: 'wrong' }), card('Plain', { support: 'partial' })],
      { kind: 'hand', cards: 2, names: ['Wrong', 'Plain'], toRead: 0, warning: null },
    ],
  ])('says %s', (_what, cards, verdict) => {
    expect(verdictOf(reportOf(cards, [], { format: 'commander' }))).toEqual(verdict)
  })

  it.each([
    ['a banned card', 'banned', 'Mana Crypt is banned in Commander'],
    ['an off-identity card', 'color_identity', "Lightning Bolt has R outside the commander's identity"],
    ['a wrong size', 'deck_size', 'A Commander deck has exactly 100 cards'],
  ] as const)('warns of %s and still plays the deck as its cards say', (_what, code, message) => {
    const report = reportOf(
      [card('Read Card', { support: 'read', draft: 'approved' })],
      [legalityIssue(message, { code })],
    )
    expect(verdictOf(report)).toEqual({
      kind: 'reads',
      cards: 1,
      warning: { issues: [{ code }], reasons: [`${message}.`] },
    })
  })

  it('plays an unplayable card by hand, with no warning', () => {
    const unplayable = reportOf([card('Forest', { count: 59 }), card('Odd', { support: 'unplayable' })])
    expect(verdictOf(unplayable)).toEqual({ kind: 'hand', cards: 1, names: ['Odd'], toRead: 0, warning: null })
  })
})

describe('the words of a verdict', () => {
  it.each<[Play, string, string]>([
    [{ kind: 'runs' }, 'verified', 'Whole deck runs'],
    [{ kind: 'reads', cards: 4 }, 'read', 'Whole deck runs'],
    [{ kind: 'hand', cards: 3, names: [], toRead: 1 }, 'manual_play', '3 cards played by hand'],
  ])('%j is the state %s and says "%s"', (verdict, state, label) => {
    expect(verdictState(verdict)).toBe(state)
    expect(verdictLabel(verdict)).toBe(label)
  })
})
