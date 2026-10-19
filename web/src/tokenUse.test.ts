// Goal: the shared components and the pages outside the table take colour, text size and focus from the tokens of index.css: no hard-coded colour, no palette colour, no pixel or rem size written inline, no size left out of the type scale, no translucent focus ring.
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { describe, expect, it } from 'vitest'

const SRC = import.meta.dirname

function sourcesIn(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) return sourcesIn(path)
    return /\.tsx?$/.test(name) && !/\.test\.tsx?$/.test(name) ? [path] : []
  })
}

// The table is rebuilt by its own tasks; everything else is held to the tokens now.
const FILES = [
  ...sourcesIn(join(SRC, 'components')),
  ...['auth', 'create', 'game', 'games'].flatMap((feature) => sourcesIn(join(SRC, 'features', feature))),
]

const PALETTE =
  '(?:slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose)'
const PAINT = '(?:bg|text|border|ring|fill|stroke|outline|from|to|via|divide|shadow|decoration|accent|caret)'
const SIZED =
  '(?:text|size|w|h|min-w|min-h|max-w|max-h|p[xytblr]?|m[xytblr]?|gap|top|left|right|bottom|inset)'

const RULES: ReadonlyArray<{ name: string; pattern: RegExp }> = [
  { name: 'colour function', pattern: /\b(?:oklch|rgba?|hsla?|color)\(/ },
  { name: 'arbitrary hex colour', pattern: new RegExp(`${PAINT}-\\[#[0-9a-fA-F]{3,8}\\]`) },
  { name: 'palette colour', pattern: new RegExp(`\\b${PAINT}-${PALETTE}-\\d{2,3}\\b`) },
  { name: 'white or black', pattern: new RegExp(`\\b${PAINT}-(?:white|black)\\b`) },
  { name: 'size written in px, rem or em', pattern: new RegExp(`\\b${SIZED}-\\[[0-9.]+(?:px|rem|em)\\]`) },
  { name: 'text size outside the type scale', pattern: /\btext-(?:xl|3xl|5xl|6xl|7xl|8xl|9xl)\b/ },
  { name: 'translucent focus ring', pattern: /\b(?:ring|outline)-ring\/\d+/ },
]

describe('tokens in the shared components and the pages outside the table', () => {
  it('has files to check', () => {
    expect(FILES.length).toBeGreaterThan(40)
  })

  it.each(RULES)('uses no $name', ({ pattern }) => {
    const found: string[] = []
    for (const file of FILES) {
      const lines = readFileSync(file, 'utf8').split('\n')
      lines.forEach((line, index) => {
        // A comment may name what it forbids.
        if (/^\s*(?:\/\/|\*|\/\*)/.test(line)) return
        if (pattern.test(line))
          found.push(`${relative(SRC, file)}:${index + 1}: ${line.trim().slice(0, 100)}`)
      })
    }
    expect(found).toEqual([])
  })
})
