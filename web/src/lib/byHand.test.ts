// Goal: the cards the engine plays only in part are called "played by hand" everywhere: the shared words count and join right, the check state and the counts line use them, and no source or end-to-end spec spells the idea another way or writes the phrase outside the shared module.
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { describe, expect, it } from 'vitest'
import { countsText, STATE_LABEL } from '@/features/decks/format'
import { cardsByHand, BY_HAND, BY_HAND_TITLE, byHandCopies, PARTLY_BY_HAND } from './byHand'
import type { Counts } from '@/protocol/generated'

const counts = (found: Partial<Counts>): Counts => ({
  total: 100,
  verified: 100,
  partial: 0,
  manual: 0,
  stale: 0,
  unplayable: 0,
  ...found,
})

describe('the words', () => {
  it('are one phrase in three shapes', () => {
    expect([BY_HAND, BY_HAND_TITLE, PARTLY_BY_HAND]).toEqual([
      'played by hand',
      'Played by hand',
      'partly played by hand',
    ])
  })

  it.each([
    [0, 'no cards played by hand'],
    [1, '1 card played by hand'],
    [2, '2 cards played by hand'],
    [60, '60 cards played by hand'],
  ])('count %i cards as "%s"', (count, words) => {
    expect(cardsByHand(count)).toBe(words)
  })

  it.each([
    ['a clean deck', counts({}), 0],
    ['partial, manual and stale copies add up', counts({ partial: 1, manual: 2, stale: 3 }), 6],
    ['unplayable copies are not played at all', counts({ unplayable: 4 }), 0],
  ])('count the copies of %s', (_what, given, copies) => {
    expect(byHandCopies(given)).toBe(copies)
  })

  it('name the check state of a deck and its counts', () => {
    expect(STATE_LABEL.manual_play).toBe(BY_HAND_TITLE)
    expect(countsText(counts({ verified: 93, partial: 1, manual: 2, stale: 3, unplayable: 1 }))).toBe(
      '93 of 100 verified, 6 played by hand, 1 unplayable',
    )
  })
})

const WEB = join(import.meta.dirname, '..', '..')

function filesIn(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) return name === 'generated' ? [] : filesIn(path)
    return /\.tsx?$/.test(name) ? [path] : []
  })
}

const ALL = [...filesIn(join(WEB, 'src')), ...filesIn(join(WEB, 'e2e'))].filter(
  (path) => path !== import.meta.filename,
)

const isTest = (path: string): boolean => /\.(?:test|spec)\.tsx?$/.test(path)

const noComments = (text: string): string =>
  text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|\s)\/\/.*$/gm, '$1')

// Each way the idea was said before the shared words; the state names of a single card (Partial, Manual) are not among
// them, but a count of copies of them in a sentence is.
const OLD_SPELLINGS: ReadonlyArray<readonly [string, RegExp]> = [
  ['manual steps', /manual steps/i],
  ['Manual play', /manual play/i],
  ['needs manual', /needs? manual/i],
  ['without manual', /without manual/i],
  ['manual handling', /manual handling/i],
  ['a count of manual or partial copies', /\b\d+ (?:manual|partial)\b/],
  ['Some cards by hand', /some cards by hand/i],
]

describe('the spelling', () => {
  it('finds the sources to look through', () => {
    expect(ALL.some((path) => path.endsWith(join('features', 'game', 'deckWords.ts')))).toBe(true)
    expect(ALL.some((path) => path.endsWith(join('e2e', 'lobby.ts')))).toBe(true)
  })

  it.each(OLD_SPELLINGS)('has no "%s" in the sources or the end-to-end specs', (_name, pattern) => {
    const found = ALL.filter((path) => pattern.test(readFileSync(path, 'utf8'))).map((path) =>
      relative(WEB, path),
    )
    expect(found).toEqual([])
  })

  it('writes "played by hand" in no source but the shared words', () => {
    const found = ALL.filter(
      (path) =>
        !isTest(path) &&
        !path.endsWith(join('lib', 'byHand.ts')) &&
        /played by hand/i.test(noComments(readFileSync(path, 'utf8'))),
    ).map((path) => relative(WEB, path))
    expect(found).toEqual([])
  })
})
