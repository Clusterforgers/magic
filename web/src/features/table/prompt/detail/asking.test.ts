// Goal: every decision kind either brings the Stack column forward or leaves it alone, the asker is the server's `asking` (else the object the decision names), a clause splits its text in three and is quoted alone, a bad one marking nothing, the "you may" of a card's own text is found with the sentences that depend on it, and the kicker, a run's rows and the sheet's line say where the question stands.
import { describe, expect, it } from 'vitest'
import type { PendingDecision } from '@/play/store'
import type { AskingView, DecisionKind, RunView } from '@/protocol/generated'
import { KINDS } from '@/test-utils/decisions'
import {
  askedWords,
  askerOf,
  askKicker,
  clauseParts,
  detailKind,
  hasDetail,
  kindKey,
  mayClause,
  namedSource,
  runPlace,
  runRowWords,
  sheetTitle,
} from './asking'

const pendingOf = (kind: DecisionKind, asking?: AskingView): PendingDecision => ({
  id: 1,
  decision: { player: 0, kind },
  ...(asking === undefined ? {} : { asking }),
})

const WITHOUT = ['Priority', 'Mulligan', 'BottomCards', 'DeclareAttackers', 'DeclareBlockers', 'Discard']

const RUN: RunView = {
  id: 7,
  index: 2,
  entries: [
    { name: 'Alpha', done: true },
    { name: 'Beta', object: 31, text: 'You may gain 1 life.' },
    { name: 'Gamma', object: 30, text: 'Draw a card.' },
    { name: 'Delta', object: 29 },
  ],
}

describe('which decisions have detail', () => {
  it.each(Object.entries(KINDS))('%s', (name, kind) => {
    const expected = !WITHOUT.includes(name)
    expect(kindKey(kind)).toBe(name)
    expect(detailKind(name)).toBe(expected)
    expect(hasDetail(pendingOf(kind))).toBe(expected)
  })

  it('none without a decision, and none for a kind with no name', () => {
    expect(hasDetail(null)).toBe(false)
    expect(detailKind('')).toBe(false)
  })
})

describe('what a decision names as its source', () => {
  it.each<[string, DecisionKind, number | undefined]>([
    ['a source', { YesNo: { source: 12 } }, 12],
    ['the spell of a payment', must(KINDS.PayMana), 1],
    ['triggers to order', { OrderTriggers: { triggers: [] } }, undefined],
    ['priority', must(KINDS.Priority), undefined],
  ])('%s', (_what, kind, source) => {
    expect(namedSource(kind)).toBe(source)
  })
})

function must<T>(value: T | undefined): T {
  if (value === undefined) throw new Error('missing')
  return value
}

describe('the asker', () => {
  it('is the server`s asking where it sent one', () => {
    const asking: AskingView = {
      stack: 40,
      resolving: true,
      source: 12,
      name: 'Mentor',
      text: 'You may pay {1}.',
      clause: [0, 16],
      cause: { object: 9, name: 'Brago', entered: true },
      run: RUN,
    }
    expect(askerOf(pendingOf({ YesNo: { source: 12, text: 'Pay {1}?' } }, asking))).toEqual({
      source: 12,
      stack: 40,
      resolving: true,
      name: 'Mentor',
      text: 'You may pay {1}.',
      clause: [0, 16],
      cause: asking.cause,
      run: RUN,
    })
  })

  it('is the named object alone where the server sent none', () => {
    expect(askerOf(pendingOf({ YesNo: { source: 12 } }))).toMatchObject({
      source: 12,
      stack: undefined,
      name: '',
    })
  })

  it('does not point at a source the server left out', () => {
    const asker = askerOf(pendingOf({ YesNo: { source: 12 } }, { stack: 40, name: 'Martyr' }))
    expect(asker).toMatchObject({ source: undefined, stack: 40, name: 'Martyr', resolving: false })
  })

  it.each(WITHOUT)('is none for %s', (name) => {
    expect(askerOf(pendingOf(must(KINDS[name as keyof typeof KINDS]), { name: 'X' }))).toBeNull()
  })

  it('is none where nothing is named', () => {
    expect(askerOf(pendingOf({ OrderTriggers: { triggers: [] } }))).toBeNull()
    expect(askerOf(null)).toBeNull()
  })
})

