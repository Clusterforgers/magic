// Goal: the words a player reads spell "color" the American way, as the rules, the cards and the server's deck issues do: no string or text of the app says "colour".
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { describe, expect, it } from 'vitest'

const SRC = import.meta.dirname

function sourcesIn(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) return name === 'generated' ? [] : sourcesIn(path)
    return /\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name) ? [path] : []
  })
}

const COMMENTS = /\/\*[\s\S]*?\*\/|(?<![:'"`])\/\/[^\n]*/g
const LITERALS = /'(?:[^'\\\n]|\\.)*'|"(?:[^"\\\n]|\\.)*"|`(?:[^`\\]|\\.)*`/g
const JSX_TEXT = />([^<>{}]*)</g
const BRITISH = /\bcolours?\b/gi
// What a template computes is code, whatever its identifiers are called.
const SUBSTITUTIONS = /\$\{[^}]*\}/g

/** The British spellings of "colour" in the strings and the JSX text of `source`; identifiers and comments are left alone. */
export function britishWords(source: string): string[] {
  const code = source.replace(COMMENTS, '')
  const spans = [...code.matchAll(LITERALS)].map((found) => found[0])
  spans.push(...[...code.matchAll(JSX_TEXT)].map((found) => found[1] ?? ''))
  return spans.flatMap((span) =>
    [...span.replace(SUBSTITUTIONS, '').matchAll(BRITISH)].map((found) => found[0]),
  )
}

describe('britishWords', () => {
  it.each([
    ['a string', "const label = 'Colour identity'", ['Colour']],
    ['a template', 'const text = `Spells by colour`', ['colour']],
    ['an attribute', '<div aria-label="Within the commander’s colours" />', ['colours']],
    ['JSX text', '<p>Narrow by colour identity</p>', ['colour']],
    ['American words', "const label = 'Color identity, colorless, colors'", []],
    ['an identifier', 'const colours = colourCounts(played)', []],
    ['a template substitution', 'const text = `\ (ability 2)`', []],
    ['a wire key', "const key = 'colour_identity'", []],
    ['a comment', "// The colour of the border\nconst label = 'Border'", []],
    ['a block comment', "/** The colours of a deck. */\nconst label = 'Deck'", []],
  ])('finds the British spelling in %s', (_what, source, found) => {
    expect(britishWords(source)).toEqual(found)
  })
})

describe('the words of the app', () => {
  const files = sourcesIn(SRC)

  it('has files to check', () => {
    expect(files.length).toBeGreaterThan(300)
  })

  it('spell color the American way', () => {
    const found = files.flatMap((path) =>
      britishWords(readFileSync(path, 'utf8')).map((word) => `${relative(SRC, path)}: ${word}`),
    )
    expect(found).toEqual([])
  })
})
