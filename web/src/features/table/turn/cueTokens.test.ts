// Goal: every colour the turn cues draw with is defined for the light and the dark theme, mapped to a utility, and meets its contrast in both: the banner's text on its fill (4.5:1), and the ladder's marks (current, passed, skipped, upcoming) against every surface the bar can sit on (3:1), the way tokens.test.ts holds the rest of the palette.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { contrastRatio, parseOklch, toSrgb } from '@/test-utils/color'
import type { Mapping, Rgb } from '@/test-utils/color'
import { blockAfter } from '@/test-utils/cssBlocks'

// Read as text rather than imported, which would hand back what Tailwind builds from it.
const css = readFileSync(join(import.meta.dirname, '../../../index.css'), 'utf8')

type Theme = 'light' | 'dark'

function declarations(block: string): Map<string, string> {
  const found = new Map<string, string>()
  for (const match of block.matchAll(/(?:^|[\s;])--([a-z0-9-]+)\s*:\s*([^;]+);/g)) {
    found.set(match[1] ?? '', (match[2] ?? '').trim())
  }
  return found
}

const root = declarations(blockAfter(css, '\n:root {'))
const dark = declarations(blockAfter(css, '\n.dark {'))
const theme = declarations(blockAfter(css, '@theme inline'))
const VARS: Record<Theme, ReadonlyMap<string, string>> = { light: root, dark: new Map([...root, ...dark]) }
const MAPPINGS: readonly Mapping[] = ['clip', 'chroma']

function valueOf(vars: ReadonlyMap<string, string>, name: string): string {
  const value = vars.get(name)
  if (value === undefined) throw new Error(`--${name} is not defined`)
  const alias = /^var\(--([a-z0-9-]+)\)$/.exec(value)
  return alias === null ? value : valueOf(vars, alias[1] ?? '')
}

function paint(name: Theme, token: string, mapping: Mapping): Rgb {
  return toSrgb(parseOklch(valueOf(VARS[name], token)), mapping)
}

/** The colours the cues add to the palette; each is set in both themes, not borrowed from another token. */
const CUE_TOKENS = ['on-turn-them', 'step-passed', 'step-skipped'] as const

const SURFACES = ['background', 'surface', 'table', 'raised'] as const

interface Pair {
  use: string
  fg: string
  bg: string
  min: number
}

const pairs: Pair[] = [
  { use: 'banner on your turn', fg: 'on-turn-you', bg: 'turn-you', min: 4.5 },
  { use: 'banner on another seat’s turn', fg: 'on-turn-them', bg: 'turn-them', min: 4.5 },
  { use: 'ticker line', fg: 'muted-foreground', bg: 'card', min: 4.5 },
  ...SURFACES.flatMap((bg) => [
    { use: 'passed step', fg: 'step-passed', bg, min: 3 },
    { use: 'skipped step', fg: 'step-skipped', bg, min: 3 },
    { use: 'step still to come', fg: 'border-strong', bg, min: 3 },
    { use: 'current step on your turn', fg: 'turn-you-edge', bg, min: 3 },
    { use: 'current step on another seat’s turn', fg: 'turn-them', bg, min: 3 },
  ]),
]

describe('the cue tokens', () => {
  it.each(CUE_TOKENS)('--%s is set for both themes and mapped to a colour utility', (token) => {
    expect(root.has(token), `${token} in :root`).toBe(true)
    expect(dark.has(token), `${token} in .dark`).toBe(true)
    expect(theme.get(`color-${token}`)).toBe(`var(--${token})`)
  })

  it('has no cue token that the contrast pairs below leave out', () => {
    const used = new Set(pairs.flatMap((pair) => [pair.fg, pair.bg]))
    for (const token of CUE_TOKENS) expect(used.has(token), token).toBe(true)
  })

  it('keeps the ladder’s marks apart by shape as well as colour: dashed for skipped, filled for passed', () => {
    const ladder = readFileSync(join(import.meta.dirname, 'StepLadder.tsx'), 'utf8')
    expect(ladder).toMatch(/skipped:[^\n]*border-dashed/)
    expect(ladder).toMatch(/passed:[^\n]*bg-step-passed/)
  })
})

describe.each(['light', 'dark'] as const)('contrast of the turn cues in the %s theme', (name) => {
  it.each(pairs)('$use: $fg on $bg is at least $min:1', ({ fg, bg, min }) => {
    for (const mapping of MAPPINGS) {
      const ratio = contrastRatio(paint(name, fg, mapping), paint(name, bg, mapping))
      expect(ratio, `${fg} on ${bg}, ${mapping}`).toBeGreaterThanOrEqual(min)
    }
  })
})
