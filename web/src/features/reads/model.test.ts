// Goal: the review screen's pure rules hold: each row state has one tone and the engine's words come from the writer or the differing wording, the counts read as a sentence, a wrong note is required and bounded, the address names one deck, the next card skips the judged ones and wraps, a verdict key is C or W outside a text field only, and the table offers a read for a card on a read or with clauses by hand.
import { describe, expect, it } from 'vitest'
import type { ReadRow } from '@/protocol/generated'
import { READ_CARDS, READ_QUEUE } from './dev/fixture'
import {
  MAX_READ_NOTE_CHARS,
  countsLine,
  differsCount,
  engineWords,
  nextToRead,
  noteProblem,
  parseReadSearch,
  targetOf,
  toneOf,
  verdictKey,
} from './model'
import { readableOf } from './readable'

const ROW = Object.values(READ_CARDS)[0]!.sheet.rows[0]!
const withState = (state: ReadRow['state'], wording: string | null = null): ReadRow => ({
  ...ROW,
  state,
  wording,
})

describe('rows', () => {
  it.each([
    [withState('written', 'Flying'), 'written', 'Flying'],
    [withState('no_clause'), 'no_clause', null],
    [withState({ differs: { written: 'Surveil 1.' } }), 'differs', 'Surveil 1.'],
    [withState({ differs: { written: 'Surveil 1.' } }, 'Own words.'), 'differs', 'Own words.'],
    [withState({ unwritten: { construct: 'x' } }), 'unwritten', null],
    [withState({ by_hand: { reason: null } }), 'by_hand', null],
  ])('reads a row as its tone and engine words', (row, tone, words) => {
    expect(toneOf(row)).toBe(tone)
    expect(engineWords(row)).toBe(words)
  })

  it('counts the clauses that differ', () => {
    expect(Object.values(READ_CARDS).map(differsCount)).toEqual([1, 0, 0])
    expect(READ_QUEUE.cards.map((card) => card.differs)).toEqual([1, 0, 0])
  })
})

describe('countsLine', () => {
  it.each([
    [{ correct: 0, wrong: 0, forge: false }, null],
    [{ correct: 0, wrong: 0, forge: true }, 'Translated from Forge; nobody has read it yet'],
    [{ correct: 1, wrong: 0, forge: false }, 'Found correct by 1 player'],
    [{ correct: 3, wrong: 2, forge: false }, 'Found correct by 3 players, found wrong 2 times'],
    [{ correct: 0, wrong: 1, forge: true }, 'Found wrong once'],
  ])('reads %j', (counts, line) => {
    expect(countsLine(counts)).toBe(line)
  })
})

describe('noteProblem', () => {
  it.each([
    ['', 'Say what the rules do wrong.'],
    ['   \n ', 'Say what the rules do wrong.'],
    ['it\tdraws', 'Remove the tab or control character.'],
    ['é'.repeat(MAX_READ_NOTE_CHARS + 1), `Keep the note to ${MAX_READ_NOTE_CHARS} characters.`],
  ])('refuses %j', (note, problem) => {
    expect(noteProblem(note)).toBe(problem)
  })

  it.each(['draws two', 'line one\nline two', 'é'.repeat(MAX_READ_NOTE_CHARS)])('takes %j', (note) => {
    expect(noteProblem(note)).toBeNull()
  })
})

describe('the address', () => {
  it.each([
    [{}, null],
    [{ deck: ' d1 ' }, { deck: 'd1' }],
    [{ precon: 'c18' }, { precon: 'c18' }],
    [{ deck: 'd1', precon: 'c18' }, { deck: 'd1' }],
    [{ deck: '', precon: 'c18' }, { precon: 'c18' }],
    [{ deck: 3 }, null],
  ])('reads %j', (search, target) => {
    expect(targetOf(parseReadSearch(search))).toEqual(target)
  })
})

describe('nextToRead', () => {
  const cards = READ_QUEUE.cards
  const [a, b, c] = cards.map((card) => card.oracle_id) as [string, string, string]
  it.each([
    [new Set<string>(), null, a],
    [new Set([a]), a, b],
    [new Set([a, b]), b, c],
    [new Set([b, c]), c, a],
    [new Set([a, b, c]), b, null],
  ])('after %j at %s goes to %s', (done, current, next) => {
    expect(nextToRead(cards, done, current)).toBe(next)
  })
})

describe('verdictKey', () => {
  const key = (key: string, rest: Partial<KeyboardEvent> = {}, target: EventTarget | null = document.body) =>
    verdictKey({ key, ctrlKey: false, metaKey: false, altKey: false, target, ...rest })
  it('reads C and W in any case', () => {
    expect([key('c'), key('C'), key('w'), key('W'), key('x')]).toEqual([
      'correct',
      'correct',
      'wrong',
      'wrong',
      null,
    ])
  })
  it('ignores a key with a modifier or in a text field', () => {
    expect(key('c', { ctrlKey: true })).toBeNull()
    expect(key('w', { metaKey: true })).toBeNull()
    expect(key('c', {}, document.createElement('textarea'))).toBeNull()
    expect(key('c', {}, document.createElement('input'))).toBeNull()
  })
})

describe('readableOf', () => {
  const object = { name: 'Opt', identity: { oracle_id: 'o1' } }
  it.each([
    [{ ...object, read: true }, 0, { oracleId: 'o1', name: 'Opt' }],
    [{ ...object }, 2, { oracleId: 'o1', name: 'Opt' }],
    [{ ...object }, 0, null],
    [{ name: 'Opt', read: true }, 1, null],
  ])('offers %j with %i clauses by hand', (card, byHand, reading) => {
    expect(readableOf(card, byHand)).toEqual(reading)
  })
})
