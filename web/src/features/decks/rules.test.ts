// Goal: the name and list checks accept what the server accepts (a name of 1 to 100 characters without control or invisible characters; a list of at most 100,000 characters and 1,000 lines) and say plainly what is wrong with the rest; their limits are the constants of the Rust protocol crate.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import {
  fileProblem,
  listProblem,
  MAX_DECK_NAME_CHARS,
  MAX_IMPORT_CHARS,
  MAX_IMPORT_FILE_BYTES,
  MAX_IMPORT_LINES,
  nameFromFile,
  nameProblem,
} from './rules'

describe('the limits', () => {
  const source = readFileSync(
    join(import.meta.dirname, '../../../../crates/mtg-protocol/src/rest/decks.rs'),
    'utf8',
  )
  const constant = (name: string): number => {
    const found = new RegExp(`pub const ${name}: \\w+ = ([\\d_]+);`).exec(source)?.[1]
    return Number((found ?? '').replaceAll('_', ''))
  }

  it.each([
    ['MAX_DECK_NAME_CHARS', MAX_DECK_NAME_CHARS],
    ['MAX_IMPORT_CHARS', MAX_IMPORT_CHARS],
    ['MAX_IMPORT_LINES', MAX_IMPORT_LINES],
  ])('copies %s of the Rust crate', (name, value) => {
    expect(constant(name)).toBeGreaterThan(0)
    expect(value).toBe(constant(name))
  })

  it('keeps the file limit in bytes at the 640 KiB the server reads for an import', () => {
    expect(MAX_IMPORT_FILE_BYTES).toBe(640 * 1024)
  })
})

describe('nameProblem', () => {
  it.each([
    ['one character', 'x'],
    ['the longest name', 'n'.repeat(MAX_DECK_NAME_CHARS)],
    ['the longest name of accented letters', 'é'.repeat(MAX_DECK_NAME_CHARS)],
    ['the longest name of emoji, which are one character each', '\u{1F409}'.repeat(MAX_DECK_NAME_CHARS)],
    ['spaces and punctuation inside', "Atraxa, Praetors' Voice (v2)"],
    ['a name with a leading space', ' Burn'],
  ])('accepts %s', (_what, name) => {
    expect(nameProblem(name)).toBeNull()
  })

  it.each([
    ['nothing', '', /enter a name/i],
    ['only spaces', '   ', /enter a name/i],
    ['101 characters', 'n'.repeat(MAX_DECK_NAME_CHARS + 1), /at most 100 characters \(this one has 101\)/],
    ['a newline', 'Burn\nDeck', /control characters/],
    ['a tab', 'Burn\tDeck', /control characters/],
    ['a zero-width space', 'Burn​Deck', /invisible marks/],
    ['a right-to-left override', 'Burn‮Deck', /invisible marks/],
    ['a byte order mark', '﻿Burn', /invisible marks/],
  ])('refuses %s', (_what, name, words) => {
    expect(nameProblem(name)).toMatch(words)
  })
})

describe('listProblem', () => {
  it.each([
    ['one line', '4 Lightning Bolt'],
    ['a trailing newline', '4 Lightning Bolt\n'],
    ['exactly the most characters', 'x'.repeat(MAX_IMPORT_CHARS)],
    ['exactly the most lines', Array.from({ length: MAX_IMPORT_LINES }, () => '1 Island').join('\n')],
    [
      'the most lines with a trailing newline',
      `${Array.from({ length: MAX_IMPORT_LINES }, () => '1 Island').join('\n')}\n`,
    ],
    ['Windows line ends', '4 Lightning Bolt\r\n20 Mountain\r\n'],
  ])('accepts %s', (_what, text) => {
    expect(listProblem(text)).toBeNull()
  })

  it.each([
    ['nothing', '', /paste a deck list/i],
    ['only blank lines', '\n  \n', /paste a deck list/i],
    [
      'one character too many',
      'x'.repeat(MAX_IMPORT_CHARS + 1),
      /at most 100,000 characters \(this one has 100,001\)/,
    ],
    [
      'one line too many',
      Array.from({ length: MAX_IMPORT_LINES + 1 }, () => '1 Island').join('\n'),
      /at most 1,000 lines \(this one has 1,001\)/,
    ],
  ])('refuses %s', (_what, text, words) => {
    expect(listProblem(text)).toMatch(words)
  })

  it('counts characters rather than UTF-16 units, as the server does', () => {
    const dragons = '\u{1F409}'.repeat(MAX_IMPORT_CHARS / 2 + 1)
    expect(dragons.length).toBeGreaterThan(MAX_IMPORT_CHARS)
    expect(listProblem(dragons)).toBeNull()
  })
})

describe('fileProblem', () => {
  it.each([
    [0, null],
    [MAX_IMPORT_FILE_BYTES, null],
    [MAX_IMPORT_FILE_BYTES + 1, /too big for a deck list/],
  ])('judges a file of %i bytes', (size, words) => {
    const problem = fileProblem({ size })
    if (words === null) expect(problem).toBeNull()
    else expect(problem).toMatch(words)
  })
})

describe('nameFromFile', () => {
  it.each([
    ['Atraxa Superfriends.txt', 'Atraxa Superfriends'],
    ['burn.dek', 'burn'],
    ['my.deck.v2.txt', 'my.deck.v2'],
    ['no extension', 'no extension'],
    ['  padded .txt', 'padded'],
    ['.txt', ''],
    ['', ''],
    [`${'n'.repeat(150)}.txt`, 'n'.repeat(MAX_DECK_NAME_CHARS)],
  ])('suggests the name of %j as %j', (file, name) => {
    expect(nameFromFile(file)).toBe(name)
  })
})
