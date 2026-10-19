// Goal: the deck check as the builder draws it: lines are ordered by name or with the worst state first (equal states keep their name order), the issues-only filter keeps exactly the lines whose card is not verified, a line takes its check from the report by card whatever its zone and falls back to the card's own state for a card added since, reasons become plain sentences (naming the face only when a clause is on a face other than the first) and legality issues become plain sentences.
import { describe, expect, it } from 'vitest'
import { cardHit, legalityIssue, reportCard, reportOf } from '@/features/decks/fakeDecks'
import type { Reason, Support } from '@/protocol/generated'
import type { Line } from './deckEdit'
import {
  arrange,
  checkOf,
  checksOf,
  isIssue,
  legalityLines,
  reasonLines,
  sentence,
  SUPPORT_ORDER,
} from './report'

const line = (name: string, support: Support, zone: Line['zone'] = 'main'): Line => ({
  zone,
  card: cardHit(name, { support }),
  count: 1,
  printingId: null,
})
const names = (lines: readonly Line[]): string[] => lines.map((item) => item.card.name)
const reason = (text: string, face = 0): Reason => ({ face, span: { start: 3, end: 9 }, text })

// In name order, as `linesIn` gives them.
const LINES = [
  line('Ant', 'verified'),
  line('Bat', 'partial'),
  line('Cow', 'manual'),
  line('Dog', 'verified'),
  line('Eel', 'unplayable'),
  line('Fox', 'stale'),
  line('Gnu', 'manual'),
]
const checks = checksOf(
  reportOf(LINES.map((item) => reportCard(item.card.name, { support: item.card.support }))),
)

describe('arrange', () => {
  it.each([
    ['by name', false, 'name', ['Ant', 'Bat', 'Cow', 'Dog', 'Eel', 'Fox', 'Gnu']],
    ['issues first', false, 'issues', ['Eel', 'Cow', 'Gnu', 'Fox', 'Bat', 'Ant', 'Dog']],
    ['issues only by name', true, 'name', ['Bat', 'Cow', 'Eel', 'Fox', 'Gnu']],
    ['issues only, issues first', true, 'issues', ['Eel', 'Cow', 'Gnu', 'Fox', 'Bat']],
  ] as const)('shows %s', (_what, issuesOnly, sort, expected) => {
    expect(names(arrange(LINES, checks, { issuesOnly, sort }))).toEqual(expected)
  })

  it('keeps no verified line under issues only, and every line without it', () => {
    const shown = arrange(LINES, checks, { issuesOnly: true, sort: 'name' })
    expect(shown.every((item) => isIssue(checkOf(checks, item).support))).toBe(true)
    expect(arrange(LINES, checks, { issuesOnly: false, sort: 'name' })).toHaveLength(LINES.length)
  })

  it('leaves the lines it was given as they were', () => {
    const before = names(LINES)
    arrange(LINES, checks, { issuesOnly: false, sort: 'issues' })
    expect(names(LINES)).toEqual(before)
  })

  it('shows nothing for no lines', () => {
    expect(arrange([], checks, { issuesOnly: true, sort: 'issues' })).toEqual([])
  })
})

describe('the order of the states', () => {
  it('goes from the worst to verified, and only verified is not an issue', () => {
    expect(SUPPORT_ORDER).toEqual(['unplayable', 'manual', 'stale', 'partial', 'verified'])
    expect(SUPPORT_ORDER.filter(isIssue)).toEqual(['unplayable', 'manual', 'stale', 'partial'])
  })
})

describe('checkOf', () => {
  it('reads the report, by card, from whichever zone the report has it in', () => {
    const report = reportOf([
      reportCard('Bat', { zone: 'main', support: 'partial', reasons: [reason('half done')] }),
    ])
    const moved = line('Bat', 'verified', 'sideboard')
    expect(checkOf(checksOf(report), moved)).toEqual({ support: 'partial', reasons: [reason('half done')] })
  })

  it('takes the first entry of a card that the report lists in two zones', () => {
    const report = reportOf([
      reportCard('Bat', { zone: 'main', support: 'manual' }),
      reportCard('Bat', { zone: 'sideboard', support: 'verified' }),
    ])
    expect(checksOf(report).get(reportCard('Bat').oracle_id)?.support).toBe('manual')
  })

  it('gives a card the report does not know the state it came with and no reasons', () => {
    const added = line('Newcomer', 'manual')
    expect(checkOf(checksOf(reportOf([])), added)).toEqual({ support: 'manual', reasons: [] })
  })
})

describe('sentence', () => {
  it.each([
    ['the engine does not run goad yet', 'The engine does not run goad yet.'],
    ['  padded  ', 'Padded.'],
    ['Already a sentence.', 'Already a sentence.'],
    ['a question?', 'A question?'],
    ['an ellipsis…', 'An ellipsis…'],
    ['', ''],
    ['   ', ''],
  ])('makes %j into %j', (text, expected) => {
    expect(sentence(text)).toBe(expected)
  })
})

describe('reasonLines', () => {
  it('has no line for a card with no reasons', () => {
    expect(reasonLines([])).toEqual([])
  })

  it('gives each reason as a plain sentence, in the report order', () => {
    const lines = reasonLines([reason('first thing'), reason('second thing.')])
    expect(lines.map((item) => item.text)).toEqual(['First thing.', 'Second thing.'])
  })

  it('names the face on every line once a clause is on a face other than the first', () => {
    const lines = reasonLines([reason('front part', 0), reason('back part', 1), reason('third part', 2)])
    expect(lines.map((item) => item.text)).toEqual([
      'Front face: front part.',
      'Back face: back part.',
      'Face 3: third part.',
    ])
  })

  it('gives every line its own key, even for two equal reasons', () => {
    const lines = reasonLines([reason('same'), reason('same')])
    expect(new Set(lines.map((item) => item.key)).size).toBe(2)
  })
})

describe('legalityLines', () => {
  it('turns each issue message into a plain sentence, in order', () => {
    const lines = legalityLines([
      legalityIssue('the main deck needs at least 60 cards, this one has 52', { code: 'deck_size' }),
      legalityIssue('Black Lotus is banned in modern', { code: 'banned', name: 'Black Lotus' }),
    ])
    expect(lines.map((item) => item.text)).toEqual([
      'The main deck needs at least 60 cards, this one has 52.',
      'Black Lotus is banned in modern.',
    ])
  })

  it('has nothing for a legal deck', () => {
    expect(legalityLines([])).toEqual([])
  })
})