describe('the asked clause', () => {
  const text = 'Whenever a creature enters, you may pay {1}. If you do, draw a card.'

  it.each<[string, readonly [number, number] | undefined, [string, string, string]]>([
    [
      'a clause in the middle',
      [28, 44],
      ['Whenever a creature enters, ', 'you may pay {1}.', ' If you do, draw a card.'],
    ],
    ['a clause to the end', [28, text.length], ['Whenever a creature enters, ', text.slice(28), '']],
    ['no clause', undefined, [text, '', '']],
    ['a clause past the text', [28, 400], [text, '', '']],
    ['an empty clause', [5, 5], [text, '', '']],
    ['a clause that runs backwards', [9, 3], [text, '', '']],
  ])('%s', (_what, clause, [before, marked, after]) => {
    expect(clauseParts(text, clause)).toEqual({ before, marked, after })
  })

  it.each<[string, readonly [number, number] | undefined, string | null]>([
    ['the words marked', [28, 44], 'you may pay {1}.'],
    ['without the space around them', [27, 45], 'you may pay {1}.'],
    ['nothing for no clause', undefined, null],
    ['nothing for a clause of spaces', [27, 28], null],
    ['nothing for a clause past the text', [28, 400], null],
  ])('quotes %s', (_what, clause, words) => {
    expect(askedWords(text, clause)).toBe(words)
  })

  it.each<[string, string, string, string | undefined]>([
    [
      'the may the question names, with what depends on it',
      text,
      'Pay {1}?',
      'you may pay {1}. If you do, draw a card.',
    ],
    ['a plain may', 'When it enters, you may gain 1 life.', 'Gain 1 life?', 'you may gain 1 life.'],
    [
      'the second of two',
      'You may draw a card. Then you may discard a card. If you do, gain 2 life.',
      'Discard a card?',
      'you may discard a card. If you do, gain 2 life.',
    ],
    [
      'the first may for a question in other words',
      'You may scry 1. Draw a card.',
      'Look?',
      'You may scry 1.',
    ],
    ['the first may for no question', 'At your upkeep, you may scry 1.', '', 'you may scry 1.'],
    ['nothing in a text with no may and no such words', 'Each player loses 1 life.', 'Pay {2}?', undefined],
  ])('finds %s', (_what, words, question, clause) => {
    const found = mayClause(words, question)
    expect(found === undefined ? undefined : words.slice(found[0], found[1])).toBe(clause)
  })
})

describe('the words around the asker', () => {
  const base = { source: 12, stack: 40, resolving: true, name: 'Mentor', text: undefined, clause: undefined }

  it.each<[string, Partial<Parameters<typeof askKicker>[0]>, string, string]>([
    ['resolving', {}, 'Resolving now', 'asks you'],
    ['being cast or put on the stack', { resolving: false }, 'Going on the stack', 'asks you'],
    ['off the stack', { resolving: false, stack: undefined }, 'Asks you', ''],
    [
      'one of a run with a cause',
      { run: RUN, cause: { object: 9, name: 'Brago' } },
      'Brago',
      'trigger 2 of 4',
    ],
    ['one of a run without', { run: RUN }, 'Your triggers', 'trigger 2 of 4'],
  ])('%s', (_what, change, lead, rest) => {
    expect(askKicker({ ...base, cause: undefined, run: undefined, ...change })).toEqual({ lead, rest })
  })

  it('places a run', () => {
    expect(runPlace(RUN)).toBe('trigger 2 of 4')
  })

  it.each<[number, string | undefined, string]>([
    [0, 'Yes · Gain 1 life', 'Yes · Gain 1 life'],
    [0, undefined, 'Done'],
    [2, undefined, 'Next · Draw a card.'],
    [3, undefined, 'Then'],
  ])('row %i of the run reads its outcome or its effect', (place, outcome, words) => {
    expect(runRowWords(must(RUN.entries[place]), place, RUN.index, outcome)).toBe(words)
  })

  it.each<[string, Parameters<typeof sheetTitle>[0], boolean, string]>([
    ['no asker', null, false, 'Order triggers'],
    ['an asker', { ...base, cause: undefined, run: undefined, shown: 'Mentor' }, false, 'Mentor asks you'],
    ['a payment', { ...base, cause: undefined, run: undefined, shown: 'Brago' }, true, 'Pay for Brago'],
    [
      'a run',
      { ...base, cause: { object: 9, name: 'Brago' }, run: RUN, shown: 'Beta' },
      false,
      'Brago · 2 of 4',
    ],
    [
      'an asker with no name',
      { ...base, cause: undefined, run: undefined, shown: '' },
      false,
      'Order triggers',
    ],
  ])('the sheet names %s', (_what, asker, paying, title) => {
    expect(sheetTitle(asker, 'Order triggers', paying)).toBe(title)
  })
})
