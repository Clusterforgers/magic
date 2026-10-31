// Goal: cards that wait for a player's read are counted in one set of words and sent to one address everywhere a deck is judged: "N cards to read" joins and counts right, the review screen's path names the deck or the precon, and no source writes the phrase outside the shared module.
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { describe, expect, it } from 'vitest'
import {
  cardsOnReads,
  cardsToRead,
  DRAFT_NOTE,
  readPath,
  RUNS_ON_READS,
  TO_READ,
  TO_READ_TITLE,
} from './toRead'

describe('the words', () => {
  it.each([
    [0, 'no cards to read'],
    [1, '1 card to read'],
    [2, '2 cards to read'],
    [14, '14 cards to read'],
  ])('count %i cards as "%s"', (count, words) => {
    expect(cardsToRead(count)).toBe(words)
  })

  it.each([
    [1, '1 card on reads'],
    [4, '4 cards on reads'],
  ])('count %i copies run on reads as "%s"', (count, words) => {
    expect(cardsOnReads(count)).toBe(words)
  })

  it('name the deck state and the title of the filter', () => {
    expect([RUNS_ON_READS, TO_READ, TO_READ_TITLE]).toEqual(['Runs on reads', 'to read', 'To read'])
  })

  it('say what a draft that waits or was found wrong means for the card', () => {
    expect(DRAFT_NOTE.pending).toBe('A draft of its rules waits for a player to read it')
    expect(DRAFT_NOTE.wrong).toContain('played by hand')
  })
})

describe('the address of the review screen', () => {
  it.each([
    [{ deck: '7d1c5b6e-0000-4000-8000-000000000001' }, '/read?deck=7d1c5b6e-0000-4000-8000-000000000001'],
    [{ precon: 'c18-adaptive-enchantment' }, '/read?precon=c18-adaptive-enchantment'],
    [{ precon: 'a b&c' }, '/read?precon=a%20b%26c'],
  ])('is %j at %s', (target, path) => {
    expect(readPath(target)).toBe(path)
  })
})

const WEB = join(import.meta.dirname, '..', '..')

function filesIn(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) return filesIn(path)
    return /\.tsx?$/.test(name) ? [path] : []
  })
}

const noComments = (text: string): string =>
  text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|\s)\/\/.*$/gm, '$1')

describe('the spelling', () => {
  it('writes "cards to read" and "runs on reads" in no source but the shared words', () => {
    const found = filesIn(join(WEB, 'src'))
      .filter(
        (path) =>
          !/\.(?:test|spec)\.tsx?$/.test(path) &&
          !path.endsWith(join('lib', 'toRead.ts')) &&
          !path.includes(join('protocol', 'generated')),
      )
      .filter((path) => /cards? to read|runs on reads/i.test(noComments(readFileSync(path, 'utf8'))))
      .map((path) => relative(WEB, path))
    expect(found).toEqual([])
  })
})
